use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const VUE_GLOBAL_PROD_JS: &str = include_str!("../assets/vue.global.prod.js");

pub fn serve(addr: &str, open_browser: bool, source_yaml: String, generated_values_yaml: String) -> Result<(), String> {
    serve_with_renderer(
        addr,
        open_browser,
        Box::new(move || render_page_html(&source_yaml, &generated_values_yaml)),
        None,
    )
}

pub fn serve_tools(addr: &str, open_browser: bool) -> Result<(), String> {
    serve_with_renderer(
        addr,
        open_browser,
        Box::new(render_tools_page_html),
        None,
    )
}

pub fn serve_compose(
    addr: &str,
    open_browser: bool,
    source_compose_yaml: String,
    compose_report_yaml: String,
    generated_values_yaml: String,
) -> Result<(), String> {
    let source_for_api = source_compose_yaml.clone();
    let report_for_api = compose_report_yaml.clone();
    let values_for_api = generated_values_yaml.clone();
    serve_with_renderer(
        addr,
        open_browser,
        Box::new(move || render_compose_page_html(&source_compose_yaml, &compose_report_yaml, &generated_values_yaml)),
        Some(Box::new(move || {
            serde_json::json!({
                "source_compose": source_for_api,
                "compose_report": report_for_api,
                "values": values_for_api,
            })
            .to_string()
        })),
    )
}

fn serve_with_renderer(
    addr: &str,
    open_browser: bool,
    html_renderer: Box<dyn Fn() -> String>,
    json_renderer: Option<Box<dyn Fn() -> String>>,
) -> Result<(), String> {
    let listener = TcpListener::bind(addr).map_err(|e| format!("bind {addr}: {e}"))?;
    let running = Arc::new(AtomicBool::new(true));
    if open_browser {
        let _ = open_in_browser(&format!("http://{addr}"));
    }
    while running.load(Ordering::SeqCst) {
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(e) => return Err(format!("accept error: {e}")),
        };
        if let Err(e) = handle_connection(&mut stream, &running, &html_renderer, json_renderer.as_ref().map(|f| f.as_ref())) {
            let _ = write_response(&mut stream, 500, "text/plain; charset=utf-8", e.as_bytes());
        }
    }
    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    running: &Arc<AtomicBool>,
    html_renderer: &dyn Fn() -> String,
    json_renderer: Option<&dyn Fn() -> String>,
) -> Result<(), String> {
    let req = read_http_request(stream)?;
    let first = req.lines().next().unwrap_or_default().to_string();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");
    let body = http_body(&req);

    if path == "/exit" {
        running.store(false, Ordering::SeqCst);
        return write_response(stream, 200, "text/plain; charset=utf-8", b"shutting down").map_err(|e| e.to_string());
    }

    if path == "/api/model" {
        let body = match json_renderer {
            Some(render_json) => render_json(),
            None => serde_json::json!({}).to_string(),
        };
        return write_response(stream, 200, "application/json; charset=utf-8", body.as_bytes()).map_err(|e| e.to_string());
    }
    if path == "/assets/vue.global.prod.js" {
        return write_response(stream, 200, "application/javascript; charset=utf-8", VUE_GLOBAL_PROD_JS.as_bytes())
            .map_err(|e| e.to_string());
    }
    if path == "/api/convert" && method == "POST" {
        let payload: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| format!("invalid JSON request: {e}"))?;
        let mode = payload.get("mode").and_then(|v| v.as_str()).unwrap_or_default();
        let input = payload.get("input").and_then(|v| v.as_str()).unwrap_or_default();
        let (ok, output) = match convert_payload(mode, input) {
            Ok(v) => (true, v),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({ "ok": ok, "output": output }).to_string();
        return write_response(stream, 200, "application/json; charset=utf-8", resp.as_bytes()).map_err(|e| e.to_string());
    }

    let html = html_renderer();
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes()).map_err(|e| e.to_string())
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let mut header_end = None;
    let mut content_length = 0usize;

    loop {
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if header_end.is_none() {
            header_end = find_header_end(&data);
            if let Some(h_end) = header_end {
                let header = String::from_utf8_lossy(&data[..h_end]);
                content_length = parse_content_length(&header);
            }
        }
        if let Some(h_end) = header_end {
            let body_len = data.len().saturating_sub(h_end + 4);
            if body_len >= content_length {
                break;
            }
        }
        if data.len() > 5 * 1024 * 1024 {
            return Err("request too large".to_string());
        }
    }
    String::from_utf8(data).map_err(|e| e.to_string())
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_content_length(header: &str) -> usize {
    for line in header.lines() {
        if let Some(v) = line.strip_prefix("Content-Length:").or_else(|| line.strip_prefix("content-length:")) {
            return v.trim().parse::<usize>().unwrap_or(0);
        }
    }
    0
}

