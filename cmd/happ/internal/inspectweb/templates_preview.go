package inspectweb

import "html/template"

var previewTemplate = template.Must(template.New("preview").Parse(`<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>helm-apps Values Lab</title>
<style>
body{margin:0;font:14px/1.4 ui-sans-serif,system-ui,sans-serif;background:#f4f6f8;color:#1f2937}
.top{padding:12px 16px;background:#0f172a;color:#fff;display:flex;gap:12px;align-items:center}
.wrap{padding:14px;max-width:1600px;margin:0 auto}
.grid{display:grid;grid-template-columns:minmax(420px,1fr) minmax(520px,1.2fr);gap:12px;align-items:start}
.grid > div{min-width:0}
.stickybar{position:sticky;top:10px;z-index:10}
.toolbar{display:flex;gap:10px;justify-content:space-between;align-items:center;flex-wrap:wrap}
.toolbar .group{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.seg{display:inline-flex;background:#e2e8f0;border-radius:10px;padding:3px;gap:3px}
.seg button{background:transparent;color:#334155;border:none;border-radius:8px;padding:6px 10px;font-size:12px}
.seg button.active{background:#2563eb;color:#fff}
.toolbar a{font-size:12px;text-decoration:none;color:#1d4ed8;background:#eff6ff;padding:6px 10px;border-radius:8px}
.toolbar .label{font-size:12px;color:#64748b}
.toolbar input[type="range"]{width:140px}
.card{background:#fff;border:1px solid #e5e7eb;border-radius:12px;padding:12px 14px;margin-bottom:12px;overflow:hidden}
.card.hidden-by-focus{display:none}
.muted{color:#64748b}
pre{white-space:pre-wrap;word-break:break-word;background:#0b1020;color:#dbeafe;padding:12px;border-radius:10px;overflow:auto;max-width:100%}
.yaml{white-space:pre;word-break:normal;overflow:auto}
body.yaml-wrap .yaml{white-space:pre-wrap;word-break:break-word}
textarea{width:100%;min-height:420px;box-sizing:border-box;border:1px solid #cbd5e1;border-radius:10px;padding:10px;font:12px/1.4 ui-monospace,SFMono-Regular,Menlo,monospace;background:#fff;color:#0f172a;resize:vertical;max-width:100%}
.editor-host{height:520px;border:1px solid #cbd5e1;border-radius:10px;overflow:hidden;background:#0b1020}
.editor-fallback{display:none}
button{border:none;border-radius:8px;padding:7px 12px;cursor:pointer;font-weight:600}
.btn-primary{background:#2563eb;color:#fff}
.btn-secondary{background:#e2e8f0;color:#0f172a}
.btn-success{background:#16a34a;color:#fff}
.row{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.field{display:flex;flex-direction:column;gap:6px}
.field input{padding:8px 10px;border:1px solid #cbd5e1;border-radius:8px}
.status{font-size:12px;padding:6px 8px;border-radius:8px;background:#eef2f7;color:#334155}
.status.err{background:#fee2e2;color:#991b1b}
.status.ok{background:#dcfce7;color:#166534}
.chips{display:flex;gap:6px;flex-wrap:wrap}
.chip{font-size:12px;background:#eef2f7;color:#334155;border-radius:999px;padding:2px 8px}
.cmp-table{width:100%;border-collapse:collapse;font-size:12px}
.cmp-table th,.cmp-table td{border-bottom:1px solid #e5e7eb;padding:6px 8px;text-align:left;vertical-align:top}
.cmp-table-wrap{max-height:420px;overflow:auto;border:1px solid #e5e7eb;border-radius:10px}
.cmp-row{cursor:pointer}
.cmp-row:hover{background:#f8fafc}
.cmp-row.sel{background:#eff6ff}
.cmp-equal{color:#166534}.cmp-changed{color:#92400e}.cmp-missing{color:#991b1b}.cmp-extra{color:#1d4ed8}
.cmp-tools{display:flex;gap:6px;flex-wrap:wrap;margin:8px 0}
.cmp-tools{position:sticky;top:0;background:#fff;padding:6px 0;z-index:2}
.cmp-tools button{font-size:12px;background:#e2e8f0;color:#334155;border:none;border-radius:8px;padding:4px 8px;cursor:pointer}
.cmp-tools button.active{background:#2563eb;color:#fff}
.cmp-tools input{font-size:12px;padding:6px 8px;border:1px solid #cbd5e1;border-radius:8px;min-width:260px;max-width:100%}
.cmp-panels{display:grid;grid-template-columns:1fr 1fr;gap:10px;margin-top:10px}
.cmp-panel-title{font-size:12px;font-weight:700;color:#334155;margin-bottom:4px}
.cmp-details-sticky{position:sticky;top:64px}
@media (max-width: 900px){.cmp-panels{grid-template-columns:1fr}}
.yaml .k{color:#93c5fd}.yaml .s{color:#fcd34d}.yaml .n{color:#86efac}.yaml .c{color:#94a3b8;font-style:italic}
.yamlnav{display:flex;gap:6px;flex-wrap:wrap;margin:8px 0}
.yamlnav a{font-size:12px;background:#eef2f7;color:#334155;border-radius:999px;padding:2px 8px;text-decoration:none}
.foldtools{display:flex;gap:6px;flex-wrap:wrap;margin:8px 0}
.foldtools button{font-size:12px;background:#e2e8f0;color:#334155;border:none;border-radius:8px;padding:4px 8px;cursor:pointer}
.yamlline{display:block;padding:0 4px;border-radius:4px}
.yamlline.hl{background:rgba(250,204,21,.18);outline:1px solid rgba(250,204,21,.35)}
.yamlline.hidden{display:none}
.foldmark{display:inline-block;width:14px;color:#94a3b8;cursor:pointer;user-select:none}
.foldmark.sp{cursor:default}
details{border:1px solid #e5e7eb;border-radius:10px;padding:8px 10px;margin:10px 0}
summary{cursor:pointer;font-weight:600}
body.focus-editor .grid, body.focus-compare .grid, body.focus-render .grid{grid-template-columns:1fr}
body.focus-render #leftCol{display:none}
body.focus-editor #resultCard, body.focus-editor #compareCard{display:none}
body.focus-editor #rightCol .card:not(#previewCard):not(#metaCard){display:none}
body.focus-compare #editorCard, body.focus-compare #resultCard{display:none}
body.focus-compare #rightCol .card:not(#previewCard):not(#metaCard){display:none}
@media (max-width: 1100px){.grid{grid-template-columns:1fr}}
</style></head>
<body>
<div class="top">
  <strong>helm-apps Values Lab</strong>
  <a href="/" style="color:#93c5fd">Back to Inspect</a>
  <a href="/api/model.yaml" target="_blank" style="color:#93c5fd">/api/model.yaml</a>
</div>
<div class="wrap">
  <div class="card stickybar">
    <div class="toolbar">
      <div class="group">
        <span class="label">Focus</span>
        <div class="seg" id="focusSeg">
          <button data-focus="all" class="active">All</button>
          <button data-focus="editor">Editor</button>
          <button data-focus="compare">Compare</button>
          <button data-focus="render">Render</button>
        </div>
      </div>
      <div class="group">
        <span class="label">Sections</span>
        <a href="#editorCard">Editor</a>
        <a href="#compareCard">Compare</a>
        <a href="#previewCard">Render</a>
      </div>
      <div class="group">
        <span class="label">Layout</span>
        <input id="paneRatio" type="range" min="35" max="70" value="46" title="Left pane width">
        <button id="yamlWrapBtn" class="btn-secondary" style="padding:6px 10px;font-size:12px">Wrap YAML: Off</button>
      </div>
    </div>
  </div>
  <div class="grid">
    <div id="leftCol">
      <div id="editorCard" class="card">
        <div class="row" style="justify-content:space-between">
          <div>
            <div style="font-weight:700">Source Chart values.yaml</div>
            <div class="muted">Edit values of the inspected chart, rerender and compare results.</div>
          </div>
          <div id="editorStatus" class="status">Loading…</div>
        </div>
        <div style="margin-top:10px" class="field">
          <label for="savePath" class="muted">Save research values to file</label>
          <input id="savePath" placeholder="/path/to/values.inspect.yaml">
        </div>
        <div class="row" style="margin-top:10px">
          <button id="runBtn" class="btn-primary">Run Render + Analyze</button>
          <button id="validateBtn" class="btn-secondary">Validate YAML</button>
          <button id="formatBtn" class="btn-secondary">Format YAML</button>
          <button id="saveBtn" class="btn-success">Save values file</button>
          <button id="resetBtn" class="btn-secondary">Reset from chart</button>
        </div>
        <div class="muted" style="margin-top:8px">Editor tips: <strong>Ctrl/Cmd+F</strong> search, <strong>Ctrl/Cmd+L</strong> go to line (Ace). Format may remove comments.</div>
        <div style="margin-top:10px">
          <div class="muted" style="margin:0 0 6px">YAML Editor</div>
          <div id="valuesEditorAce" class="editor-host"></div>
          <textarea id="valuesEditor" class="editor-fallback" spellcheck="false" placeholder="Loading chart values.yaml..."></textarea>
        </div>
      </div>
      <div id="resultCard" class="card">
        <div style="font-weight:700;margin-bottom:6px">Render Result / Errors</div>
        <div id="experimentStatus" class="muted">No experiment run yet.</div>
        <div id="diffBox" style="margin-top:8px"></div>
      </div>
      <div id="compareCard" class="card">
        <div class="row" style="justify-content:space-between">
          <div>
            <div style="font-weight:700">Entity Compare (Source chart vs Library render)</div>
            <div class="muted">Compares rendered Kubernetes resources after conversion to helm-apps.</div>
          </div>
          <button id="compareBtn" class="btn-secondary">Run compare</button>
        </div>
        <div id="compareStatus" class="muted" style="margin-top:8px">No comparison run yet.</div>
        <div id="compareBox" style="margin-top:8px"></div>
        <div id="compareDetails" style="margin-top:8px"></div>
      </div>
    </div>
    <div id="rightCol">
      <div id="metaCard" class="card"><div id="meta" class="muted">Loading…</div></div>
      <div id="previewCard" class="card"><div id="content">Loading…</div></div>
    </div>
  </div>
</div>
<script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.32.6/ace.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.32.6/ext-language_tools.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/js-yaml/4.1.0/js-yaml.min.js"></script>
<script>
const byId=id=>document.getElementById(id);
let initialValuesYAML='', currentModel=null, valuesEditorAce=null;
let previewLineMeta=[], collapsedPreviewLines=new Set();
const previewFoldStorageKey='helmAppsImport.preview.folds';
let comparePayload=null, compareFilter='all', compareSelectedKey='', compareHideEqual=false;
let compareSearch='', previewFocus='all';
const previewUIStorageKey='helmAppsImport.preview.ui';
let previewUIPrefs={paneRatio:46,yamlWrap:false};
function setCookie(name, value, days){
  try{
    const d = new Date();
    d.setTime(d.getTime() + ((Number(days||30)||30) * 24 * 60 * 60 * 1000));
    document.cookie = name + '=' + encodeURIComponent(String(value)) + '; expires=' + d.toUTCString() + '; path=/; SameSite=Lax';
  }catch(_){}
}
function getCookie(name){
  try{
    const prefix = name + '=';
    const parts = String(document.cookie || '').split(';');
    for(let i=0;i<parts.length;i++){
      const p = parts[i].trim();
      if(p.startsWith(prefix)) return decodeURIComponent(p.slice(prefix.length));
    }
  }catch(_){}
  return '';
}
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
  const meta = lines.map((line, idx)=>{
    const indent = (line.match(/^ */)||[''])[0].length;
    return {lineNo: idx+1, indent, raw: line, collapsible:false};
  });
  for(let i=0;i<lines.length;i++){
    const mTop = lines[i].match(/^([A-Za-z0-9_.-]+):\s*$/);
    if(mTop) nav.push({label:mTop[1], line:i+1});
    if(!lines[i].trim() || /^\s*#/.test(lines[i])) continue;
    const curIndent = meta[i].indent;
    for(let j=i+1;j<lines.length;j++){
      if(!lines[j].trim()) continue;
      const nextIndent = meta[j].indent;
      if(nextIndent > curIndent){ meta[i].collapsible = true; }
      break;
    }
  }
  const html = lines.map((line, idx)=>{
    const m = meta[idx];
    const fm = m.collapsible ? '<span class="foldmark" data-fold-line="'+m.lineNo+'" title="Toggle fold">▾</span>' : '<span class="foldmark sp"> </span>';
    return '<span id="yaml-line-'+(idx+1)+'" data-line="'+m.lineNo+'" data-indent="'+m.indent+'" class="yamlline" title="'+escAttr(line)+'">'+fm+yamlHtml(line)+'</span>';
  }).join('');
  return {html, nav, meta};
}
function loadPreviewFoldState(){
  try{
    const raw = sessionStorage.getItem(previewFoldStorageKey) || getCookie(previewFoldStorageKey);
    const arr = raw ? JSON.parse(raw) : [];
    collapsedPreviewLines = new Set(Array.isArray(arr) ? arr.map(x=>Number(x)).filter(Boolean) : []);
  }catch(_){ collapsedPreviewLines = new Set(); }
}
function loadPreviewUIPrefs(){
  try{
    const raw = sessionStorage.getItem(previewUIStorageKey) || getCookie(previewUIStorageKey);
    const v = raw ? JSON.parse(raw) : {};
    if(v && typeof v === 'object'){
      if(Number.isFinite(Number(v.paneRatio))) previewUIPrefs.paneRatio = Math.min(70, Math.max(35, Number(v.paneRatio)));
      previewUIPrefs.yamlWrap = !!v.yamlWrap;
    }
  }catch(_){}
}
function savePreviewUIPrefs(){
  try{
    const raw = JSON.stringify(previewUIPrefs);
    sessionStorage.setItem(previewUIStorageKey, raw);
    setCookie(previewUIStorageKey, raw, 90);
  }catch(_){}
}
function applyPreviewUIPrefs(){
  const grid = document.querySelector('.grid');
  if(grid){
    const left = Math.min(70, Math.max(35, Number(previewUIPrefs.paneRatio||46)));
    const right = 100-left;
    grid.style.gridTemplateColumns = 'minmax(420px,'+left+'fr) minmax(520px,'+right+'fr)';
  }
  document.body.classList.toggle('yaml-wrap', !!previewUIPrefs.yamlWrap);
  const ratio = byId('paneRatio');
  if(ratio) ratio.value = String(previewUIPrefs.paneRatio);
  const wrapBtn = byId('yamlWrapBtn');
  if(wrapBtn) wrapBtn.textContent = 'Wrap YAML: ' + (previewUIPrefs.yamlWrap ? 'On' : 'Off');
}
function savePreviewFoldState(){
  try{
    const raw = JSON.stringify(Array.from(collapsedPreviewLines).sort((a,b)=>a-b));
    sessionStorage.setItem(previewFoldStorageKey, raw);
    setCookie(previewFoldStorageKey, raw, 30);
  }catch(_){}
}
function applyPreviewFolds(){
  const pre = byId('previewYaml');
  if(!pre) return;
  const lines = Array.from(pre.querySelectorAll('.yamlline'));
  lines.forEach(el=>{
    el.classList.remove('hidden');
    const mark = el.querySelector('.foldmark[data-fold-line]');
    if(mark) mark.textContent = collapsedPreviewLines.has(Number(mark.dataset.foldLine)) ? '▸' : '▾';
  });
  for(let i=0;i<previewLineMeta.length;i++){
    const m = previewLineMeta[i];
    if(!collapsedPreviewLines.has(m.lineNo)) continue;
    for(let j=i+1;j<previewLineMeta.length;j++){
      if(previewLineMeta[j].indent <= m.indent && previewLineMeta[j].raw.trim() !== '') break;
      const el = byId('yaml-line-'+previewLineMeta[j].lineNo);
      if(el) el.classList.add('hidden');
    }
  }
}
function unfoldParentsForLine(lineNo){
  const idx = previewLineMeta.findIndex(m=>m.lineNo===lineNo);
  if(idx < 0) return;
  const targetIndent = previewLineMeta[idx].indent;
  for(let i=idx-1;i>=0;i--){
    const m = previewLineMeta[i];
    if(!m.collapsible) continue;
    if(m.indent < targetIndent && collapsedPreviewLines.has(m.lineNo)){
      collapsedPreviewLines.delete(m.lineNo);
    }
  }
}
function bindPreviewFoldUI(){
  const pre = byId('previewYaml');
  if(!pre) return;
  pre.querySelectorAll('.foldmark[data-fold-line]').forEach(el=>el.onclick=(e)=>{
    e.preventDefault();
    e.stopPropagation();
    const lineNo = Number(el.dataset.foldLine||0);
    if(!lineNo) return;
    if(collapsedPreviewLines.has(lineNo)) collapsedPreviewLines.delete(lineNo); else collapsedPreviewLines.add(lineNo);
    savePreviewFoldState();
    applyPreviewFolds();
  });
}
function collapseAllPreview(){
  collapsedPreviewLines = new Set((previewLineMeta||[]).filter(m=>m.collapsible).map(m=>m.lineNo));
  savePreviewFoldState();
  applyPreviewFolds();
}
function expandAllPreview(){
  collapsedPreviewLines = new Set();
  savePreviewFoldState();
  applyPreviewFolds();
}
function foldPreviewToLevel(level){
  const threshold = Math.max(0, Number(level||1)-1);
  collapsedPreviewLines = new Set((previewLineMeta||[])
    .filter(m=>m.collapsible && Math.floor((m.indent||0)/2) >= threshold)
    .map(m=>m.lineNo));
  savePreviewFoldState();
  applyPreviewFolds();
}
function bindPreviewFoldToolbar(){
  const root = byId('content');
  if(!root) return;
  root.querySelectorAll('[data-fold-action]').forEach(btn=>btn.onclick=(e)=>{
    e.preventDefault();
    const action = btn.dataset.foldAction;
    if(action === 'collapse-all') return collapseAllPreview();
    if(action === 'expand-all') return expandAllPreview();
    if(action === 'level') return foldPreviewToLevel(Number(btn.dataset.foldLevel||1));
  });
}
function fetchWithTimeout(url, opts, ms){
  const controller = new AbortController();
  const t = setTimeout(()=>controller.abort(), ms||10000);
  const req = Object.assign({}, opts||{}, {signal: controller.signal});
  return fetch(url, req).finally(()=>clearTimeout(t));
}
function initCodeEditor(){
  if(!window.ace) {
    byId('valuesEditor').style.display = '';
    setEditorStatus('Editor fallback (Ace failed to load)', 'err');
    return;
  }
  try {
    valuesEditorAce = ace.edit('valuesEditorAce');
    valuesEditorAce.setTheme('ace/theme/tomorrow_night');
    valuesEditorAce.session.setMode('ace/mode/yaml');
    valuesEditorAce.session.setUseWrapMode(false);
    valuesEditorAce.setOptions({
      fontSize: '13px',
      showPrintMargin: false,
      tabSize: 2,
      useSoftTabs: true,
      enableBasicAutocompletion: true,
      enableLiveAutocompletion: false,
    });
    valuesEditorAce.session.on('change', ()=>{ byId('valuesEditor').value = valuesEditorAce.getValue(); });
    clearAceAnnotations();
  } catch (e) {
    byId('valuesEditor').style.display = '';
    setEditorStatus('Editor init error: '+String(e), 'err');
  }
}
function clearAceAnnotations(){
  if(!valuesEditorAce) return;
  valuesEditorAce.session.clearAnnotations();
}
function setAceYamlError(err){
  if(!valuesEditorAce || !err || !err.mark) return;
  const row = Math.max(0, Number(err.mark.line || 0));
  const col = Math.max(0, Number(err.mark.column || 0));
  valuesEditorAce.session.setAnnotations([{
    row,
    column: col,
    text: String(err.message || err.reason || 'YAML parse error'),
    type: 'error',
  }]);
  valuesEditorAce.scrollToLine(row, true, true, function(){});
  valuesEditorAce.gotoLine(row + 1, col, true);
}
function validateEditorYAML(showStatus){
  const text = getEditorValue();
  clearAceAnnotations();
  if(!window.jsyaml){
    if(showStatus) setEditorStatus('YAML validator unavailable (js-yaml failed to load)', 'err');
    return {ok:false, error:new Error('js-yaml not loaded')};
  }
  try {
    jsyaml.load(text);
    if(showStatus) setEditorStatus('YAML valid', 'ok');
    return {ok:true};
  } catch (e) {
    setAceYamlError(e);
    if(showStatus) {
      const line = e && e.mark ? (e.mark.line + 1) : '?';
      const col = e && e.mark ? (e.mark.column + 1) : '?';
      setEditorStatus('YAML error at '+line+':'+col+' - '+String(e.reason || e.message || e), 'err');
    }
    return {ok:false, error:e};
  }
}
function setEditorValue(v){
  const s = String(v||'');
  byId('valuesEditor').value = s;
  if(valuesEditorAce){
    const pos = valuesEditorAce.getCursorPosition();
    valuesEditorAce.setValue(s, -1);
    try { valuesEditorAce.moveCursorToPosition(pos); } catch(_) {}
    clearAceAnnotations();
  }
}
function getEditorValue(){
  if(valuesEditorAce) return valuesEditorAce.getValue();
  return byId('valuesEditor').value;
}
function renderPreviewFromModel(m){
  currentModel = m;
  const p = m && m.helmApps;
  if(!p){
    byId('meta').textContent='Preview unavailable';
    byId('content').innerHTML='<div class="muted">No helm-apps preview in model</div>';
    return;
  }
  byId('meta').textContent='strategy: '+(p.strategy||'-')+' · group: '+(p.groupName||'-')+' · renderer: '+(p.groupType||'-')+
    ' · resources: '+((m.summary&&m.summary.resourceCount)||0)+' · apps: '+((m.summary&&m.summary.applicationCount)||0);
  if(p.error){
    byId('content').innerHTML='<pre class="yaml">'+yamlHtml(String(p.error))+'</pre>';
    return;
  }
  const pv = buildYamlPreview(p.valuesYAML||'');
  previewLineMeta = pv.meta || [];
  byId('content').innerHTML =
    '<div class="yamlnav">'+(pv.nav||[]).map(n=>'<a href="#" data-yline="'+n.line+'">'+esc(n.label)+'</a>').join('')+'</div>'+
    '<div class="foldtools">'+
      '<button data-fold-action="collapse-all">Collapse all</button>'+
      '<button data-fold-action="expand-all">Expand all</button>'+
      '<button data-fold-action="level" data-fold-level="1">L1</button>'+
      '<button data-fold-action="level" data-fold-level="2">L2</button>'+
      '<button data-fold-action="level" data-fold-level="3">L3</button>'+
      '<button data-fold-action="level" data-fold-level="4">L4</button>'+
    '</div>'+
    '<pre id="previewYaml" class="yaml">'+pv.html+'</pre>';
  bindPreviewFoldToolbar();
  bindPreviewFoldUI();
  applyPreviewFolds();
  byId('content').querySelectorAll('a[data-yline]').forEach(a=>a.onclick=(e)=>{
    e.preventDefault();
    const ln=Number(e.target.dataset.yline||0);
    unfoldParentsForLine(ln);
    applyPreviewFolds();
    const el=byId('yaml-line-'+ln);
    if(!el) return;
    byId('content').querySelectorAll('.yamlline.hl').forEach(x=>x.classList.remove('hl'));
    el.classList.add('hl');
    el.scrollIntoView({block:'center', behavior:'smooth'});
  });
}
function setEditorStatus(text, cls){
  const el = byId('editorStatus');
  el.textContent = text;
  el.className = 'status' + (cls ? (' '+cls) : '');
}
function setExperimentStatus(text, cls){
  const el = byId('experimentStatus');
  el.textContent = text;
  el.className = cls ? ('status '+cls) : 'muted';
}
function setCompareStatus(text, cls){
  const el = byId('compareStatus');
  el.textContent = text;
  el.className = cls ? ('status '+cls) : 'muted';
}
function renderDiff(diff){
  const el = byId('diffBox');
  if(!diff){ el.innerHTML=''; return; }
  const mk = (title, items, color)=>'<details'+(items&&items.length?' open':'')+'><summary>'+title+' ('+((items&&items.length)||0)+')</summary>'+
    ((items&&items.length)?('<div class="chips">'+items.map(x=>'<span class="chip" style="background:'+color+';color:#111827">'+esc(x)+'</span>').join('')+'</div>'):'<div class="muted">None</div>')+
    '</details>';
  el.innerHTML = mk('Added resources', diff.added, '#d1fae5') + mk('Changed resources', diff.changed, '#fef3c7') + mk('Removed resources', diff.removed, '#fee2e2');
}
function renderCompare(res){
  comparePayload = normalizeComparePayload(res);
  renderCompareUI();
}
function normalizeComparePayload(res){
  if(res && res.compare && Array.isArray(res.compare.resources)){
    return {
      compare: res.compare,
      sourceYAMLByKey: res.sourceYAMLByKey || {},
      generatedYAMLByKey: res.generatedYAMLByKey || {},
      dyffByKey: res.dyffByKey || {},
      dyffAvailable: !!res.dyffAvailable,
    };
  }
  return {
    compare: res || {resources:[]},
    sourceYAMLByKey: {},
    generatedYAMLByKey: {},
    dyffByKey: {},
    dyffAvailable: false,
  };
}
function compareStatusCounts(items){
  const c={all:items.length,equal:0,changed:0,missing_in_generated:0,extra_in_generated:0,mismatch:0};
  (items||[]).forEach(r=>{ if(c[r.status] != null) c[r.status]++; });
  c.mismatch = (c.changed||0) + (c.missing_in_generated||0) + (c.extra_in_generated||0);
  return c;
}
function setCompareFilter(v){
  compareFilter = v || 'all';
  renderCompareUI();
}
function setCompareHideEqual(v){
  compareHideEqual = !!v;
  renderCompareUI();
}
function setCompareOnlyMismatches(){
  compareHideEqual = true;
  compareFilter = 'all';
  renderCompareUI();
}
function filterCompareItems(items){
  let out = Array.isArray(items) ? items.slice() : [];
  if(compareFilter && compareFilter !== 'all'){
    if(compareFilter === 'mismatch') out = out.filter(r=>r.status !== 'equal');
    else out = out.filter(r=>r.status === compareFilter);
  }
  if(compareHideEqual) out = out.filter(r=>r.status !== 'equal');
  const q = (compareSearch||'').trim().toLowerCase();
  if(q){
    out = out.filter(r=>{
      const hay = [r.key, r.status, r.diffPath, r.sourceVal, r.genVal].map(x=>String(x||'').toLowerCase()).join('\n');
      return hay.includes(q);
    });
  }
  return out;
}
function applyPreviewFocus(mode){
  previewFocus = (mode || 'all');
  document.body.classList.remove('focus-editor','focus-compare','focus-render');
  if(previewFocus === 'editor') document.body.classList.add('focus-editor');
  if(previewFocus === 'compare') document.body.classList.add('focus-compare');
  if(previewFocus === 'render') document.body.classList.add('focus-render');
  byId('focusSeg')?.querySelectorAll('button[data-focus]').forEach(btn=>{
    btn.classList.toggle('active', (btn.getAttribute('data-focus')||'all') === previewFocus);
  });
}
function renderCompareUI(){
  const box = byId('compareBox');
  const detailsBox = byId('compareDetails');
  const payload = comparePayload;
  if(!payload){ box.innerHTML=''; detailsBox.innerHTML=''; return; }
  const cmp = payload.compare || {};
  const items = Array.isArray(cmp.resources) ? cmp.resources : [];
  const counts = compareStatusCounts(items);
  const visible = filterCompareItems(items);
  const rows = visible.map(r=>{
    const cls = r.status==='equal' ? 'cmp-equal' : (r.status==='changed' ? 'cmp-changed' : (r.status==='missing_in_generated' ? 'cmp-missing' : 'cmp-extra'));
    const details = r.status==='changed'
      ? ('<div><code>'+esc(r.diffPath||'')+'</code></div><div class="muted">src: '+esc(r.sourceVal||'')+'</div><div class="muted">gen: '+esc(r.genVal||'')+'</div>')
      : '';
    const sel = compareSelectedKey === r.key ? ' sel' : '';
    return '<tr class="cmp-row'+sel+'" data-cmp-key="'+escAttr(r.key)+'"><td class="'+cls+'">'+esc(r.status)+'</td><td><code>'+esc(r.key)+'</code>'+details+'</td></tr>';
  }).join('');
  const filterBtn = (id, label)=>'<button data-cmp-filter="'+id+'" class="'+(compareFilter===id?'active':'')+'">'+label+' ('+(counts[id]||0)+')</button>';
  box.innerHTML = '<details open><summary>'+esc(cmp.summary||'Comparison')+'</summary>' +
    '<div class="cmp-tools">'+
      '<button data-cmp-preset="only-mismatches" class="'+((compareHideEqual&&compareFilter==='all')?'active':'')+'">Only mismatches</button>'+
      '<button data-cmp-toggle="hide-equal" class="'+(compareHideEqual?'active':'')+'">Hide equal</button>'+
      '<input id="compareSearchInput" type="text" placeholder="Search resource / path / values" value="'+escAttr(compareSearch)+'">'+
      filterBtn('all','All')+
      filterBtn('changed','Changed')+
      filterBtn('missing_in_generated','Missing')+
      filterBtn('extra_in_generated','Extra')+
      filterBtn('equal','Equal')+
    '</div>' +
    ((rows && rows.length) ? ('<div class="cmp-table-wrap"><table class="cmp-table"><tr><th>Status</th><th>Resource</th></tr>'+rows+'</table></div>') : '<div class="muted">No items for current filter</div>') +
    '</details>';
  const selected = items.find(r=>r.key===compareSelectedKey) || visible[0] || items[0] || null;
  compareSelectedKey = selected ? selected.key : '';
  if(!selected){
    detailsBox.innerHTML = '';
    return;
  }
  renderCompareDetails(selected);
}
function renderCompareDetails(item){
  const detailsBox = byId('compareDetails');
  const src = (comparePayload && comparePayload.sourceYAMLByKey && comparePayload.sourceYAMLByKey[item.key]) || '';
  const gen = (comparePayload && comparePayload.generatedYAMLByKey && comparePayload.generatedYAMLByKey[item.key]) || '';
  const dyff = (comparePayload && comparePayload.dyffByKey && comparePayload.dyffByKey[item.key]) || '';
  const block = (title, y, emptyText, kind)=>'<div class="card" style="margin:0"><div class="cmp-panel-title">'+title+'</div>' +
    (y ? ('<pre class="yaml">'+renderCompareYamlLines(y, item, kind)+'</pre>')
       : ('<div class="muted">'+emptyText+'</div>')) + '</div>';
  const dyffBlock = item.status === 'changed'
    ? ('<div class="card" style="margin-top:8px"><div class="cmp-panel-title">dyff Diff</div>' +
       (dyff
         ? ('<pre class="yaml" style="max-height:320px;overflow:auto;white-space:pre">'+esc(dyff)+'</pre>')
         : ('<div class="muted">'+((comparePayload && comparePayload.dyffAvailable) ? 'dyff produced no output for this resource' : 'dyff binary not available')+'</div>')
       ) +
      '</div>')
    : '';
  detailsBox.innerHTML =
    '<div class="cmp-details-sticky">' +
      '<div style="font-weight:700;margin:6px 0 4px">Selected Resource</div>' +
      '<div class="muted"><code>'+esc(item.key)+'</code></div>' +
      '<div class="cmp-panels">' +
        block('Source chart render', src, 'No source resource (extra in generated)', 'source') +
        block('Library render', gen, 'No generated resource (missing in generated)', 'generated') +
      '</div>' + dyffBlock +
    '</div>';
}
function compareDiffHint(diffPath){
  const p = String(diffPath||'');
  if(!p || p === '$') return null;
  const noIdx = p.replace(/\[\d+\]/g,'');
  const parts = noIdx.split('.').filter(Boolean).filter(x=>x !== '$' && x !== 'length');
  if(!parts.length) return null;
  return {parts, leaf: parts[parts.length-1], parent: parts.length>1 ? parts[parts.length-2] : ''};
}
function renderCompareYamlLines(yamlText, item, side){
  const lines = String(yamlText||'').split('\n');
  const hint = item && item.status === 'changed' ? compareDiffHint(item.diffPath) : null;
  const wantedVal = side === 'source' ? String(item.sourceVal||'') : String(item.genVal||'');
  const lineMeta = lines.map((line)=>({
    raw: line,
    indent: ((line.match(/^ */)||[''])[0]||'').length,
    trimmed: line.trim(),
  }));
  let targetIdx = -1;
  if(hint && hint.leaf){
    // Try to locate exact path context by walking parent keys with indentation.
    let ctxIdx = -1;
    let ctxIndent = -1;
    for(let pi=0; pi<hint.parts.length-1; pi++){
      const key = hint.parts[pi];
      let found = -1;
      for(let i=(ctxIdx+1); i<lineMeta.length; i++){
        const m = lineMeta[i];
        if(!m.trimmed || m.trimmed.startsWith('#')) continue;
        if(ctxIdx >= 0 && m.indent <= ctxIndent) break;
        if(m.trimmed.startsWith(key+':')){
          found = i; ctxIndent = m.indent; break;
        }
      }
      if(found < 0){ ctxIdx = -1; break; }
      ctxIdx = found;
    }
    const start = ctxIdx >= 0 ? (ctxIdx + 1) : 0;
    const minIndent = ctxIdx >= 0 ? (ctxIndent + 1) : -1;
    for(let i=start; i<lineMeta.length; i++){
      const m = lineMeta[i];
      if(!m.trimmed || m.trimmed.startsWith('#')) continue;
      if(ctxIdx >= 0 && m.indent <= ctxIndent) break;
      if(m.trimmed.startsWith(hint.leaf+':')){
        if(minIndent < 0 || m.indent >= minIndent){
          targetIdx = i;
          break;
        }
      }
    }
    if(targetIdx < 0){
      // fallback to any line containing leaf key (handles dotted label keys under matchLabels)
      for(let i=0;i<lineMeta.length;i++){
        if(lineMeta[i].trimmed.startsWith(hint.leaf+':')){ targetIdx = i; break; }
      }
    }
  }
  if(targetIdx < 0 && wantedVal && wantedVal.length > 1 && wantedVal.length < 80){
    const plain = wantedVal.replace(/^\"|\"$/g,'');
    // Last resort: value match, but avoid common metadata lines first.
    let fallback = -1;
    for(let i=0;i<lineMeta.length;i++){
      const t = lineMeta[i].trimmed;
      if(!t || !lineMeta[i].raw.includes(plain)) continue;
      if(/^name:\s/.test(t) || /^namespace:\s/.test(t)) { if(fallback < 0) fallback = i; continue; }
      targetIdx = i; break;
    }
    if(targetIdx < 0) targetIdx = fallback;
  }
  return lines.map((line, idx)=>{
    let cls = 'yamlline';
    if(idx === targetIdx){
      cls += ' hl';
    }
    return '<span class="'+cls+'" title="'+escAttr(line)+'">'+yamlHtml(line)+'</span>';
  }).join('');
}
function jsonReq(method, url, body, timeoutMs){
  return fetchWithTimeout(url, {
    method,
    headers:{'Content-Type':'application/json'},
    body: body ? JSON.stringify(body) : undefined
  }, timeoutMs).then(async r=>{
    const data = await r.json().catch(()=>({}));
    if(!r.ok){
      throw new Error(data.error || (r.status+' '+r.statusText));
    }
    return data;
  });
}
function loadBaseModel(){
  return fetchWithTimeout('/api/model', null, 10000).then(r=>r.json()).then(m=>{
    renderPreviewFromModel(m);
    return m;
  });
}
function loadSourceValues(){
  return fetchWithTimeout('/api/source-values', null, 10000).then(async r=>{
    if(r.status === 404){
      setEditorStatus('Interactive values unavailable (manifests mode)', 'err');
      byId('runBtn').disabled = true;
      byId('saveBtn').disabled = true;
      return null;
    }
    const data = await r.json();
    initialValuesYAML = String(data.valuesYAML || '');
    setEditorValue(initialValuesYAML);
    byId('savePath').value = String(data.suggestedSavePath || '');
    setEditorStatus('Ready', 'ok');
    return data;
  });
}
function runExperiment(){
  const valuesYAML = getEditorValue();
  const valid = validateEditorYAML(true);
  if(!valid.ok){
    setExperimentStatus('Fix YAML errors before render', 'err');
    return;
  }
  setExperimentStatus('Running helm template + analysis...', '');
  byId('runBtn').disabled = true;
  jsonReq('POST', '/api/experiment/render', {valuesYAML}, 120000)
    .then(resp=>{
      renderPreviewFromModel(resp.model);
      renderDiff(resp.diff);
      setExperimentStatus('Rendered successfully', 'ok');
    })
    .catch(err=>{
      setExperimentStatus('Error: '+String(err), 'err');
    })
    .finally(()=>{ byId('runBtn').disabled = false; });
}
function saveValues(){
  const path = byId('savePath').value.trim();
  const valuesYAML = getEditorValue();
  if(!path){
    setEditorStatus('Save path is required', 'err');
    return;
  }
  setEditorStatus('Saving...', '');
  byId('saveBtn').disabled = true;
  jsonReq('POST', '/api/experiment/save', {path, valuesYAML}, 10000)
    .then(resp=>{
      setEditorStatus('Saved: '+(resp.path||path), 'ok');
    })
    .catch(err=>{
      setEditorStatus('Save error: '+String(err), 'err');
    })
    .finally(()=>{ byId('saveBtn').disabled = false; });
}
function runCompare(){
  const valuesYAML = getEditorValue();
  const valid = validateEditorYAML(true);
  if(!valid.ok){
    setCompareStatus('Fix YAML errors before compare', 'err');
    return;
  }
  setCompareStatus('Rendering source and library chart for comparison...', '');
  byId('compareBtn').disabled = true;
  jsonReq('POST', '/api/experiment/compare', {valuesYAML}, 180000)
    .then(res=>{
      renderCompare(res);
      const cmp = (res && res.compare) ? res.compare : res;
      setCompareStatus(cmp && cmp.equal ? 'Entity comparison: equivalent' : 'Entity comparison: mismatches found', (cmp && cmp.equal) ? 'ok' : 'err');
    })
    .catch(err=>{
      setCompareStatus('Compare error: '+String(err), 'err');
    })
    .finally(()=>{ byId('compareBtn').disabled = false; });
}
byId('runBtn').addEventListener('click', runExperiment);
byId('validateBtn').addEventListener('click', ()=>{ validateEditorYAML(true); });
byId('formatBtn').addEventListener('click', ()=>{
  const valid = validateEditorYAML(false);
  if(!valid.ok){
    setExperimentStatus('Cannot format invalid YAML', 'err');
    return;
  }
  if(!window.jsyaml){
    setEditorStatus('Formatter unavailable (js-yaml failed to load)', 'err');
    return;
  }
  try {
    const parsed = jsyaml.load(getEditorValue());
    const formatted = jsyaml.dump(parsed, {
      indent: 2,
      lineWidth: -1,
      noRefs: true,
      sortKeys: false,
    });
    setEditorValue(formatted);
    setEditorStatus('Formatted YAML (comments may be lost)', 'ok');
  } catch (e) {
    setAceYamlError(e);
    setEditorStatus('Format error: '+String(e.reason || e.message || e), 'err');
  }
});
byId('saveBtn').addEventListener('click', saveValues);
byId('compareBtn').addEventListener('click', runCompare);
byId('compareBox').addEventListener('click', (e)=>{
  const preset = e.target.closest('button[data-cmp-preset]');
  if(preset){
    if((preset.getAttribute('data-cmp-preset')||'') === 'only-mismatches') setCompareOnlyMismatches();
    return;
  }
  const toggle = e.target.closest('button[data-cmp-toggle]');
  if(toggle){
    if((toggle.getAttribute('data-cmp-toggle')||'') === 'hide-equal') setCompareHideEqual(!compareHideEqual);
    return;
  }
  const btn = e.target.closest('button[data-cmp-filter]');
  if(btn){
    setCompareFilter(btn.getAttribute('data-cmp-filter') || 'all');
    return;
  }
  const row = e.target.closest('tr[data-cmp-key]');
  if(row){
    compareSelectedKey = row.getAttribute('data-cmp-key') || '';
    renderCompareUI();
  }
});
byId('compareBox').addEventListener('input', (e)=>{
  const input = e.target.closest('#compareSearchInput');
  if(!input) return;
  compareSearch = input.value || '';
  renderCompareUI();
});
byId('focusSeg').addEventListener('click', (e)=>{
  const btn = e.target.closest('button[data-focus]');
  if(!btn) return;
  e.preventDefault();
  applyPreviewFocus(btn.getAttribute('data-focus') || 'all');
});
byId('paneRatio').addEventListener('input', (e)=>{
  previewUIPrefs.paneRatio = Number(e.target.value || 46);
  applyPreviewUIPrefs();
  savePreviewUIPrefs();
});
byId('yamlWrapBtn').addEventListener('click', ()=>{
  previewUIPrefs.yamlWrap = !previewUIPrefs.yamlWrap;
  applyPreviewUIPrefs();
  savePreviewUIPrefs();
});
byId('resetBtn').addEventListener('click', ()=>{
  setEditorValue(initialValuesYAML);
  setEditorStatus('Reset from chart values.yaml', 'ok');
});
initCodeEditor();
applyPreviewFocus('all');
loadPreviewUIPrefs();
applyPreviewUIPrefs();
loadPreviewFoldState();
Promise.all([loadBaseModel(), loadSourceValues()]).catch(err=>{
  byId('meta').textContent='Failed to load preview';
  byId('content').innerHTML='<pre class="yaml">'+esc(String(err))+'</pre>';
});
</script>
</body></html>`))
