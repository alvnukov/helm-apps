package inspectweb

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"os/exec"
	"runtime"
	"sort"
	"strings"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/analyze"
	"gopkg.in/yaml.v3"
)

func BuildModel(docs []map[string]any, rep *analyze.Report, preview *HelmAppsPreview) Model {
	resources := make([]Resource, 0, len(docs))
	for _, d := range docs {
		r := resourceFromDoc(d)
		resources = append(resources, r)
	}
	sort.Slice(resources, func(i, j int) bool { return resources[i].ID < resources[j].ID })
	relations := buildRelations(resources)
	apps := buildApplications(resources, relations)
	return Model{
		Resources:    resources,
		Relations:    relations,
		Applications: apps,
		Analysis:     rep,
		HelmApps:     preview,
		Summary: Summary{
			ResourceCount:    len(resources),
			RelationCount:    len(relations),
			ApplicationCount: len(apps),
		},
	}
}

func Serve(addr string, openBrowser bool, model Model, hooks *InteractiveHooks) error {
	mux := http.NewServeMux()
	srv := &http.Server{Addr: addr, Handler: mux}
	mux.HandleFunc("/api/model", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_ = json.NewEncoder(w).Encode(model)
	})
	mux.HandleFunc("/api/source-values", func(w http.ResponseWriter, r *http.Request) {
		if hooks == nil || !hooks.Enabled {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_ = json.NewEncoder(w).Encode(map[string]any{
			"valuesYAML":         hooks.SourceValuesYAML,
			"suggestedSavePath":  hooks.SuggestedSavePath,
			"interactiveEnabled": true,
		})
	})
	mux.HandleFunc("/api/experiment/render", func(w http.ResponseWriter, r *http.Request) {
		if hooks == nil || !hooks.Enabled || hooks.RunExperiment == nil {
			http.NotFound(w, r)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		var req struct {
			ValuesYAML string `json:"valuesYAML"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		resp, err := hooks.RunExperiment(req.ValuesYAML)
		if err != nil {
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			w.WriteHeader(http.StatusBadRequest)
			_ = json.NewEncoder(w).Encode(map[string]any{"error": err.Error()})
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_ = json.NewEncoder(w).Encode(resp)
	})
	mux.HandleFunc("/api/experiment/save", func(w http.ResponseWriter, r *http.Request) {
		if hooks == nil || !hooks.Enabled || hooks.SaveValues == nil {
			http.NotFound(w, r)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		var req struct {
			Path       string `json:"path"`
			ValuesYAML string `json:"valuesYAML"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		if err := hooks.SaveValues(req.Path, req.ValuesYAML); err != nil {
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			w.WriteHeader(http.StatusBadRequest)
			_ = json.NewEncoder(w).Encode(map[string]any{"error": err.Error()})
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "path": req.Path})
	})
	mux.HandleFunc("/api/experiment/compare", func(w http.ResponseWriter, r *http.Request) {
		if hooks == nil || !hooks.Enabled || hooks.CompareEntities == nil {
			http.NotFound(w, r)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		var req struct {
			ValuesYAML string `json:"valuesYAML"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		resp, err := hooks.CompareEntities(req.ValuesYAML)
		if err != nil {
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			w.WriteHeader(http.StatusBadRequest)
			_ = json.NewEncoder(w).Encode(map[string]any{"error": err.Error()})
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_ = json.NewEncoder(w).Encode(resp)
	})
	mux.HandleFunc("/api/model.yaml", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/yaml; charset=utf-8")
		b, err := yaml.Marshal(model)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		_, _ = w.Write(b)
	})
	mux.HandleFunc("/api/shutdown", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_, _ = w.Write([]byte(`{"ok":true}`))
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			_ = srv.Shutdown(ctx)
		}()
	})
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_ = pageTemplate.Execute(w, map[string]any{"Addr": addr})
	})
	mux.HandleFunc("/preview", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_ = previewTemplate.Execute(w, map[string]any{"Addr": addr})
	})
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return err
	}
	uiURL := "http://" + ln.Addr().String()
	previewURL := uiURL + "/preview"
	fmt.Printf("Render Preview: %s\n", previewURL)
	fmt.Printf("Inspect UI (advanced): %s\n", uiURL)
	if openBrowser {
		go func() {
			// Delay a bit so the server starts accepting connections first.
			time.Sleep(150 * time.Millisecond)
			if err := openURL(previewURL); err != nil {
				fmt.Printf("Could not open browser automatically: %v\n", err)
			}
		}()
	}
	err = srv.Serve(ln)
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

func DiffModels(base, next Model) ExperimentDiff {
	baseByID := map[string]Resource{}
	nextByID := map[string]Resource{}
	for _, r := range base.Resources {
		baseByID[r.ID] = r
	}
	for _, r := range next.Resources {
		nextByID[r.ID] = r
	}
	var out ExperimentDiff
	for id, r := range nextByID {
		if br, ok := baseByID[id]; !ok {
			out.Added = append(out.Added, shortResourceLabel(r))
		} else if !resourceEqual(br, r) {
			out.Changed = append(out.Changed, shortResourceLabel(r))
		}
	}
	for id, r := range baseByID {
		if _, ok := nextByID[id]; !ok {
			out.Removed = append(out.Removed, shortResourceLabel(r))
		}
	}
	sort.Strings(out.Added)
	sort.Strings(out.Changed)
	sort.Strings(out.Removed)
	return out
}

func resourceEqual(a, b Resource) bool {
	ab, _ := json.Marshal(a.Raw)
	bb, _ := json.Marshal(b.Raw)
	return bytesEqual(ab, bb)
}

func bytesEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func shortResourceLabel(r Resource) string {
	if strings.TrimSpace(r.Namespace) != "" {
		return fmt.Sprintf("%s/%s (%s)", r.Namespace, r.Name, r.Kind)
	}
	return fmt.Sprintf("%s (%s)", r.Name, r.Kind)
}

func openURL(url string) error {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "darwin":
		cmd = exec.Command("open", url)
	case "linux":
		cmd = exec.Command("xdg-open", url)
	case "windows":
		cmd = exec.Command("cmd", "/c", "start", "", url)
	default:
		return fmt.Errorf("unsupported platform %q", runtime.GOOS)
	}
	return cmd.Start()
}

func resourceFromDoc(d map[string]any) Resource {
	meta, _ := d["metadata"].(map[string]any)
	name, _ := meta["name"].(string)
	ns, _ := meta["namespace"].(string)
	kind, _ := d["kind"].(string)
	apiVersion, _ := d["apiVersion"].(string)
	id := strings.Join([]string{apiVersion, kind, ns, name}, "/")
	return Resource{
		ID:          id,
		Kind:        kind,
		APIVersion:  apiVersion,
		Name:        name,
		Namespace:   ns,
		Labels:      mapAny(meta["labels"]),
		Spec:        d["spec"],
		Data:        firstNonNil(d["data"], d["stringData"]),
		Raw:         d,
		RawYAML:     yamlString(d),
		SummaryYAML: yamlString(buildSummaryMap(d, meta)),
	}
}

func buildRelations(resources []Resource) []Relation {
	var rels []Relation
	resBySvc := map[string]Resource{}
	workloads := []Resource{}
	for _, r := range resources {
		if r.Kind == "Service" {
			resBySvc[nsNameKey(r.Namespace, r.Name)] = r
		}
		if isWorkloadKind(r.Kind) {
			workloads = append(workloads, r)
		}
	}
	// Ingress -> Service
	for _, r := range resources {
		if r.Kind != "Ingress" {
			continue
		}
		for _, svc := range ingressServiceNames(r.Raw) {
			if to, ok := resBySvc[nsNameKey(r.Namespace, svc)]; ok {
				rels = append(rels, Relation{From: r.ID, To: to.ID, Type: "routes-to-service", Detail: svc})
			}
		}
	}
	// Service -> Workload by selector labels
	for _, svc := range resources {
		if svc.Kind != "Service" {
			continue
		}
		sel := serviceSelector(svc.Raw)
		if len(sel) == 0 {
			continue
		}
		for _, w := range workloads {
			if svc.Namespace != w.Namespace {
				continue
			}
			if labelsMatch(sel, workloadPodLabels(w.Raw)) {
				rels = append(rels, Relation{From: svc.ID, To: w.ID, Type: "selects-workload"})
			}
		}
	}
	// Workload -> ConfigMap/Secret refs
	resIndex := map[string]Resource{}
	for _, r := range resources {
		resIndex[nsNameKey(r.Namespace, r.Name)] = r
	}
	for _, w := range workloads {
		for _, ref := range workloadRefs(w.Raw) {
			if to, ok := resIndex[nsNameKey(w.Namespace, ref.Name)]; ok && to.Kind == ref.Kind {
				rels = append(rels, Relation{From: w.ID, To: to.ID, Type: ref.RelType, Detail: ref.Path})
			}
		}
	}
	sort.Slice(rels, func(i, j int) bool {
		a := rels[i].From + "|" + rels[i].Type + "|" + rels[i].To + "|" + rels[i].Detail
		b := rels[j].From + "|" + rels[j].Type + "|" + rels[j].To + "|" + rels[j].Detail
		return a < b
	})
	return rels
}

func buildApplications(resources []Resource, relations []Relation) []Application {
	resByID := map[string]Resource{}
	for _, r := range resources {
		resByID[r.ID] = r
	}
	assigned := map[string]string{}
	apps := []Application{}
	appByWorkload := map[string]*Application{}

	for _, r := range resources {
		if !isWorkloadKind(r.Kind) {
			continue
		}
		app := Application{
			ID:           "app:" + r.ID,
			Name:         r.Name,
			Namespace:    r.Namespace,
			WorkloadID:   r.ID,
			WorkloadKind: r.Kind,
			WorkloadName: r.Name,
			TypeHint:     workloadTypeHint(r.Kind),
			ResourceIDs:  []string{r.ID},
		}
		apps = append(apps, app)
		appByWorkload[r.ID] = &apps[len(apps)-1]
		assigned[r.ID] = app.ID
	}

	// Service -> workload
	for _, rel := range relations {
		if rel.Type != "selects-workload" {
			continue
		}
		app := appByWorkload[rel.To]
		if app == nil {
			continue
		}
		if _, ok := resByID[rel.From]; !ok {
			continue
		}
		addResourceToApp(app, rel.From, "service")
		assigned[rel.From] = app.ID
	}
	// Ingress -> service -> workload app
	serviceToApp := map[string]*Application{}
	for _, a := range apps {
		for _, svc := range a.ServiceIDs {
			serviceToApp[svc] = appByWorkload[a.WorkloadID]
		}
	}
	for _, rel := range relations {
		if rel.Type != "routes-to-service" {
			continue
		}
		if app := serviceToApp[rel.To]; app != nil {
			addResourceToApp(app, rel.From, "ingress")
			assigned[rel.From] = app.ID
		}
	}
	// Workload -> config/secret refs
	for _, rel := range relations {
		app := appByWorkload[rel.From]
		if app == nil {
			continue
		}
		r, ok := resByID[rel.To]
		if !ok {
			continue
		}
		switch r.Kind {
		case "ConfigMap":
			addResourceToApp(app, r.ID, "configmap")
			assigned[r.ID] = app.ID
		case "Secret":
			addResourceToApp(app, r.ID, "secret")
			assigned[r.ID] = app.ID
		}
	}
	// ServiceAccount via workload spec.template.spec.serviceAccountName
	nameIndex := map[string]Resource{}
	for _, r := range resources {
		nameIndex[nsNameKey(r.Namespace, r.Name)] = r
	}
	for i := range apps {
		a := &apps[i]
		if a.WorkloadID == "" {
			continue
		}
		w := resByID[a.WorkloadID]
		if sa := workloadServiceAccountName(w.Raw); sa != "" {
			if saRes, ok := nameIndex[nsNameKey(a.Namespace, sa)]; ok && saRes.Kind == "ServiceAccount" {
				addResourceToApp(a, saRes.ID, "other")
				assigned[saRes.ID] = a.ID
			}
		}
	}

	// standalone/orphan groups for unassigned resources
	for _, r := range resources {
		if _, ok := assigned[r.ID]; ok {
			continue
		}
		app := Application{
			ID:          "standalone:" + r.ID,
			Name:        r.Name,
			Namespace:   r.Namespace,
			TypeHint:    "standalone",
			ResourceIDs: []string{r.ID},
		}
		switch r.Kind {
		case "Service":
			app.ServiceIDs = []string{r.ID}
		case "Ingress":
			app.IngressIDs = []string{r.ID}
		case "ConfigMap":
			app.ConfigMapIDs = []string{r.ID}
		case "Secret":
			app.SecretIDs = []string{r.ID}
		default:
			app.OtherIDs = []string{r.ID}
		}
		apps = append(apps, app)
	}

	for i := range apps {
		sort.Strings(apps[i].ResourceIDs)
		sort.Strings(apps[i].ServiceIDs)
		sort.Strings(apps[i].IngressIDs)
		sort.Strings(apps[i].ConfigMapIDs)
		sort.Strings(apps[i].SecretIDs)
		sort.Strings(apps[i].OtherIDs)
		apps[i].ImportPlan = buildImportPlan(apps[i], resByID)
	}
	sort.Slice(apps, func(i, j int) bool {
		if apps[i].Namespace != apps[j].Namespace {
			return apps[i].Namespace < apps[j].Namespace
		}
		if apps[i].TypeHint != apps[j].TypeHint {
			return apps[i].TypeHint < apps[j].TypeHint
		}
		return apps[i].Name < apps[j].Name
	})
	return apps
}

func addResourceToApp(app *Application, resourceID, bucket string) {
	if app == nil {
		return
	}
	if !containsString(app.ResourceIDs, resourceID) {
		app.ResourceIDs = append(app.ResourceIDs, resourceID)
	}
	switch bucket {
	case "service":
		if !containsString(app.ServiceIDs, resourceID) {
			app.ServiceIDs = append(app.ServiceIDs, resourceID)
		}
	case "ingress":
		if !containsString(app.IngressIDs, resourceID) {
			app.IngressIDs = append(app.IngressIDs, resourceID)
		}
	case "configmap":
		if !containsString(app.ConfigMapIDs, resourceID) {
			app.ConfigMapIDs = append(app.ConfigMapIDs, resourceID)
		}
	case "secret":
		if !containsString(app.SecretIDs, resourceID) {
			app.SecretIDs = append(app.SecretIDs, resourceID)
		}
	default:
		if !containsString(app.OtherIDs, resourceID) {
			app.OtherIDs = append(app.OtherIDs, resourceID)
		}
	}
}

func buildImportPlan(app Application, resByID map[string]Resource) []PlanItem {
	out := []PlanItem{}
	for _, rid := range app.ResourceIDs {
		r := resByID[rid]
		target, mode, reason := importTargetHint(r)
		out = append(out, PlanItem{ResourceID: rid, Target: target, Mode: mode, Reason: reason})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].ResourceID < out[j].ResourceID })
	return out
}

func importTargetHint(r Resource) (target, mode, reason string) {
	switch r.Kind {
	case "Deployment":
		return "apps-stateless.<app>", "helper-candidate", "workload kind"
	case "StatefulSet":
		return "apps-stateful.<app>", "helper-candidate", "workload kind"
	case "Job":
		return "apps-jobs.<app>", "helper-candidate", "workload kind"
	case "CronJob":
		return "apps-cronjobs.<app>", "helper-candidate", "workload kind"
	case "Service":
		return "apps-services.<name>", "helper-candidate", "service resource"
	case "Ingress":
		return "apps-ingresses.<name>", "helper-candidate", "ingress resource"
	case "ConfigMap":
		return "apps-configmaps.<name>", "helper-candidate", "configmap resource"
	case "Secret":
		return "apps-secrets.<name>", "helper-candidate", "secret resource"
	default:
		return "imported-manifests.<name>", "raw-fallback", "unsupported/unknown specialized mapping"
	}
}

func workloadTypeHint(kind string) string {
	switch kind {
	case "StatefulSet":
		return "stateful"
	case "Deployment", "DaemonSet":
		return "stateless"
	case "Job", "CronJob":
		return "job"
	default:
		return "workload"
	}
}

type refHit struct {
	Kind    string
	Name    string
	RelType string
	Path    string
}

func workloadRefs(doc map[string]any) []refHit {
	var hits []refHit
	spec := mapAny(doc["spec"])
	tpl := mapAny(spec["template"])
	podSpec := mapAny(mapAny(tpl["spec"]))
	vols, _ := podSpec["volumes"].([]any)
	for i, v := range vols {
		vm := mapAny(v)
		if cm := mapAny(vm["configMap"]); len(cm) > 0 {
			if n, _ := cm["name"].(string); n != "" {
				hits = append(hits, refHit{Kind: "ConfigMap", Name: n, RelType: "mounts-configmap", Path: fmt.Sprintf("spec.template.spec.volumes[%d].configMap.name", i)})
			}
		}
		if s := mapAny(vm["secret"]); len(s) > 0 {
			if n, _ := s["secretName"].(string); n != "" {
				hits = append(hits, refHit{Kind: "Secret", Name: n, RelType: "mounts-secret", Path: fmt.Sprintf("spec.template.spec.volumes[%d].secret.secretName", i)})
			}
		}
	}
	for ci, c := range append(containersFromPodSpec(podSpec, "initContainers"), containersFromPodSpec(podSpec, "containers")...) {
		cm := mapAny(c)
		if envFrom, ok := cm["envFrom"].([]any); ok {
			for ei, e := range envFrom {
				em := mapAny(e)
				if cmr := mapAny(em["configMapRef"]); len(cmr) > 0 {
					if n, _ := cmr["name"].(string); n != "" {
						hits = append(hits, refHit{Kind: "ConfigMap", Name: n, RelType: "envfrom-configmap", Path: fmt.Sprintf("container[%d].envFrom[%d].configMapRef.name", ci, ei)})
					}
				}
				if sr := mapAny(em["secretRef"]); len(sr) > 0 {
					if n, _ := sr["name"].(string); n != "" {
						hits = append(hits, refHit{Kind: "Secret", Name: n, RelType: "envfrom-secret", Path: fmt.Sprintf("container[%d].envFrom[%d].secretRef.name", ci, ei)})
					}
				}
			}
		}
	}
	return hits
}

func ingressServiceNames(doc map[string]any) []string {
	out := []string{}
	spec := mapAny(doc["spec"])
	if db := mapAny(spec["defaultBackend"]); len(db) > 0 {
		if svc := mapAny(db["service"]); len(svc) > 0 {
			if n, _ := svc["name"].(string); n != "" {
				out = append(out, n)
			}
		}
	}
	if rules, ok := spec["rules"].([]any); ok {
		for _, r := range rules {
			rm := mapAny(r)
			httpm := mapAny(rm["http"])
			paths, _ := httpm["paths"].([]any)
			for _, p := range paths {
				pm := mapAny(p)
				backend := mapAny(pm["backend"])
				svc := mapAny(backend["service"])
				if n, _ := svc["name"].(string); n != "" {
					out = append(out, n)
				}
			}
		}
	}
	return uniqStrings(out)
}

func serviceSelector(doc map[string]any) map[string]string {
	spec := mapAny(doc["spec"])
	sel := mapAny(spec["selector"])
	out := map[string]string{}
	for k, v := range sel {
		if s, ok := v.(string); ok {
			out[k] = s
		}
	}
	return out
}

func workloadPodLabels(doc map[string]any) map[string]string {
	spec := mapAny(doc["spec"])
	tpl := mapAny(spec["template"])
	meta := mapAny(tpl["metadata"])
	labels := mapAny(meta["labels"])
	out := map[string]string{}
	for k, v := range labels {
		if s, ok := v.(string); ok {
			out[k] = s
		}
	}
	return out
}

func labelsMatch(sel, labels map[string]string) bool {
	if len(sel) == 0 {
		return false
	}
	for k, v := range sel {
		if labels[k] != v {
			return false
		}
	}
	return true
}

func isWorkloadKind(kind string) bool {
	switch kind {
	case "Deployment", "StatefulSet", "DaemonSet", "Job", "CronJob":
		return true
	default:
		return false
	}
}

func containersFromPodSpec(podSpec map[string]any, key string) []any {
	arr, _ := podSpec[key].([]any)
	return arr
}

func workloadServiceAccountName(doc map[string]any) string {
	spec := mapAny(doc["spec"])
	tpl := mapAny(spec["template"])
	podSpec := mapAny(tpl["spec"])
	if s, _ := podSpec["serviceAccountName"].(string); strings.TrimSpace(s) != "" {
		return strings.TrimSpace(s)
	}
	if s, _ := podSpec["serviceAccount"].(string); strings.TrimSpace(s) != "" {
		return strings.TrimSpace(s)
	}
	return ""
}

func nsNameKey(ns, name string) string { return ns + "/" + name }
func firstNonNil(vals ...any) any {
	for _, v := range vals {
		if v != nil {
			return v
		}
	}
	return nil
}
func mapAny(v any) map[string]any {
	m, _ := v.(map[string]any)
	if m == nil {
		return map[string]any{}
	}
	return m
}
func uniqStrings(in []string) []string {
	set := map[string]struct{}{}
	out := []string{}
	for _, s := range in {
		if _, ok := set[s]; ok {
			continue
		}
		set[s] = struct{}{}
		out = append(out, s)
	}
	return out
}

func containsString(in []string, needle string) bool {
	for _, s := range in {
		if s == needle {
			return true
		}
	}
	return false
}

func buildSummaryMap(doc map[string]any, meta map[string]any) map[string]any {
	out := map[string]any{}
	if labels := mapAny(meta["labels"]); len(labels) > 0 {
		out["labels"] = labels
	}
	if spec, ok := doc["spec"]; ok && spec != nil {
		out["spec"] = spec
	}
	if data := firstNonNil(doc["data"], doc["stringData"]); data != nil {
		out["data"] = data
	}
	return out
}

func yamlString(v any) string {
	if m, ok := v.(map[string]any); ok && len(m) == 0 {
		return "# no spec/data/labels summary\n"
	}
	b, err := yaml.Marshal(v)
	if err != nil {
		return fmt.Sprintf("yaml error: %v", err)
	}
	return string(b)
}