fn http_body(req: &str) -> &str {
    req.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("")
}

fn convert_payload(mode: &str, input: &str) -> Result<String, String> {
    match mode {
        "yaml-to-json" => {
            let y: serde_yaml::Value = serde_yaml::from_str(input).map_err(|e| format!("YAML parse error: {e}"))?;
            let y = crate::yamlmerge::normalize_value(y);
            let j = serde_json::to_value(y).map_err(|e| format!("YAML->JSON conversion error: {e}"))?;
            serde_json::to_string_pretty(&j).map_err(|e| format!("JSON format error: {e}"))
        }
        "json-to-yaml" => {
            let j: serde_json::Value = serde_json::from_str(input).map_err(|e| format!("JSON parse error: {e}"))?;
            serde_yaml::to_string(&j).map_err(|e| format!("JSON->YAML conversion error: {e}"))
        }
        _ => Err("unsupported mode".to_string()),
    }
}

fn write_response(stream: &mut TcpStream, code: u16, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        code,
        content_type,
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

pub fn render_page_html(source_yaml: &str, generated_values_yaml: &str) -> String {
    let model = serde_json::json!({
        "title": "happ inspect",
        "utilities": [
            {
                "id": "inspect",
                "title": "Inspect",
                "description": "Rendered manifests and generated values.",
                "panes": [
                    {"title": "Source render", "content": source_yaml},
                    {"title": "Generated values.yaml", "content": generated_values_yaml}
                ]
            },
            {
                "id": "converter",
                "title": "YAML/JSON Converter",
                "description": "Convert data between YAML and JSON formats."
            }
        ]
    });
    render_vue_page_html("happ inspect", &model.to_string())
}

pub fn render_compose_page_html(source_compose_yaml: &str, compose_report_yaml: &str, generated_values_yaml: &str) -> String {
    let model = serde_json::json!({
        "title": "happ compose-inspect",
        "utilities": [
            {
                "id": "compose-inspect",
                "title": "Compose Inspect",
                "description": "Compose source, analyzed report and generated values.",
                "panes": [
                    {"title": "Source docker-compose", "content": source_compose_yaml},
                    {"title": "Compose report", "content": compose_report_yaml},
                    {"title": "Generated values.yaml", "content": generated_values_yaml}
                ]
            },
            {
                "id": "converter",
                "title": "YAML/JSON Converter",
                "description": "Convert data between YAML and JSON formats."
            }
        ]
    });
    render_vue_page_html("happ compose-inspect", &model.to_string())
}

pub fn render_tools_page_html() -> String {
    let model = serde_json::json!({
        "title": "happ web tools",
        "utilities": [
            {
                "id": "converter",
                "title": "YAML/JSON Converter",
                "description": "Convert data between YAML and JSON formats."
            }
        ]
    });
    render_vue_page_html("happ web tools", &model.to_string())
}

fn json_script_escape(s: &str) -> String {
    s.replace("</script>", "<\\/script>")
}

fn render_vue_page_html(page_title: &str, model_json: &str) -> String {
    let model_json = json_script_escape(model_json);
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset='utf-8'/>
<title>{}</title>
<style>
:root {{ color-scheme: light dark; }}
body {{ font-family: ui-monospace, Menlo, monospace; margin:0; padding:16px; background:#f6f8fb; color:#0f172a; }}
#app {{ max-width: 1800px; margin: 0 auto; }}
.top {{ display:flex; gap:8px; align-items:center; margin-bottom:12px; flex-wrap:wrap; }}
.title {{ margin:0; flex:1; min-width:280px; }}
.toolbar {{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; }}
button {{ border:0; background:#0f172a; color:#fff; padding:8px 12px; border-radius:8px; cursor:pointer; }}
button.secondary {{ background:#374151; }}
button.tab {{ background:#334155; }}
button.tab.active {{ background:#0f172a; }}
input[type='text'] {{ border:1px solid #cbd5e1; border-radius:8px; padding:7px 10px; min-width:220px; }}
select {{ border:1px solid #cbd5e1; border-radius:8px; padding:7px 10px; min-width:220px; background:#fff; color:#0f172a; }}
textarea {{ border:1px solid #cbd5e1; border-radius:12px; padding:10px; min-height:240px; width:100%; font-family: ui-monospace, Menlo, monospace; font-size:13px; line-height:1.45; box-sizing:border-box; }}
label.chk {{ display:flex; gap:6px; align-items:center; font-size:13px; }}
.tabs {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 12px 0; }}
.util-head {{ margin:0 0 12px 0; }}
.grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(380px,1fr)); gap:12px; }}
.card {{ border:1px solid #d0dae8; border-radius:12px; padding:12px; background:#ffffffa8; }}
.cardhead {{ display:flex; align-items:center; justify-content:space-between; gap:8px; margin-bottom:8px; }}
.cardhead h3 {{ margin:0; font-size:16px; }}
.cardbtns {{ display:flex; gap:6px; }}
pre {{ background:#081126; color:#d8e6ff; padding:12px; border-radius:12px; overflow:auto; min-height:280px; margin:0; white-space:pre; font-size:13px; line-height:1.45; }}
pre.wrap {{ white-space:pre-wrap; word-break:break-word; }}
.muted {{ color:#475569; font-size:12px; }}
.conv-grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(420px,1fr)); gap:12px; }}
.converter-controls {{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; margin-bottom:10px; }}
.converter-controls .muted {{ margin-left:auto; }}
.err {{ color:#991b1b; font-weight:600; }}
@media (prefers-color-scheme: dark) {{
 body {{ background:#0b1220; color:#dbe7ff; }}
 .card {{ border-color:#243247; background:#0f172ab3; }}
 input[type='text'] {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 select {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 textarea {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 .muted {{ color:#9fb0ca; }}
 .err {{ color:#fca5a5; }}
}}
</style>
<script src='/assets/vue.global.prod.js'></script>
<script>window.__HAPP_MODEL__={{}};</script>
</head>
<body>
<div id='app'>
  <div class='top'>
    <h2 class='title'>{{{{ model.title }}}}</h2>
    <div class='toolbar'>
      <input v-if='activeHasPanes' type='text' v-model='query' placeholder='Search'/>
      <label class='chk'><input type='checkbox' v-model='wrapLines'/> Wrap lines</label>
      <label class='chk'>Font
        <input type='range' min='11' max='18' step='1' v-model.number='fontSize'/>
      </label>
      <button v-if='activeHasPanes' class='secondary' @click='expandAll'>Expand all</button>
      <button v-if='activeHasPanes' class='secondary' @click='collapseAll'>Collapse all</button>
      <button @click='exitUi'>Exit</button>
    </div>
  </div>
  <div class='tabs'>
    <button class='tab'
            :class='{{ active: activeUtilityId === u.id }}'
            v-for='u in utilities'
            :key='u.id'
            @click='selectUtility(u.id)'>{{{{ u.title }}}}</button>
  </div>
  <div class='util-head'>
    <div><strong>{{{{ currentUtility.title }}}}</strong></div>
    <div class='muted'>{{{{ currentUtility.description || "" }}}}</div>
  </div>
  <div class='muted' style='margin:0 0 10px 0;'>Settings are persisted in localStorage.</div>

  <div v-if='activeHasPanes' class='grid'>
    <div class='card' v-for='(pane, idx) in filteredPanes' :key='pane.title'>
      <div class='cardhead'>
        <h3>{{{{ pane.title }}}}</h3>
        <div class='cardbtns'>
          <button class='secondary' @click='togglePane(idx)'>{{{{ isCollapsed(idx) ? "Expand" : "Collapse" }}}}</button>
          <button class='secondary' @click='copyPane(pane)'>Copy</button>
          <button class='secondary' @click='downloadPane(pane)'>Download</button>
        </div>
      </div>
      <pre v-if='!isCollapsed(idx)' :class='{{ wrap: wrapLines }}' :style='{{ fontSize: fontSize + "px" }}'>{{{{ pane.content }}}}</pre>
    </div>
  </div>

  <div v-else class='card'>
    <div class='cardhead'>
      <h3>YAML ↔ JSON Converter</h3>
      <div class='cardbtns'>
        <button class='secondary' @click='swapConvertMode'>Swap</button>
        <button class='secondary' @click='clearConverter'>Clear</button>
      </div>
    </div>
    <div class='converter-controls'>
      <select v-model='converterMode'>
        <option value='yaml-to-json'>YAML → JSON</option>
        <option value='json-to-yaml'>JSON → YAML</option>
      </select>
      <button class='secondary' @click='copyConverterOutput'>Copy output</button>
      <div class='muted'>Live conversion is enabled</div>
    </div>
    <div class='conv-grid'>
      <div>
        <div class='muted' style='margin-bottom:6px;'>Input</div>
        <textarea v-model='converterInput' spellcheck='false'></textarea>
      </div>
      <div>
        <div class='muted' style='margin-bottom:6px;'>Output</div>
        <textarea :value='converterOutput' readonly spellcheck='false'></textarea>
      </div>
    </div>
    <div class='err' v-if='converterError' style='margin-top:8px;'>{{{{ converterError }}}}</div>
  </div>
</div>
<script>
window.__HAPP_MODEL__ = {};
const APP_STORE_KEY = 'happ.inspect.ui.v3';
const app = Vue.createApp({{
  data() {{
    const model = window.__HAPP_MODEL__ || {{ title: 'happ', utilities: [] }};
    const utilities = (model.utilities || []).length ? model.utilities : [{{ id: 'main', title: 'Main', panes: model.panes || [] }}];
    return {{
      model,
      utilities,
      activeUtilityId: utilities[0] ? utilities[0].id : 'main',
      query: '',
      wrapLines: false,
      fontSize: 13,
      collapsedTitles: {{}},
      converterMode: 'yaml-to-json',
      converterInput: '',
      converterOutput: '',
      converterError: '',
      converterRequestSeq: 0,
      converterTimer: null,
      converting: false,
    }};
  }},
  computed: {{
    currentUtility() {{
      const u = (this.utilities || []).find(x => x.id === this.activeUtilityId);
      return u || (this.utilities[0] || {{ id: 'main', title: 'Main', panes: [] }});
    }},
    activeHasPanes() {{
      return Array.isArray(this.currentUtility.panes);
    }},
    filteredPanes() {{
      const panes = this.currentUtility.panes || [];
      const q = (this.query || '').toLowerCase().trim();
      if(!q) return panes;
      return panes.filter(p =>
        (p.title || '').toLowerCase().includes(q) || (p.content || '').toLowerCase().includes(q)
      );
    }},
    converterModeLabel() {{
      return this.converterMode === 'yaml-to-json' ? 'YAML → JSON' : 'JSON → YAML';
    }}
  }},
  mounted() {{
    try {{
      const raw = localStorage.getItem(APP_STORE_KEY);
      if(raw) {{
        const s = JSON.parse(raw);
        this.wrapLines = !!s.wrapLines;
        this.fontSize = Number(s.fontSize || 13);
        this.collapsedTitles = s.collapsedTitles || {{}};
        if(s.activeUtilityId) this.activeUtilityId = s.activeUtilityId;
        if(s.converterMode) this.converterMode = s.converterMode;
        this.converterInput = s.converterInput || '';
      }}
    }} catch(_) {{}}
    this.scheduleConvert();
  }},
  watch: {{
    wrapLines: 'saveSettings',
    fontSize: 'saveSettings',
    collapsedTitles: {{ handler: 'saveSettings', deep: true }},
    activeUtilityId: 'saveSettings',
    converterMode() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    converterInput() {{
      this.saveSettings();
      this.scheduleConvert();
    }}
  }},
  methods: {{
    saveSettings() {{
      try {{
        localStorage.setItem(APP_STORE_KEY, JSON.stringify({{
          wrapLines: this.wrapLines,
          fontSize: this.fontSize,
          collapsedTitles: this.collapsedTitles,
          activeUtilityId: this.activeUtilityId,
          converterMode: this.converterMode,
          converterInput: this.converterInput
        }}));
      }} catch(_) {{}}
    }},
    selectUtility(id) {{
      this.activeUtilityId = id;
    }},
    paneKey(pane) {{ return pane.title || ''; }},
    paneKeyWithUtility(pane) {{ return this.activeUtilityId + '::' + this.paneKey(pane); }},
    isCollapsed(idx) {{
      const pane = this.filteredPanes[idx];
      return !!this.collapsedTitles[this.paneKeyWithUtility(pane)];
    }},
    togglePane(idx) {{
      const pane = this.filteredPanes[idx];
      const k = this.paneKeyWithUtility(pane);
      this.collapsedTitles[k] = !this.collapsedTitles[k];
    }},
    expandAll() {{
      const out = {{ ...this.collapsedTitles }};
      (this.filteredPanes || []).forEach(p => delete out[this.paneKeyWithUtility(p)]);
      this.collapsedTitles = out;
    }},
    collapseAll() {{
      const out = {{ ...this.collapsedTitles }};
      (this.filteredPanes || []).forEach(p => out[this.paneKeyWithUtility(p)] = true);
      this.collapsedTitles = out;
    }},
    async copyPane(pane) {{
      const txt = pane.content || '';
      try {{
        await navigator.clipboard.writeText(txt);
      }} catch(_) {{}}
    }},
    downloadPane(pane) {{
      const blob = new Blob([pane.content || ''], {{type:'text/plain;charset=utf-8'}});
      const a = document.createElement('a');
      const safe = (pane.title || 'pane').toLowerCase().replace(/[^a-z0-9._-]+/g, '-');
      a.href = URL.createObjectURL(blob);
      a.download = safe + '.yaml';
      a.click();
      URL.revokeObjectURL(a.href);
    }},
    async runConvert(mode) {{
      this.converterMode = mode || this.converterMode;
      this.converterError = '';
      const payload = this.converterInput || '';
      if(!payload.trim()) {{
        this.converterOutput = '';
        return;
      }}
      const reqId = ++this.converterRequestSeq;
      this.converting = true;
      try {{
        const res = await fetch('/api/convert', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            mode: this.converterMode,
            input: payload
          }})
        }});
        const data = await res.json();
        if(reqId !== this.converterRequestSeq) return;
        if(!data.ok) {{
          this.converterError = data.output || 'Conversion failed';
          this.converterOutput = '';
          return;
        }}
        this.converterOutput = data.output || '';
      }} catch(e) {{
        if(reqId !== this.converterRequestSeq) return;
        this.converterError = String(e);
        this.converterOutput = '';
      }} finally {{
        if(reqId === this.converterRequestSeq) {{
          this.converting = false;
        }}
      }}
    }},
    scheduleConvert() {{
      if(this.converterTimer) {{
        clearTimeout(this.converterTimer);
      }}
      this.converterTimer = setTimeout(() => {{
        this.runConvert();
      }}, 120);
    }},
    swapConvertMode() {{
      this.converterMode = this.converterMode === 'yaml-to-json' ? 'json-to-yaml' : 'yaml-to-json';
    }},
    clearConverter() {{
      this.converterInput = '';
      this.converterOutput = '';
      this.converterError = '';
    }},
    async copyConverterOutput() {{
      if(!this.converterOutput) return;
      try {{ await navigator.clipboard.writeText(this.converterOutput); }} catch(_) {{}}
    }},
    async exitUi() {{
      try {{ await fetch('/exit'); }} finally {{ window.close(); }}
    }}
  }}
}});
app.mount('#app');
</script>
</body>
</html>"#,
        page_title,
        model_json
    )
}

fn open_in_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())
            .map(|_| ())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())
            .map(|_| ())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .status()
            .map_err(|e| e.to_string())
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_contains_exit_button() {
        let html = render_page_html("a: 1", "global:\n  env: dev");
        assert!(html.contains("Exit"));
        assert!(html.contains("/exit"));
        assert!(html.contains("/assets/vue.global.prod.js"));
        assert!(html.contains("id='app'"));
        assert!(html.contains("Wrap lines"));
        assert!(html.contains("Copy"));
        assert!(html.contains("Download"));
        assert!(html.contains("YAML/JSON Converter"));
        assert!(html.contains("YAML → JSON"));
        assert!(html.contains("localStorage"));
    }

    #[test]
    fn compose_page_has_report_and_preview_sections() {
        let html = render_compose_page_html("services:\n  web: {}", "services:\n- name: web", "apps-stateless:\n  web: {}");
        assert!(html.contains("/assets/vue.global.prod.js"));
        assert!(html.contains("id='app'"));
        assert!(html.contains("window.__HAPP_MODEL__"));
        assert!(html.contains("Search"));
        assert!(html.contains("Wrap lines"));
        assert!(html.contains("Compose Inspect"));
    }

    #[test]
    fn convert_payload_yaml_to_json_and_back() {
        let j = convert_payload("yaml-to-json", "a: 1\nb:\n  - x\n").expect("yaml->json");
        assert!(j.contains("\"a\": 1"));
        let y = convert_payload("json-to-yaml", r#"{"a":1,"b":["x"]}"#).expect("json->yaml");
        assert!(y.contains("a: 1"));
        assert!(y.contains("- x"));
    }

    #[test]
    fn convert_payload_yaml_to_json_resolves_inline_merge() {
        let input = r#"
base: &base
  dummy: 42
obj:
  <<: { foo: 123, bar: 456 }
  baz: 999
"#;
        let j = convert_payload("yaml-to-json", input).expect("yaml->json");
        assert!(j.contains("\"foo\": 123"));
        assert!(j.contains("\"bar\": 456"));
        assert!(j.contains("\"baz\": 999"));
        assert!(!j.contains("\"<<\""));
    }

    #[test]
    fn convert_payload_yaml_block_and_folded_scalars_keep_semantics() {
        let src = r#"
literal: |-
  line1
  line2
folded: >-
  a
  b
"#;
        let j = convert_payload("yaml-to-json", src).expect("yaml->json");
        let v: serde_json::Value = serde_json::from_str(&j).expect("json");
        assert_eq!(v["literal"], "line1\nline2");
        assert_eq!(v["folded"], "a b");
    }

    #[test]
    fn convert_payload_roundtrip_preserves_data_model() {
        let src = r#"
a: 1
b:
  c: true
  d:
    - x
    - y
text: |-
  hello
  world
"#;
        let as_json = convert_payload("yaml-to-json", src).expect("yaml->json");
        let back_yaml = convert_payload("json-to-yaml", &as_json).expect("json->yaml");

        let left: serde_yaml::Value = serde_yaml::from_str(src).expect("src yaml");
        let right: serde_yaml::Value = serde_yaml::from_str(&back_yaml).expect("roundtrip yaml");
        let left_norm = crate::yamlmerge::normalize_value(left);
        let right_norm = crate::yamlmerge::normalize_value(right);
        assert_eq!(left_norm, right_norm);
    }

    #[test]
    fn convert_payload_rejects_multi_document_yaml() {
        let src = "a: 1\n---\na: 2\n";
        let err = convert_payload("yaml-to-json", src).expect_err("error");
        assert!(err.contains("YAML parse error"));
        assert!(err.to_lowercase().contains("more than one document"));
    }

    #[test]
    fn convert_payload_rejects_duplicate_keys_yaml() {
        let src = "a: 1\na: 2\n";
        let err = convert_payload("yaml-to-json", src).expect_err("error");
        assert!(err.contains("YAML parse error"));
        assert!(err.to_lowercase().contains("duplicate"));
    }

    #[test]
    fn convert_payload_rejects_bad_mode() {
        let err = convert_payload("bad", "a: 1").expect_err("error");
        assert!(err.contains("unsupported mode"));
    }
}
