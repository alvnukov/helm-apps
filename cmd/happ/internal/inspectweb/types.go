package inspectweb

import (
	"github.com/zol/helm-apps/cmd/happ/internal/analyze"
	"github.com/zol/helm-apps/cmd/happ/internal/verify"
)

type Model struct {
	Resources    []Resource       `json:"resources"`
	Relations    []Relation       `json:"relations"`
	Applications []Application    `json:"applications"`
	Analysis     *analyze.Report  `json:"analysis,omitempty"`
	HelmApps     *HelmAppsPreview `json:"helmApps,omitempty"`
	Summary      Summary          `json:"summary"`
}

type Summary struct {
	ResourceCount    int `json:"resourceCount"`
	RelationCount    int `json:"relationCount"`
	ApplicationCount int `json:"applicationCount"`
}

type Resource struct {
	ID          string         `json:"id"`
	Kind        string         `json:"kind"`
	APIVersion  string         `json:"apiVersion"`
	Name        string         `json:"name"`
	Namespace   string         `json:"namespace,omitempty"`
	Labels      map[string]any `json:"labels,omitempty"`
	Spec        any            `json:"spec,omitempty"`
	Data        any            `json:"data,omitempty"`
	Raw         map[string]any `json:"raw"`
	RawYAML     string         `json:"rawYAML,omitempty"`
	SummaryYAML string         `json:"summaryYAML,omitempty"`
}

type Relation struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Type   string `json:"type"`
	Detail string `json:"detail,omitempty"`
}

type Application struct {
	ID           string     `json:"id"`
	Name         string     `json:"name"`
	Namespace    string     `json:"namespace,omitempty"`
	WorkloadID   string     `json:"workloadId,omitempty"`
	WorkloadKind string     `json:"workloadKind,omitempty"`
	WorkloadName string     `json:"workloadName,omitempty"`
	TypeHint     string     `json:"typeHint,omitempty"`
	ResourceIDs  []string   `json:"resourceIds"`
	ServiceIDs   []string   `json:"serviceIds,omitempty"`
	IngressIDs   []string   `json:"ingressIds,omitempty"`
	ConfigMapIDs []string   `json:"configMapIds,omitempty"`
	SecretIDs    []string   `json:"secretIds,omitempty"`
	OtherIDs     []string   `json:"otherIds,omitempty"`
	ImportPlan   []PlanItem `json:"importPlan,omitempty"`
}

type PlanItem struct {
	ResourceID string `json:"resourceId"`
	Target     string `json:"target"`
	Mode       string `json:"mode"`
	Reason     string `json:"reason,omitempty"`
}

type HelmAppsPreview struct {
	Strategy   string `json:"strategy"`
	GroupName  string `json:"groupName,omitempty"`
	GroupType  string `json:"groupType,omitempty"`
	ValuesYAML string `json:"valuesYAML,omitempty"`
	Error      string `json:"error,omitempty"`
}

type InteractiveHooks struct {
	Enabled           bool
	SourceValuesYAML  string
	SuggestedSavePath string
	RunExperiment     func(valuesYAML string) (ExperimentResponse, error)
	CompareEntities   func(valuesYAML string) (CompareEntitiesResponse, error)
	SaveValues        func(path, valuesYAML string) error
}

type ExperimentDiff struct {
	Added   []string `json:"added,omitempty"`
	Removed []string `json:"removed,omitempty"`
	Changed []string `json:"changed,omitempty"`
}

type ExperimentResponse struct {
	Model Model          `json:"model"`
	Diff  ExperimentDiff `json:"diff"`
}

type CompareEntitiesResponse struct {
	Compare            verify.DetailedResult `json:"compare"`
	SourceYAMLByKey    map[string]string     `json:"sourceYAMLByKey,omitempty"`
	GeneratedYAMLByKey map[string]string     `json:"generatedYAMLByKey,omitempty"`
	DyffByKey          map[string]string     `json:"dyffByKey,omitempty"`
	DyffAvailable      bool                  `json:"dyffAvailable,omitempty"`
}
