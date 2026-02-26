package inspectweb

import (
	"context"
	"encoding/json"
	"errors"
	"html/template"
	"net"
	"net/http"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/composeinspect"
	"gopkg.in/yaml.v3"
)

type composeWebPayload struct {
	Report      composeinspect.Report `json:"report"`
	SourceYAML  string                `json:"sourceYAML,omitempty"`
	PreviewYAML string                `json:"previewYAML,omitempty"`
	PreviewErr  string                `json:"previewError,omitempty"`
}

func ServeCompose(addr string, openBrowser bool, rep composeinspect.Report, sourceYAML, previewYAML, previewErr string) error {
	payload := composeWebPayload{Report: rep, SourceYAML: sourceYAML, PreviewYAML: previewYAML, PreviewErr: previewErr}
	mux := http.NewServeMux()
	srv := &http.Server{Addr: addr, Handler: mux}

	mux.HandleFunc("/api/compose-report", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		_ = json.NewEncoder(w).Encode(payload)
	})
	mux.HandleFunc("/api/compose-report.yaml", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/yaml; charset=utf-8")
		b, err := yaml.Marshal(payload)
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
		_ = composeTemplate.Execute(w, map[string]any{"Addr": addr})
	})

	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return err
	}
	uiURL := "http://" + ln.Addr().String()
	println("Compose Inspect UI: " + uiURL)
	if openBrowser {
		go func() {
			time.Sleep(150 * time.Millisecond)
			_ = openURL(uiURL)
		}()
	}
	err = srv.Serve(ln)
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

