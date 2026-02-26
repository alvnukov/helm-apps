package inspectweb

import "html/template"

var pageTemplate = template.Must(template.New("page").Parse(`<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>happ Inspect</title>
<style>
body{margin:0;font:14px/1.4 ui-sans-serif,system-ui,sans-serif;background:#f4f6f8;color:#1f2937}
.top{padding:12px 16px;background:#0f172a;color:#fff;display:flex;gap:16px;align-items:center}
.wrap{display:grid;grid-template-columns:300px 1fr 420px;height:calc(100vh - 48px)}
.pane{overflow:auto;border-right:1px solid #dbe1e8;background:white}
.pane:last-child{border-right:none}
.section{padding:12px 14px;border-bottom:1px solid #eef2f7}
.item{padding:8px 10px;border:1px solid #e5e7eb;border-radius:10px;margin:8px 0;cursor:pointer}
.item:hover{background:#f8fafc}
.muted{color:#64748b}
.badge{display:inline-block;background:#e2e8f0;border-radius:999px;padding:2px 8px;font-size:12px;margin-right:6px}
pre{white-space:pre-wrap;word-break:break-word;background:#0b1020;color:#dbeafe;padding:12px;border-radius:10px;overflow:auto}
.yaml{white-space:pre;word-break:normal;overflow:auto}
.yaml .k{color:#93c5fd}.yaml .s{color:#fcd34d}.yaml .n{color:#86efac}.yaml .c{color:#94a3b8;font-style:italic}
details{border:1px solid #e5e7eb;border-radius:10px;padding:8px 10px;margin:10px 0}
summary{cursor:pointer;font-weight:600}
table{width:100%;border-collapse:collapse} td,th{padding:6px;border-bottom:1px solid #eef2f7;text-align:left;vertical-align:top}
input{width:100%;padding:8px 10px;border:1px solid #cbd5e1;border-radius:8px}
.rel{font-size:12px;padding:6px;border-left:3px solid #94a3b8;background:#f8fafc;margin:6px 0}
.tabs{display:flex;gap:6px;flex-wrap:wrap}
.tabbtn{background:#e2e8f0;border:none;border-radius:8px;padding:6px 10px;cursor:pointer}
.tabbtn.active{background:#0f172a;color:#fff}
.pill{display:inline-block;font-size:11px;background:#eef2f7;color:#334155;border-radius:999px;padding:1px 7px;margin-right:4px}
.yamlnav{display:flex;gap:6px;flex-wrap:wrap;margin:8px 0}
.yamlnav a{font-size:12px;background:#eef2f7;color:#334155;border-radius:999px;padding:2px 8px;text-decoration:none}
.yamlline{display:block;padding:0 4px;border-radius:4px}
.yamlline.hl{background:rgba(250,204,21,.18);outline:1px solid rgba(250,204,21,.35)}
</style></head>
<body>
<div class="top"><strong>happ Inspect</strong><span id="summary"></span><span class="muted">API: <a href="/api/model.yaml" target="_blank" style="color:#93c5fd">/api/model.yaml</a></span><a href="/preview" target="_blank" style="margin-left:auto;background:#2563eb;color:white;border:none;border-radius:8px;padding:6px 10px;cursor:pointer;text-decoration:none;font-weight:600">Open Render Preview</a><button id="exitBtn" style="background:#ef4444;color:white;border:none;border-radius:8px;padding:6px 10px;cursor:pointer">Exit</button></div>
<div class="wrap">
  <div class="pane">
    <div class="section"><input id="search" placeholder="Search resources"></div>
    <div class="section"><div class="tabs"><button class="tabbtn active" id="tabApps">Applications</button><button class="tabbtn" id="tabResources">Resources</button></div></div>
    <div class="section"><h3 style="margin:0 0 8px" id="listTitle">Applications</h3><div id="apps"></div><div id="resources" style="display:none"></div></div>
    <div class="section"><h3 style="margin:0 0 8px">Relations</h3><div id="relations"></div></div>
  </div>
  <div class="pane">
    <div class="section"><h3 style="margin:0 0 8px">Details</h3><div id="detail">Click an application or resource</div></div>
  </div>
  <div class="pane">
    <div class="section"><h3 style="margin:0 0 8px">Values / Template Analysis</h3><div id="analysis">Loading…</div></div>
    <div class="section"><h3 style="margin:0 0 8px">helm-apps Preview</h3><div id="helmAppsPreview">Preview is available in the top bar: <code>/preview</code></div></div>
  </div>
</div>
<script>
let model=null, selected=null, selectedType='app', listMode='apps', previewLines=[];
const byId=id=>document.getElementById(id);
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
  }).join('');
}
function buildYamlPreview(yamlText){
  const lines = String(yamlText||'').split('\n');
  const nav = [];
  const html = lines.map((line, idx)=>{
    const mTop = line.match(/^([A-Za-z0-9_.-]+):\s*$/);
    if(mTop){
      nav.push({label:mTop[1], line:idx+1});
    }
    return '<span id="yaml-line-'+(idx+1)+'" data-line="'+(idx+1)+'" class="yamlline" title="'+escAttr(line)+'">'+yamlHtml(line)+'</span>';
  }).join('');
  return {html, nav, lines};
}
function fetchWithTimeout(url, opts, ms){
  const controller = new AbortController();
  const t = setTimeout(()=>controller.abort(), ms||10000);
  const req = Object.assign({}, opts||{}, {signal: controller.signal});
  return fetch(url, req).finally(()=>clearTimeout(t));
}
function focusPreviewByNeedles(needles){
  const pre = byId('previewYaml');
  if(!pre) return;
  pre.querySelectorAll('.yamlline.hl').forEach(n=>n.classList.remove('hl'));
  if(!needles || !needles.length) return;
  const lines = previewLines || [];
  let hitIdx = -1;
  for(let i=0;i<lines.length;i++){
    const ln = lines[i];
    for(const n of needles){
      if(n && ln.includes(n+':')) { hitIdx = i; break; }
      if(n && ln.includes(n)) { hitIdx = i; break; }
    }
    if(hitIdx >= 0) break;
  }
  if(hitIdx < 0) return;
  const el = byId('yaml-line-'+(hitIdx+1));
  if(!el) return;
  el.classList.add('hl');
  el.scrollIntoView({block:'center', behavior:'smooth'});
}
function setListMode(mode){
  listMode=mode;
  byId('tabApps').classList.toggle('active', mode==='apps');
  byId('tabResources').classList.toggle('active', mode==='resources');
  byId('apps').style.display = mode==='apps' ? '' : 'none';
  byId('resources').style.display = mode==='resources' ? '' : 'none';
  byId('listTitle').textContent = mode==='apps' ? 'Applications' : 'Resources';
}
function renderApps(){
 const q=byId('search').value.toLowerCase();
 const el=byId('apps'); el.innerHTML='';
 (model.applications||[]).filter(a=>{
   const txt=(a.name+' '+(a.namespace||'')+' '+(a.workloadKind||'')+' '+(a.typeHint||'')).toLowerCase();
   return txt.includes(q);
 }).forEach(a=>{
   const d=document.createElement('div'); d.className='item';
   d.innerHTML =
     '<div><span class="badge">'+esc(a.typeHint||'app')+'</span><strong>'+esc(a.name)+'</strong></div>'+
     '<div class="muted">'+esc(a.namespace||'default')+'</div>'+
     '<div style="margin-top:6px">'+
       '<span class="pill">res '+(a.resourceIds||[]).length+'</span>'+
       '<span class="pill">svc '+(a.serviceIds||[]).length+'</span>'+
       '<span class="pill">ing '+(a.ingressIds||[]).length+'</span>'+
       '<span class="pill">cm '+(a.configMapIds||[]).length+'</span>'+
       '<span class="pill">sec '+(a.secretIds||[]).length+'</span>'+
     '</div>';
   d.onclick=()=>{selectedType='app'; selected=a.id; renderDetail();};
   el.appendChild(d);
 });
}
function renderResources(){
 const q=byId('search').value.toLowerCase();
 const el=byId('resources'); el.innerHTML='';
 model.resources.filter(r=>(r.kind+' '+r.name+' '+(r.namespace||'')).toLowerCase().includes(q)).forEach(r=>{
   const d=document.createElement('div'); d.className='item';
   d.innerHTML='<div><span class="badge">'+esc(r.kind)+'</span><strong>'+esc(r.name)+'</strong></div><div class="muted">'+esc(r.namespace||'default')+' · '+esc(r.apiVersion)+'</div>';
   d.onclick=()=>{selectedType='resource'; selected=r.id; renderDetail();};
   el.appendChild(d);
 });
}
function renderRelations(){
 const el=byId('relations'); el.innerHTML='';
 model.relations.forEach(rel=>{
   const d=document.createElement('div'); d.className='rel';
   d.innerHTML='<div><strong>'+esc(rel.type)+'</strong></div><div><a href="#" data-id="'+esc(rel.from)+'">'+esc(shortId(rel.from))+'</a> → <a href="#" data-id="'+esc(rel.to)+'">'+esc(shortId(rel.to))+'</a></div><div class="muted">'+esc(rel.detail||'')+'</div>';
   el.appendChild(d);
 });
 el.querySelectorAll('a[data-id]').forEach(a=>a.onclick=(e)=>{e.preventDefault(); selected=e.target.dataset.id; renderDetail();});
}
function shortId(id){ const p=id.split('/'); return p.slice(-2).join('/'); }
function renderDetail(){
 const el=byId('detail');
 if(selectedType==='app'){
   const a=(model.applications||[]).find(x=>x.id===selected);
   if(!a){ el.innerHTML='Click an application or resource'; return; }
   const resources=(a.resourceIds||[]).map(id=>model.resources.find(r=>r.id===id)).filter(Boolean);
   const planRows=(a.importPlan||[]).map(p=>{
     const rid=shortId(p.resourceId);
     return '<tr><td>'+esc(rid)+'</td><td>'+esc(p.target)+'</td><td>'+esc(p.mode)+'</td><td>'+esc(p.reason||'')+'</td></tr>';
   }).join('');
   const resList=resources.map(r=>'<div class="rel"><strong>'+esc(r.kind)+'</strong> <a href="#" data-resid="'+esc(r.id)+'">'+esc(r.name)+'</a><div class="muted">'+esc(r.namespace||'default')+'</div></div>').join('') || '<div class="muted">No resources</div>';
   el.innerHTML =
     '<div><span class="badge">'+esc(a.typeHint||'app')+'</span><strong>'+esc(a.name)+'</strong> <span class="muted">'+esc(a.namespace||'default')+'</span></div>'+
     '<div class="muted" style="margin:6px 0 10px">Workload: '+esc((a.workloadKind||'-')+' '+(a.workloadName||''))+'</div>'+
     '<details open><summary>Application Resources ('+resources.length+')</summary>'+resList+'</details>'+
     '<details open><summary>Import Plan ('+((a.importPlan||[]).length)+')</summary>'+(planRows?'<table><tr><th>Resource</th><th>Target</th><th>Mode</th><th>Reason</th></tr>'+planRows+'</table>':'<div class="muted">No plan</div>')+'</details>';
   el.querySelectorAll('a[data-resid]').forEach(link=>link.onclick=(e)=>{e.preventDefault(); selectedType='resource'; selected=e.target.dataset.resid; renderDetail();});
   focusPreviewByNeedles([a.name, a.workloadName]);
   return;
 }
 const r=model.resources.find(x=>x.id===selected);
 if(!r){ el.innerHTML='Click an application or resource'; return; }
 const rels=model.relations.filter(x=>x.from===r.id||x.to===r.id);
 const relHtml = (rels.map(x=>'<div class="rel"><strong>'+esc(x.type)+'</strong> '+esc(shortId(x.from))+' → '+esc(shortId(x.to))+'<div class="muted">'+esc(x.detail||'')+'</div></div>').join('')) || '<div class="muted">No detected relations</div>';
 const specState = (r.spec === null || r.spec === undefined) ? 'absent' : 'present';
 const dataState = (r.data === null || r.data === undefined) ? 'absent' : 'present';
 el.innerHTML =
   '<div><span class="badge">'+esc(r.kind)+'</span><strong>'+esc(r.name)+'</strong> <span class="muted">'+esc(r.namespace||'default')+'</span></div>'+
   '<div class="muted" style="margin:6px 0 10px">'+esc(r.apiVersion)+' · '+esc(r.id)+'</div>'+
   '<div class="muted" style="margin:0 0 10px">Spec: '+specState+' · Data: '+dataState+'</div>'+
   '<details open><summary>Spec / Data (YAML)</summary><pre class="yaml">'+yamlHtml(r.summaryYAML)+'</pre></details>'+
   '<details><summary>Raw Resource (YAML)</summary><pre class="yaml">'+yamlHtml(r.rawYAML)+'</pre></details>'+
   '<details open><summary>Relations ('+rels.length+')</summary>'+relHtml+'</details>';
 focusPreviewByNeedles([r.name]);
}
function renderAnalysis(){
 const el=byId('analysis');
 const a=model.analysis;
 if(!a){ el.innerHTML='<div class="muted">No template analysis (available in chart inspect mode)</div>'; return; }
 const occ = (a.occurrences||[]).map(o=>'<details><summary>'+esc(o.valuesPaths.join(', '))+' <span class="muted">('+esc(o.file+':'+o.line)+')</span></summary><div><div class="muted">Pipes: '+esc((o.pipeFunctions||[]).join(', ')||'-')+' · Hint: '+esc(o.formatHint||'-')+'</div><pre>'+esc(o.action)+'</pre></div></details>').join('');
 const risky = (a.riskyConstructs||[]).map(r=>'<tr><td>'+esc(r.kind)+'</td><td>'+esc(r.file+':'+r.line)+'</td><td><code>'+esc(r.snippet)+'</code></td></tr>').join('');
 const riskyOpen = ((a.riskyConstructs||[]).length ? ' open' : '');
 const riskyBlock = risky ? ('<table><tr><th>Kind</th><th>Location</th><th>Snippet</th></tr>'+risky+'</table>') : '<div class="muted">None detected</div>';
 el.innerHTML =
   '<div><strong>Templates:</strong> '+a.templateFiles+' · <strong>.Values paths:</strong> '+((a.summary&&a.summary.uniqueValuesPaths)||0)+' · <strong>Occurrences:</strong> '+((a.summary&&a.summary.occurrences)||0)+'</div>'+
   '<details open><summary>Values Paths ('+((a.valuesPaths||[]).length)+')</summary><pre class="yaml">'+yamlHtml((a.valuesPaths||[]).map(x=>'- '+x).join('\\n'))+'</pre></details>'+
   '<details open><summary>Occurrences ('+((a.occurrences||[]).length)+')</summary>'+(occ || '<div class="muted">No occurrences</div>')+'</details>'+
   '<details'+riskyOpen+'><summary>Risky Constructs ('+((a.riskyConstructs||[]).length)+')</summary>'+riskyBlock+'</details>';
}
function renderHelmAppsPreview(){
 const el=byId('helmAppsPreview');
 if(!model.helmApps){ el.innerHTML='<div class="muted">Preview unavailable</div>'; return; }
 const p=model.helmApps;
 const meta = '<div class="muted">strategy: '+esc(p.strategy||'-')+' · group: '+esc((p.groupName||'-'))+' · renderer: '+esc((p.groupType||'-'))+'</div>';
 el.innerHTML = meta + '<div class="muted" style="margin-top:8px">Open generated values preview from the top bar.</div>';
}
fetchWithTimeout('/api/model', null, 10000).then(r=>r.json()).then(m=>{
  model=m;
  byId('summary').textContent='Apps: '+(m.summary.applicationCount||0)+' · Resources: '+m.summary.resourceCount+' · Relations: '+m.summary.relationCount;
  renderApps(); renderResources(); renderRelations(); renderAnalysis(); renderHelmAppsPreview();
  if((m.applications||[])[0]){selectedType='app'; selected=m.applications[0].id;}
  else if((m.resources||[])[0]){selectedType='resource'; selected=m.resources[0].id;}
  renderDetail();
});
byId('search').addEventListener('input', renderResources);
byId('search').addEventListener('input', renderApps);
byId('tabApps').addEventListener('click', ()=>setListMode('apps'));
byId('tabResources').addEventListener('click', ()=>setListMode('resources'));
byId('exitBtn').addEventListener('click', ()=>{
  const btn = byId('exitBtn');
  btn.disabled = true;
  btn.textContent = 'Stopping...';
  fetchWithTimeout('/api/shutdown', {method:'POST'}, 5000).then(()=>{ document.body.innerHTML='<div style="padding:24px;font:16px ui-sans-serif,system-ui">Server stopped. You can close this tab.</div>'; }).catch(()=>{ btn.disabled=false; btn.textContent='Exit'; });
});
</script>
</body></html>`))