var composeTemplate = template.Must(template.New("compose-web").Parse(`<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Compose Inspect</title>
<style>
body{margin:0;font:14px/1.4 ui-sans-serif,system-ui,sans-serif;background:#f4f6f8;color:#1f2937}
.top{padding:12px 16px;background:#0f172a;color:#fff;display:flex;gap:10px;align-items:center;justify-content:space-between;position:sticky;top:0;z-index:5}
.top .left{display:flex;gap:10px;align-items:center;flex-wrap:wrap}
.top a{color:#93c5fd;text-decoration:none}
.wrap{padding:14px;max-width:1700px;margin:0 auto}
.grid{display:grid;grid-template-columns:minmax(520px,1fr) minmax(520px,1fr);gap:12px;align-items:start}
.grid>div{min-width:0}
.card{background:#fff;border:1px solid #e5e7eb;border-radius:12px;padding:12px 14px;margin-bottom:12px;overflow:hidden}
.muted{color:#64748b}
.row{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
button{border:none;border-radius:8px;padding:7px 12px;cursor:pointer;font-weight:600;background:#e2e8f0;color:#0f172a}
.btn-primary{background:#2563eb;color:#fff}
.btn-danger{background:#ef4444;color:#fff}
.chips{display:flex;gap:6px;flex-wrap:wrap}
.chip{font-size:12px;background:#eef2f7;color:#334155;border-radius:999px;padding:2px 8px}
.tbl-wrap{max-height:340px;overflow:auto;border:1px solid #e5e7eb;border-radius:10px}
table{width:100%;border-collapse:collapse;font-size:12px}
th,td{border-bottom:1px solid #e5e7eb;padding:6px 8px;text-align:left;vertical-align:top}
th{position:sticky;top:0;background:#fff;z-index:1}
tr:hover td{background:#f8fafc}
tr.sel td{background:#eff6ff}
pre{background:#0b1020;color:#dbeafe;padding:12px;border-radius:10px;overflow:auto;max-width:100%;margin:0}
.yaml{white-space:pre;word-break:normal}
.k{color:#93c5fd}.s{color:#fcd34d}.n{color:#86efac}.c{color:#94a3b8;font-style:italic}
.split{display:grid;grid-template-columns:1fr 1fr;gap:10px}
.yaml-tools{display:flex;gap:8px;align-items:center;justify-content:space-between;flex-wrap:wrap;margin:8px 0}
@media (max-width: 1100px){.grid,.split{grid-template-columns:1fr}}
</style></head>
<body>
<div class="top">
  <div class="left">
    <strong>Compose Inspect</strong>
    <span class="muted" style="color:#cbd5e1">Graph + helm-apps preview</span>
    <a href="/api/compose-report.yaml" target="_blank">/api/compose-report.yaml</a>
  </div>
  <button id="exitBtn" class="btn-danger">Exit</button>
</div>
<div class="wrap">
  <div class="grid">
    <div>
      <div class="card">
        <div style="font-weight:700">Compose Summary</div>
        <div id="summary" class="muted" style="margin-top:6px">Loading…</div>
        <div id="warnings" style="margin-top:8px"></div>
      </div>
      <div class="card">
        <div class="row" style="justify-content:space-between">
          <div>
            <div style="font-weight:700">Services</div>
            <div class="muted">Dependencies, ports, env refs, role hints.</div>
          </div>
          <input id="svcSearch" placeholder="Search services" style="padding:8px 10px;border:1px solid #cbd5e1;border-radius:8px;min-width:220px">
        </div>
        <div id="servicesBox" style="margin-top:8px"></div>
      </div>
      <div class="split">
        <div class="card">
          <div style="font-weight:700">Relations</div>
          <div id="relationsBox" style="margin-top:8px"></div>
        </div>
        <div class="card">
          <div style="font-weight:700">Resources</div>
          <div id="resourcesBox" style="margin-top:8px"></div>
        </div>
      </div>
    </div>
    <div>
      <div class="card">
        <div style="font-weight:700">Selected Service</div>
        <div id="serviceDetails" class="muted" style="margin-top:8px">Select a service</div>
      </div>
      <div class="card">
        <div class="row" style="justify-content:space-between">
          <div>
            <div style="font-weight:700">Source docker-compose.yml</div>
            <div class="muted">Read-only source with YAML syntax highlight.</div>
          </div>
          <button id="wrapComposeBtn">Wrap YAML: Off</button>
        </div>
        <div id="composeMeta" class="muted" style="margin:8px 0"></div>
        <div id="composeBox"></div>
      </div>
      <div class="card">
        <div class="row" style="justify-content:space-between">
          <div>
            <div style="font-weight:700">Generated helm-apps values.yaml</div>
            <div class="muted">Compose import MVP preview (app-centric).</div>
          </div>
          <button id="wrapBtn">Wrap YAML: Off</button>
        </div>
        <div id="previewMeta" class="muted" style="margin:8px 0"></div>
        <div id="previewBox"></div>
      </div>
    </div>
  </div>
</div>
<script>
const byId=id=>document.getElementById(id);
let payload=null, selectedServiceId='', svcSearch='', wrapYAML=false;
let wrapComposeYAML=false;
function esc(s){return String(s??'').replace(/[&<>]/g,m=>({ '&':'&amp;','<':'&lt;','>':'&gt;' }[m]));}
function escAttr(s){return esc(s).replace(/"/g,'&quot;').replace(/'/g,'&#39;');}
function yamlHtml(s){
  const lines = String(s||'').split('\n');
  return lines.map(line=>{
    const e = esc(line);
    if(!e) return '';
    if(/^\s*#/.test(e)) return '<span class="c">'+e+'</span>';
    const m = e.match(/^(\s*[^:#][^:]*:)(\s*)(.*)$/);
    if(!m) return e;
    let v = m[3];
    if(/^['"].*['"]$/.test(v)) v = '<span class="s">'+v+'</span>';
    else if(/^(true|false|null|[-]?\d+(\.\d+)?)$/.test(v)) v = '<span class="n">'+v+'</span>';
    return '<span class="k">'+m[1]+'</span>'+m[2]+v;
  }).join('\n');
}
function fetchJSON(url, ms){
  const c = new AbortController();
  const t = setTimeout(()=>c.abort(), ms||10000);
  return fetch(url, {signal:c.signal}).then(r=>r.json()).finally(()=>clearTimeout(t));
}
function render(){
  if(!payload) return;
  const rep = payload.report || {};
  byId('summary').innerHTML =
    '<code>'+esc(rep.sourcePath||'')+'</code><br>'+
    'project: <strong>'+esc(rep.project||'-')+'</strong> · '+
    'services: <strong>'+esc((rep.summary&&rep.summary.serviceCount)||0)+'</strong> · '+
    'resources: <strong>'+esc((rep.summary&&rep.summary.resourceCount)||0)+'</strong> · '+
    'relations: <strong>'+esc((rep.summary&&rep.summary.relationCount)||0)+'</strong>';
  const warns = Array.isArray(rep.warnings)?rep.warnings:[];
  byId('warnings').innerHTML = warns.length
    ? '<details><summary>Warnings ('+warns.length+')</summary><div class="chips" style="margin-top:6px">'+warns.map(w=>'<span class="chip" style="background:#fee2e2;color:#991b1b">'+esc(w)+'</span>').join('')+'</div></details>'
    : '<div class="muted">No warnings</div>';

  renderServices();
  renderRelations();
  renderResources();
  renderPreview();
}
function filteredServices(){
  const rep = payload.report || {};
  let svcs = Array.isArray(rep.services) ? rep.services.slice() : [];
  const q = (svcSearch||'').trim().toLowerCase();
  if(q){
    svcs = svcs.filter(s=>{
      const h = [s.name, s.image, ...(s.roleHints||[]), ...(s.dependsOn||[])].join('\n').toLowerCase();
      return h.includes(q);
    });
  }
  return svcs;
}
function renderServices(){
  const svcs = filteredServices();
  const rows = svcs.map(s=>{
    const sel = selectedServiceId===s.id ? ' sel' : '';
    return '<tr class="'+sel+'" data-svc-id="'+escAttr(s.id)+'">'+
      '<td><strong>'+esc(s.name)+'</strong><div class="muted">'+esc(s.image||'(no image)')+'</div></td>'+
      '<td>'+esc((s.dependsOn||[]).length)+'</td>'+
      '<td>'+esc((s.portsPublished||[]).length + (s.expose||[]).length)+'</td>'+
      '<td>'+esc((s.volumes||[]).length)+'</td>'+
      '<td>'+(s.roleHints||[]).map(x=>'<span class="chip">'+esc(x)+'</span>').join(' ')+'</td>'+
    '</tr>';
  }).join('');
  byId('servicesBox').innerHTML = rows
    ? '<div class="tbl-wrap"><table><tr><th>Service</th><th>Deps</th><th>Ports</th><th>Mounts</th><th>Hints</th></tr>'+rows+'</table></div>'
    : '<div class="muted">No services for current filter</div>';

  const all = Array.isArray((payload.report||{}).services) ? payload.report.services : [];
  const selected = all.find(s=>s.id===selectedServiceId) || svcs[0] || all[0] || null;
  selectedServiceId = selected ? selected.id : '';
  renderServiceDetails(selected);
}
function renderServiceDetails(s){
  if(!s){ byId('serviceDetails').innerHTML='<div class="muted">Select a service</div>'; return; }
  const kv = (obj)=> {
    const keys = Object.keys(obj||{}).sort();
    if(!keys.length) return '<div class="muted">None</div>';
    return '<div class="tbl-wrap"><table>'+keys.map(k=>'<tr><td><code>'+esc(k)+'</code></td><td>'+esc(obj[k])+'</td></tr>').join('')+'</table></div>';
  };
  byId('serviceDetails').innerHTML =
    '<div class="row"><span class="chip">'+esc(s.name)+'</span>'+(s.roleHints||[]).map(x=>'<span class="chip">'+esc(x)+'</span>').join('')+'</div>'+
    '<div style="margin-top:8px"><strong>Image</strong><div class="muted"><code>'+esc(s.image||'')+'</code>'+(s.hasBuild?' · build: true':'')+'</div></div>'+
    '<details><summary>Command / Entrypoint</summary>'+
      '<div class="row" style="margin-top:6px"><span class="muted">entrypoint:</span>' + ((s.entrypoint&&s.entrypoint.length)?('<code>'+esc(JSON.stringify(s.entrypoint))+'</code>'):(s.entrypointShell?('<code>'+esc(s.entrypointShell)+'</code>'):'<span class="muted">none</span>')) + '</div>'+
      '<div class="row" style="margin-top:6px"><span class="muted">command:</span>' + ((s.command&&s.command.length)?('<code>'+esc(JSON.stringify(s.command))+'</code>'):(s.commandShell?('<code>'+esc(s.commandShell)+'</code>'):'<span class="muted">none</span>')) + '</div>'+
      '<div class="row" style="margin-top:6px"><span class="muted">working_dir:</span>' + (s.workingDir?('<code>'+esc(s.workingDir)+'</code>'):'<span class="muted">none</span>') + '</div>'+
    '</details>'+
    '<details><summary>Healthcheck</summary>'+
      ((s.healthcheck && !s.healthcheck.disable)
        ? '<div class="row" style="margin-top:6px"><span class="muted">test:</span>' + ((s.healthcheck.test&&s.healthcheck.test.length)?('<code>'+esc(JSON.stringify(s.healthcheck.test))+'</code>'):(s.healthcheck.testShell?('<code>'+esc(s.healthcheck.testShell)+'</code>'):'<span class="muted">none</span>')) + '</div>'+
          '<div class="row" style="margin-top:6px"><span class="muted">timings:</span><span class="chip">interval '+esc(s.healthcheck.intervalSeconds||0)+'s</span><span class="chip">timeout '+esc(s.healthcheck.timeoutSeconds||0)+'s</span><span class="chip">start '+esc(s.healthcheck.startPeriodSeconds||0)+'s</span><span class="chip">retries '+esc(s.healthcheck.retries||0)+'</span></div>'
        : '<div class="muted">No healthcheck</div>')+
    '</details>'+
    '<details open><summary>Dependencies / Networks / Profiles</summary>'+
      '<div class="row" style="margin-top:6px"><span class="muted">depends_on:</span>'+((s.dependsOn||[]).length?(s.dependsOn||[]).map(x=>'<span class="chip">'+esc(x)+'</span>').join(' '):'<span class="muted">none</span>')+'</div>'+
      '<div class="row" style="margin-top:6px"><span class="muted">networks:</span>'+((s.networks||[]).length?(s.networks||[]).map(x=>'<span class="chip">'+esc(x)+'</span>').join(' '):'<span class="muted">none</span>')+'</div>'+
      '<div class="row" style="margin-top:6px"><span class="muted">profiles:</span>'+((s.profiles||[]).length?(s.profiles||[]).map(x=>'<span class="chip">'+esc(x)+'</span>').join(' '):'<span class="muted">none</span>')+'</div>'+
    '</details>'+
    '<details open><summary>Ports</summary>'+
      '<div class="tbl-wrap"><table><tr><th>Published</th><th>Target</th><th>Protocol</th><th>Raw</th></tr>'+
        (s.portsPublished||[]).map(p=>'<tr><td>'+esc(p.published||'')+'</td><td>'+esc(p.target||'')+'</td><td>'+esc(p.protocol||'tcp')+'</td><td><code>'+esc(p.raw||'')+'</code></td></tr>').join('')+
      '</table></div>'+
      '<div class="row" style="margin-top:6px"><span class="muted">expose:</span>'+((s.expose||[]).length?(s.expose||[]).map(x=>'<span class="chip">'+esc(x)+'</span>').join(' '):'<span class="muted">none</span>')+'</div>'+
    '</details>'+
    '<details><summary>Environment</summary>'+kv(s.env||{})+'</details>'+
    '<details><summary>Volumes / Configs / Secrets</summary>'+
      '<div class="tbl-wrap"><table><tr><th>Kind</th><th>Source</th><th>Target</th><th>RO</th></tr>'+
        (s.volumes||[]).map(v=>'<tr><td>'+esc(v.kind||'')+'</td><td>'+esc(v.source||'')+'</td><td>'+esc(v.target||'')+'</td><td>'+esc(v.readOnly||false)+'</td></tr>').join('')+
        (s.configs||[]).map(v=>'<tr><td>config</td><td>'+esc(v.source||'')+'</td><td>'+esc(v.target||'')+'</td><td>'+esc(v.readOnly||false)+'</td></tr>').join('')+
        (s.secrets||[]).map(v=>'<tr><td>secret</td><td>'+esc(v.source||'')+'</td><td>'+esc(v.target||'')+'</td><td>'+esc(v.readOnly||false)+'</td></tr>').join('')+
      '</table></div>'+
    '</details>';
}
function renderRelations(){
  const items = Array.isArray((payload.report||{}).relations) ? payload.report.relations : [];
  const rows = items.slice(0,500).map(r=>'<tr><td><code>'+esc(r.from)+'</code></td><td>'+esc(r.type)+'</td><td><code>'+esc(r.to)+'</code></td><td>'+esc(r.detail||'')+'</td></tr>').join('');
  byId('relationsBox').innerHTML = rows ? '<div class="tbl-wrap"><table><tr><th>From</th><th>Type</th><th>To</th><th>Detail</th></tr>'+rows+'</table></div>' : '<div class="muted">No relations</div>';
}
function renderResources(){
  const items = Array.isArray((payload.report||{}).resources) ? payload.report.resources : [];
  const rows = items.map(r=>'<tr><td>'+esc(r.kind)+'</td><td><code>'+esc(r.name)+'</code></td></tr>').join('');
  byId('resourcesBox').innerHTML = rows ? '<div class="tbl-wrap"><table><tr><th>Kind</th><th>Name</th></tr>'+rows+'</table></div>' : '<div class="muted">No declared resources</div>';
}
function renderPreview(){
  byId('previewMeta').textContent = payload.previewError ? 'Preview unavailable' : 'Native helm-apps preview from compose import MVP';
  const cls = wrapYAML ? 'yaml' : 'yaml';
  if(payload.previewError){
    byId('previewBox').innerHTML = '<pre class="'+cls+'">'+esc(payload.previewError)+'</pre>';
    return;
  }
  byId('previewBox').innerHTML = '<pre class="'+cls+'" style="'+(wrapYAML?'white-space:pre-wrap;word-break:break-word':'')+'">'+yamlHtml(payload.previewYAML||'')+'</pre>';
}
function renderComposeSource(){
  const src = String((payload && payload.sourceYAML) || '');
  byId('composeMeta').textContent = src ? (((payload.report||{}).sourcePath)||'') : 'Source compose YAML unavailable';
  if(!src){
    byId('composeBox').innerHTML = '<div class="muted">Source compose YAML unavailable</div>';
    return;
  }
  byId('composeBox').innerHTML = '<pre class="yaml" style="'+(wrapComposeYAML?'white-space:pre-wrap;word-break:break-word':'')+'">'+yamlHtml(src)+'</pre>';
}
byId('svcSearch').addEventListener('input', e=>{ svcSearch = e.target.value || ''; renderServices(); });
byId('servicesBox').addEventListener('click', e=>{
  const tr = e.target.closest('tr[data-svc-id]');
  if(!tr) return;
  selectedServiceId = tr.getAttribute('data-svc-id') || '';
  renderServices();
});
byId('wrapBtn').addEventListener('click', ()=>{
  wrapYAML = !wrapYAML;
  byId('wrapBtn').textContent = 'Wrap YAML: ' + (wrapYAML ? 'On' : 'Off');
  renderPreview();
});
byId('wrapComposeBtn').addEventListener('click', ()=>{
  wrapComposeYAML = !wrapComposeYAML;
  byId('wrapComposeBtn').textContent = 'Wrap YAML: ' + (wrapComposeYAML ? 'On' : 'Off');
  renderComposeSource();
});
byId('exitBtn').addEventListener('click', ()=>{
  const btn = byId('exitBtn');
  btn.disabled = true; btn.textContent='Stopping...';
  fetch('/api/shutdown', {method:'POST'}).then(()=>{
    document.body.innerHTML='<div style="padding:24px;font:16px ui-sans-serif,system-ui">Server stopped. You can close this tab.</div>';
  }).catch(()=>{ btn.disabled=false; btn.textContent='Exit'; });
});
fetchJSON('/api/compose-report', 15000).then(data=>{
  payload = data || {};
  render();
  renderComposeSource();
}).catch(err=>{
  byId('summary').textContent = 'Failed to load: ' + String(err);
});
</script>
</body></html>`))
