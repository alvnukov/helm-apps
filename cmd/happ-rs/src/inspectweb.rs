use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const VUE_GLOBAL_PROD_JS: &str = include_str!("../assets/vue.global.prod.js");
const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024 * 1024;

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
        let doc_mode = payload.get("docMode").and_then(|v| v.as_str()).unwrap_or("all");
        let doc_index = payload
            .get("docIndex")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize);
        let (ok, output) = match convert_payload(mode, input, doc_mode, doc_index) {
            Ok(v) => (true, v),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({ "ok": ok, "output": output }).to_string();
        return write_response(stream, 200, "application/json; charset=utf-8", resp.as_bytes()).map_err(|e| e.to_string());
    }
    if path == "/api/jq" && method == "POST" {
        let payload: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| format!("invalid JSON request: {e}"))?;
        let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or(".");
        let input = payload.get("input").and_then(|v| v.as_str()).unwrap_or_default();
        let doc_mode = payload.get("docMode").and_then(|v| v.as_str()).unwrap_or("first");
        let doc_index = payload
            .get("docIndex")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize);
        let compact = payload.get("compact").and_then(|v| v.as_bool()).unwrap_or(false);
        let raw_output = payload.get("rawOutput").and_then(|v| v.as_bool()).unwrap_or(false);
        let (ok, output) = match jq_payload(query, input, doc_mode, doc_index, compact, raw_output) {
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
        if data.len() > MAX_HTTP_REQUEST_BYTES {
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

fn convert_payload(mode: &str, input: &str, doc_mode: &str, doc_index: Option<usize>) -> Result<String, String> {
    match mode {
        "yaml-to-json" => {
            let docs = crate::yamlmerge::normalize_documents(input)
                .map_err(|e| format!("YAML parse error: {e}"))?;
            let json_docs: Vec<serde_json::Value> = docs
                .into_iter()
                .map(|y| serde_json::to_value(y).map_err(|e| format!("YAML->JSON conversion error: {e}")))
                .collect::<Result<_, _>>()?;
            match doc_mode.trim().to_ascii_lowercase().as_str() {
                "all" | "" => serde_json::to_string_pretty(&serde_json::Value::Array(json_docs))
                    .map_err(|e| format!("JSON format error: {e}")),
                "first" => {
                    let first = json_docs.into_iter().next().unwrap_or(serde_json::Value::Null);
                    serde_json::to_string_pretty(&first).map_err(|e| format!("JSON format error: {e}"))
                }
                "index" => {
                    let idx = doc_index.ok_or_else(|| "doc index is required for doc mode 'index'".to_string())?;
                    if idx >= json_docs.len() {
                        return Err(format!(
                            "doc index {} is out of range for {} document(s)",
                            idx,
                            json_docs.len()
                        ));
                    }
                    serde_json::to_string_pretty(&json_docs[idx]).map_err(|e| format!("JSON format error: {e}"))
                }
                other => Err(format!("unsupported doc mode: {other}")),
            }
        }
        "json-to-yaml" => {
            let j: serde_json::Value = serde_json::from_str(input).map_err(|e| format!("JSON parse error: {e}"))?;
            serde_yaml::to_string(&j).map_err(|e| format!("JSON->YAML conversion error: {e}"))
        }
        _ => Err("unsupported mode".to_string()),
    }
}

fn jq_payload(
    query: &str,
    input: &str,
    doc_mode: &str,
    doc_index: Option<usize>,
    compact: bool,
    raw_output: bool,
) -> Result<String, String> {
    let docs = crate::query::parse_input_docs_prefer_json(input).map_err(|e| format!("jq parse error: {e}"))?;
    let selected = select_docs_for_web(docs, doc_mode, doc_index, "jq")?;
    let out = crate::query::run_query_stream(query, selected).map_err(|e| format!("jq query error: {e}"))?;
    let mut lines = Vec::with_capacity(out.len());
    for v in out {
        if raw_output {
            if let Some(s) = v.as_str() {
                lines.push(s.to_string());
                continue;
            }
        }
        let line = if compact {
            serde_json::to_string(&v).map_err(|e| format!("jq output encode error: {e}"))?
        } else {
            serde_json::to_string_pretty(&v).map_err(|e| format!("jq output encode error: {e}"))?
        };
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

fn select_docs_for_web(
    docs: Vec<serde_json::Value>,
    doc_mode: &str,
    doc_index: Option<usize>,
    tool: &str,
) -> Result<Vec<serde_json::Value>, String> {
    match doc_mode.trim().to_ascii_lowercase().as_str() {
        "" | "first" => Ok(docs.into_iter().next().into_iter().collect()),
        "all" => Ok(docs),
        "index" => {
            let idx = doc_index.ok_or_else(|| format!("{tool}: doc index is required for doc mode 'index'"))?;
            if idx >= docs.len() {
                return Err(format!(
                    "{tool}: doc index {} is out of range for {} document(s)",
                    idx,
                    docs.len()
                ));
            }
            Ok(vec![docs[idx].clone()])
        }
        other => Err(format!(
            "{tool}: unsupported doc mode '{other}' (expected first|all|index)"
        )),
    }
}

fn write_response(stream: &mut TcpStream, code: u16, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        code,
        reason,
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
            },
            {
                "id": "jq-playground",
                "title": "jq Playground",
                "description": "Run jq queries on JSON or YAML input."
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
            },
            {
                "id": "jq-playground",
                "title": "jq Playground",
                "description": "Run jq queries on JSON or YAML input."
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
            },
            {
                "id": "jq-playground",
                "title": "jq Playground",
                "description": "Run jq queries on JSON or YAML input."
            }
        ]
    });
    render_vue_page_html("happ web tools", &model.to_string())
}

fn json_html_escape(s: &str) -> String {
    s.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

fn render_vue_page_html(page_title: &str, model_json: &str) -> String {
    let model_json = json_html_escape(model_json);
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
.jq-query-editor {{ position:relative; }}
.jq-query-highlight {{
  position:absolute; inset:0;
  margin:0;
  border:1px solid #cbd5e1;
  border-radius:12px;
  background:#f8fafc;
  color:#1e293b;
  padding:10px;
  overflow:auto;
  min-height:72px;
  white-space:pre-wrap;
  word-break:break-word;
  font-size:13px;
  line-height:1.45;
  z-index:1;
}}
.jq-query-input {{
  position:relative;
  z-index:2;
  background:transparent;
  color:transparent;
  caret-color:#0f172a;
  min-height:72px;
}}
.jq-token-keyword {{ color:#7c3aed; font-weight:700; }}
.jq-token-func {{ color:#0c4a6e; font-weight:700; }}
.jq-token-string {{ color:#0f766e; }}
.jq-token-number {{ color:#b45309; }}
.jq-token-op {{ color:#be123c; font-weight:700; }}
.jq-token-field {{ color:#1d4ed8; }}
.jq-suggest {{
  margin-top:6px;
  border:1px solid #cbd5e1;
  border-radius:10px;
  background:#ffffff;
  overflow:hidden;
}}
.jq-suggest-row {{
  display:flex;
  gap:8px;
  justify-content:space-between;
  align-items:flex-start;
  padding:7px 10px;
  cursor:pointer;
}}
.jq-suggest-row:hover,
.jq-suggest-row.active {{ background:#eff6ff; }}
.jq-suggest-label {{ font-weight:700; color:#0f172a; }}
.jq-suggest-desc {{ color:#475569; font-size:12px; margin-top:2px; }}
.jq-suggest-hint {{
  margin-top:6px;
  border:1px dashed #cbd5e1;
  border-radius:10px;
  padding:8px 10px;
  background:#f8fafc;
  font-size:12px;
  color:#334155;
}}
.err {{ color:#991b1b; font-weight:600; }}
@media (prefers-color-scheme: dark) {{
 body {{ background:#0b1220; color:#dbe7ff; }}
 .card {{ border-color:#243247; background:#0f172ab3; }}
 input[type='text'] {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 select {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 textarea {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 .jq-query-highlight {{ border-color:#334155; background:#0b1220; color:#dbe7ff; }}
 .jq-query-input {{ caret-color:#dbe7ff; }}
 .jq-token-keyword {{ color:#c4b5fd; }}
 .jq-token-func {{ color:#93c5fd; }}
 .jq-token-string {{ color:#5eead4; }}
 .jq-token-number {{ color:#fcd34d; }}
 .jq-token-op {{ color:#fda4af; }}
 .jq-token-field {{ color:#93c5fd; }}
 .jq-suggest {{ border-color:#334155; background:#0f172a; }}
 .jq-suggest-row:hover,
 .jq-suggest-row.active {{ background:#1e293b; }}
 .jq-suggest-label {{ color:#dbe7ff; }}
 .jq-suggest-desc {{ color:#9fb0ca; }}
 .jq-suggest-hint {{ border-color:#334155; background:#0b1220; color:#c7d6ef; }}
 .muted {{ color:#9fb0ca; }}
 .err {{ color:#fca5a5; }}
}}
</style>
<script src='/assets/vue.global.prod.js'></script>
<script id='happ-model' type='application/json'>{}</script>
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
            :class='{{ active: activeUtilityKey === u.id }}'
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

  <div v-else-if='activeUtilityKey === "converter"' class='card'>
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
      <select v-model='converterDocMode' :disabled='converterMode !== "yaml-to-json"'>
        <option value='all'>YAML docs: all</option>
        <option value='first'>YAML docs: first</option>
        <option value='index'>YAML docs: index</option>
      </select>
      <input v-if='converterMode === "yaml-to-json" && converterDocMode === "index"'
             v-model.number='converterDocIndex'
             type='number'
             min='0'
             step='1'
             style='width:140px;'
             placeholder='doc index' />
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

  <div v-else-if='activeUtilityKey === "jq-playground"' class='card'>
    <div class='cardhead'>
      <h3>jq Playground</h3>
      <div class='cardbtns'>
        <button class='secondary' @click='runJq'>Run</button>
        <button class='secondary' @click='clearJq'>Clear</button>
      </div>
    </div>
    <div class='converter-controls'>
      <select v-model='jqDocMode'>
        <option value='first'>Input docs: first</option>
        <option value='all'>Input docs: all</option>
        <option value='index'>Input docs: index</option>
      </select>
      <input v-if='jqDocMode === "index"'
             v-model.number='jqDocIndex'
             type='number'
             min='0'
             step='1'
             style='width:140px;'
             placeholder='doc index' />
      <label class='chk'><input type='checkbox' v-model='jqCompact'/> compact</label>
      <label class='chk'><input type='checkbox' v-model='jqRawOutput'/> raw output</label>
      <button class='secondary' @click='copyJqOutput'>Copy output</button>
      <div class='muted'>Live query execution is enabled</div>
    </div>
    <div style='margin-bottom:10px;'>
      <div class='muted' style='margin-bottom:6px;'>jq query (syntax highlighted)</div>
      <div class='jq-query-editor'>
        <pre class='jq-query-highlight' aria-hidden='true' v-html='jqQueryHighlighted'></pre>
        <textarea class='jq-query-input'
                  v-model='jqQuery'
                  spellcheck='false'
                  @input='onJqInput'
                  @click='updateJqSuggestState'
                  @keyup='updateJqSuggestState'
                  @keydown='onJqKeydown'
                  @blur='closeJqSuggestSoon'
                  @scroll='syncJqScroll'
                  ref='jqQueryInput'></textarea>
      </div>
      <div class='jq-suggest' v-if='jqSuggestOpen && jqSuggestions.length'>
        <div class='jq-suggest-row'
             v-for='(s, idx) in jqSuggestions'
             :key='s.label'
             :class='{{ active: idx === jqSuggestIndex }}'
             @mousedown.prevent='pickJqSuggestion(idx)'>
          <div>
            <div class='jq-suggest-label'>{{{{ s.label }}}}</div>
            <div class='jq-suggest-desc'>{{{{ s.desc }}}}</div>
          </div>
          <div class='muted'>{{{{ s.kind }}}}</div>
        </div>
      </div>
      <div class='jq-suggest-hint' v-if='jqSuggestOpen && jqSuggestions.length'>
        {{{{ jqActiveSuggestionHint }}}}
      </div>
    </div>
    <div class='conv-grid'>
      <div>
        <div class='muted' style='margin-bottom:6px;'>Input (JSON or YAML)</div>
        <textarea v-model='jqInput' spellcheck='false'></textarea>
      </div>
      <div>
        <div class='muted' style='margin-bottom:6px;'>Output</div>
        <textarea :value='jqOutput' readonly spellcheck='false'></textarea>
      </div>
    </div>
    <div class='err' v-if='jqError' style='margin-top:8px;'>{{{{ jqError }}}}</div>
  </div>
</div>
<script>
(() => {{
  const raw = document.getElementById('happ-model')?.textContent || '{{}}';
  try {{
    window.__HAPP_MODEL__ = JSON.parse(raw);
  }} catch(_) {{
    window.__HAPP_MODEL__ = {{}};
  }}
}})();
const APP_STORE_KEY = 'happ.inspect.ui.v6';
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
      converterDocMode: 'all',
      converterDocIndex: 0,
      converterInput: '',
      converterOutput: '',
      converterError: '',
      converterRequestSeq: 0,
      converterTimer: null,
      jqQuery: '.',
      jqInput: '',
      jqOutput: '',
      jqError: '',
      jqDocMode: 'first',
      jqDocIndex: 0,
      jqCompact: false,
      jqRawOutput: false,
      jqRequestSeq: 0,
      jqTimer: null,
      jqSuggestOpen: false,
      jqSuggestIndex: 0,
      jqCatalog: [
        {{ label:'select()', snippet:'select()', cursor:-1, kind:'filter', desc:'Filter stream by predicate.' }},
        {{ label:'map()', snippet:'map()', cursor:-1, kind:'transform', desc:'Apply expression to each array element.' }},
        {{ label:'contains()', snippet:'contains()', cursor:-1, kind:'predicate', desc:'Check container/string includes argument.' }},
        {{ label:'startswith()', snippet:'startswith()', cursor:-1, kind:'predicate', desc:'String starts with prefix.' }},
        {{ label:'endswith()', snippet:'endswith()', cursor:-1, kind:'predicate', desc:'String ends with suffix.' }},
        {{ label:'has()', snippet:'has()', cursor:-1, kind:'predicate', desc:'Object has key / array has index.' }},
        {{ label:'keys', snippet:'keys', cursor:0, kind:'function', desc:'Return object keys as array.' }},
        {{ label:'length', snippet:'length', cursor:0, kind:'function', desc:'Length of string/array/object.' }},
        {{ label:'type', snippet:'type', cursor:0, kind:'function', desc:'Type name: object/array/string/number/boolean/null.' }},
        {{ label:'tostring', snippet:'tostring', cursor:0, kind:'function', desc:'Convert value to string.' }},
        {{ label:'tonumber', snippet:'tonumber', cursor:0, kind:'function', desc:'Convert string/number to number.' }},
        {{ label:'values', snippet:'values', cursor:0, kind:'function', desc:'Values of object/array items.' }},
        {{ label:'add', snippet:'add', cursor:0, kind:'aggregate', desc:'Sum/concat array items.' }},
        {{ label:'sort', snippet:'sort', cursor:0, kind:'aggregate', desc:'Sort array values.' }},
        {{ label:'reverse', snippet:'reverse', cursor:0, kind:'aggregate', desc:'Reverse array/string.' }},
        {{ label:'min', snippet:'min', cursor:0, kind:'aggregate', desc:'Minimum array value.' }},
        {{ label:'max', snippet:'max', cursor:0, kind:'aggregate', desc:'Maximum array value.' }},
        {{ label:'index()', snippet:'index()', cursor:-1, kind:'search', desc:'Index of substring/element.' }},
        {{ label:'rindex()', snippet:'rindex()', cursor:-1, kind:'search', desc:'Last index of substring/element.' }},
        {{ label:'split()', snippet:'split()', cursor:-1, kind:'string', desc:'Split string by separator.' }},
        {{ label:'join()', snippet:'join()', cursor:-1, kind:'string', desc:'Join array by separator.' }},
        {{ label:'if then else end', snippet:'if  then  else  end', cursor:-14, kind:'flow', desc:'Conditional expression.' }},
        {{ label:'and', snippet:'and', cursor:0, kind:'operator', desc:'Logical AND in predicates.' }},
        {{ label:'or', snippet:'or', cursor:0, kind:'operator', desc:'Logical OR in predicates.' }},
        {{ label:'not', snippet:'not', cursor:0, kind:'operator', desc:'Logical negation.' }},
      ],
      converting: false,
    }};
  }},
  computed: {{
    activeUtilityKey() {{
      return this.currentUtility && this.currentUtility.id ? this.currentUtility.id : '';
    }},
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
    }},
    jqQueryHighlighted() {{
      return this.highlightJq(this.jqQuery || '');
    }},
    jqSuggestions() {{
      const token = (this.currentJqToken() || '').toLowerCase();
      if(!token) return this.jqCatalog.slice(0, 10);
      return this.jqCatalog
        .filter(x => x.label.toLowerCase().startsWith(token) || x.snippet.toLowerCase().startsWith(token))
        .slice(0, 10);
    }},
    jqActiveSuggestionHint() {{
      if(!this.jqSuggestions.length) return 'No suggestions';
      const idx = Math.min(this.jqSuggestIndex, this.jqSuggestions.length - 1);
      const s = this.jqSuggestions[idx];
      return s ? (s.label + ' — ' + s.desc) : 'No suggestions';
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
        if(s.converterDocMode) this.converterDocMode = s.converterDocMode;
        this.converterDocIndex = Number.isFinite(s.converterDocIndex) ? Number(s.converterDocIndex) : 0;
        this.converterInput = s.converterInput || '';
        this.jqQuery = s.jqQuery || '.';
        this.jqInput = s.jqInput || '';
        this.jqDocMode = s.jqDocMode || 'first';
        this.jqDocIndex = Number.isFinite(s.jqDocIndex) ? Number(s.jqDocIndex) : 0;
        this.jqCompact = !!s.jqCompact;
        this.jqRawOutput = !!s.jqRawOutput;
      }}
    }} catch(_) {{}}
    if(!(this.utilities || []).some(u => u.id === this.activeUtilityId)) {{
      this.activeUtilityId = (this.utilities[0] && this.utilities[0].id) || 'main';
    }}
    this.scheduleConvert();
    this.scheduleJqRun();
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
    converterDocMode() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    converterDocIndex() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    converterInput() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    jqQuery() {{
      this.saveSettings();
      this.scheduleJqRun();
    }},
    jqInput() {{
      this.saveSettings();
      this.scheduleJqRun();
    }},
    jqDocMode() {{
      this.saveSettings();
      this.scheduleJqRun();
    }},
    jqDocIndex() {{
      this.saveSettings();
      this.scheduleJqRun();
    }},
    jqCompact() {{
      this.saveSettings();
      this.scheduleJqRun();
    }},
    jqRawOutput() {{
      this.saveSettings();
      this.scheduleJqRun();
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
          converterDocMode: this.converterDocMode,
          converterDocIndex: this.converterDocIndex,
          converterInput: this.converterInput,
          jqQuery: this.jqQuery,
          jqInput: this.jqInput,
          jqDocMode: this.jqDocMode,
          jqDocIndex: this.jqDocIndex,
          jqCompact: this.jqCompact,
          jqRawOutput: this.jqRawOutput
        }}));
      }} catch(_) {{}}
    }},
    selectUtility(id) {{
      this.activeUtilityId = id;
      if(id !== 'jq-playground') this.jqSuggestOpen = false;
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
            docMode: this.converterDocMode,
            docIndex: this.converterDocMode === 'index' ? Number(this.converterDocIndex) : undefined,
            input: payload
          }})
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('convert API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.output || ('convert API HTTP ' + res.status));
        }}
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
    onJqInput() {{
      this.updateJqSuggestState();
    }},
    currentJqToken() {{
      const ta = this.$refs.jqQueryInput;
      const src = this.jqQuery || '';
      const pos = ta && Number.isFinite(ta.selectionStart) ? ta.selectionStart : src.length;
      const left = src.slice(0, pos);
      const m = left.match(/([A-Za-z_][A-Za-z0-9_]*)$/);
      return m ? m[1] : '';
    }},
    updateJqSuggestState() {{
      const token = this.currentJqToken();
      this.jqSuggestOpen = this.activeUtilityId === 'jq-playground' && (!!token || (this.jqQuery || '').trim() === '');
      this.jqSuggestIndex = 0;
    }},
    closeJqSuggestSoon() {{
      setTimeout(() => {{
        this.jqSuggestOpen = false;
      }}, 120);
    }},
    replaceCurrentJqToken(text, cursorFromEnd) {{
      const ta = this.$refs.jqQueryInput;
      const src = this.jqQuery || '';
      const pos = ta && Number.isFinite(ta.selectionStart) ? ta.selectionStart : src.length;
      const left = src.slice(0, pos);
      const m = left.match(/([A-Za-z_][A-Za-z0-9_]*)$/);
      const tokenLen = m ? m[1].length : 0;
      const start = pos - tokenLen;
      this.jqQuery = src.slice(0, start) + text + src.slice(pos);
      const base = start + text.length;
      const nextPos = Math.max(0, base + (cursorFromEnd || 0));
      this.$nextTick(() => {{
        const area = this.$refs.jqQueryInput;
        if(!area) return;
        area.focus();
        area.setSelectionRange(nextPos, nextPos);
        this.syncJqScroll();
      }});
    }},
    pickJqSuggestion(idx) {{
      if(!this.jqSuggestions.length) return;
      const i = Math.min(Math.max(0, idx), this.jqSuggestions.length - 1);
      const s = this.jqSuggestions[i];
      this.replaceCurrentJqToken(s.snippet, s.cursor || 0);
      this.jqSuggestOpen = false;
    }},
    onJqKeydown(e) {{
      if(!this.jqSuggestOpen || !this.jqSuggestions.length) {{
        if((e.ctrlKey || e.metaKey) && e.key === ' ') {{
          e.preventDefault();
          this.updateJqSuggestState();
          this.jqSuggestOpen = true;
        }}
        return;
      }}
      if(e.key === 'ArrowDown') {{
        e.preventDefault();
        this.jqSuggestIndex = (this.jqSuggestIndex + 1) % this.jqSuggestions.length;
        return;
      }}
      if(e.key === 'ArrowUp') {{
        e.preventDefault();
        this.jqSuggestIndex = (this.jqSuggestIndex - 1 + this.jqSuggestions.length) % this.jqSuggestions.length;
        return;
      }}
      if(e.key === 'Tab' || e.key === 'Enter') {{
        e.preventDefault();
        this.pickJqSuggestion(this.jqSuggestIndex);
        return;
      }}
      if(e.key === 'Escape') {{
        e.preventDefault();
        this.jqSuggestOpen = false;
      }}
    }},
    syncJqScroll() {{
      const ta = this.$refs.jqQueryInput;
      const pre = this.$el && this.$el.querySelector('.jq-query-highlight');
      if(!ta || !pre) return;
      pre.scrollTop = ta.scrollTop;
      pre.scrollLeft = ta.scrollLeft;
    }},
    highlightJq(src) {{
      const escapeHtml = (s) => s
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
      let out = escapeHtml(src);
      out = out.replace(/(\"(?:[^\"\\\\]|\\\\.)*\")/g, "<span class='jq-token-string'>$1</span>");
      out = out.replace(/\b(-?\d+(?:\.\d+)?)\b/g, "<span class='jq-token-number'>$1</span>");
      out = out.replace(/(\|\||\/\/|==|!=|>=|<=|[|,()[\]{{}}+\-*\/])/g, "<span class='jq-token-op'>$1</span>");
      out = out.replace(/\b(select|map|if|then|else|end|and|or|not|empty|contains|startswith|endswith|keys|length|type|tostring|tonumber|add|sort|reverse|min|max|values|has|index|rindex|split|join)\b/g, "<span class='jq-token-func'>$1</span>");
      out = out.replace(/(\.[A-Za-z0-9_\-]+)/g, "<span class='jq-token-field'>$1</span>");
      return out || ' ';
    }},
    async runJq() {{
      this.jqError = '';
      const input = this.jqInput || '';
      const reqId = ++this.jqRequestSeq;
      if(!input.trim()) {{
        this.jqOutput = '';
        return;
      }}
      try {{
        const res = await fetch('/api/jq', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            query: this.jqQuery || '.',
            input,
            docMode: this.jqDocMode,
            docIndex: this.jqDocMode === 'index' ? Number(this.jqDocIndex) : undefined,
            compact: this.jqCompact,
            rawOutput: this.jqRawOutput
          }})
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('jq API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.output || ('jq API HTTP ' + res.status));
        }}
        if(reqId !== this.jqRequestSeq) return;
        if(!data.ok) {{
          this.jqError = data.output || 'jq execution failed';
          this.jqOutput = '';
          return;
        }}
        this.jqOutput = data.output || '';
      }} catch(e) {{
        if(reqId !== this.jqRequestSeq) return;
        this.jqError = String(e);
        this.jqOutput = '';
      }}
    }},
    scheduleJqRun() {{
      if(this.jqTimer) {{
        clearTimeout(this.jqTimer);
      }}
      this.jqTimer = setTimeout(() => {{
        this.runJq();
      }}, 120);
    }},
    clearJq() {{
      this.jqInput = '';
      this.jqOutput = '';
      this.jqError = '';
      this.jqQuery = '.';
      this.jqSuggestOpen = false;
    }},
    async copyJqOutput() {{
      if(!this.jqOutput) return;
      try {{ await navigator.clipboard.writeText(this.jqOutput); }} catch(_) {{}}
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
        assert!(html.contains("jq Playground"));
        assert!(html.contains("/api/jq"));
        assert!(html.contains("jq-suggest"));
        assert!(html.contains("onJqKeydown"));
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
        let j = convert_payload("yaml-to-json", "a: 1\nb:\n  - x\n", "all", None).expect("yaml->json");
        assert!(j.contains("\"a\": 1"));
        let y = convert_payload("json-to-yaml", r#"{"a":1,"b":["x"]}"#, "all", None).expect("json->yaml");
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
        let j = convert_payload("yaml-to-json", input, "all", None).expect("yaml->json");
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
        let j = convert_payload("yaml-to-json", src, "all", None).expect("yaml->json");
        let v: serde_json::Value = serde_json::from_str(&j).expect("json");
        assert_eq!(v[0]["literal"], "line1\nline2");
        assert_eq!(v[0]["folded"], "a b");
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
        let as_json = convert_payload("yaml-to-json", src, "first", None).expect("yaml->json");
        let back_yaml = convert_payload("json-to-yaml", &as_json, "all", None).expect("json->yaml");

        let left: serde_yaml::Value = serde_yaml::from_str(src).expect("src yaml");
        let right: serde_yaml::Value = serde_yaml::from_str(&back_yaml).expect("roundtrip yaml");
        let left_norm = crate::yamlmerge::normalize_value(left);
        let right_norm = crate::yamlmerge::normalize_value(right);
        assert_eq!(left_norm, right_norm);
    }

    #[test]
    fn convert_payload_rejects_multi_document_yaml() {
        let src = "a: 1\n---\na: 2\n";
        let all = convert_payload("yaml-to-json", src, "all", None).expect("ok");
        let v: serde_json::Value = serde_json::from_str(&all).expect("json");
        assert_eq!(v.as_array().map(|x| x.len()), Some(2));
        let first = convert_payload("yaml-to-json", src, "first", None).expect("ok");
        let one: serde_json::Value = serde_json::from_str(&first).expect("json");
        assert_eq!(one["a"], 1);
    }

    #[test]
    fn convert_payload_supports_index_doc_mode() {
        let src = "a: 1\n---\na: 2\n---\na: 3\n";
        let at_1 = convert_payload("yaml-to-json", src, "index", Some(1)).expect("ok");
        let one: serde_json::Value = serde_json::from_str(&at_1).expect("json");
        assert_eq!(one["a"], 2);
    }

    #[test]
    fn convert_payload_rejects_missing_index_for_index_doc_mode() {
        let src = "a: 1\n---\na: 2\n";
        let err = convert_payload("yaml-to-json", src, "index", None).expect_err("error");
        assert!(err.contains("doc index is required"));
    }

    #[test]
    fn convert_payload_rejects_out_of_range_index_doc_mode() {
        let src = "a: 1\n---\na: 2\n";
        let err = convert_payload("yaml-to-json", src, "index", Some(5)).expect_err("error");
        assert!(err.contains("out of range"));
    }

    #[test]
    fn convert_payload_rejects_duplicate_keys_yaml() {
        let src = "a: 1\na: 2\n";
        let err = convert_payload("yaml-to-json", src, "all", None).expect_err("error");
        assert!(err.contains("YAML parse error"));
        assert!(err.to_lowercase().contains("duplicate"));
    }

    #[test]
    fn convert_payload_rejects_bad_mode() {
        let err = convert_payload("bad", "a: 1", "all", None).expect_err("error");
        assert!(err.contains("unsupported mode"));
    }

    #[test]
    fn convert_payload_rejects_bad_doc_mode() {
        let err = convert_payload("yaml-to-json", "a: 1\n", "weird", None).expect_err("error");
        assert!(err.contains("unsupported doc mode"));
    }

    #[test]
    fn jq_payload_runs_query_for_yaml_input() {
        let out = jq_payload(".apps[] | .name", "apps:\n  - name: a\n  - name: b\n", "first", None, false, true)
            .expect("jq");
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn jq_payload_supports_doc_index_mode() {
        let out = jq_payload(".a", "a: 1\n---\na: 2\n", "index", Some(1), false, false).expect("jq");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn jq_payload_rejects_bad_doc_mode() {
        let err = jq_payload(".", "a: 1\n", "weird", None, false, false).expect_err("error");
        assert!(err.contains("unsupported doc mode"));
    }

    #[test]
    fn jq_payload_rejects_out_of_range_doc_index() {
        let err = jq_payload(".", "a: 1\n", "index", Some(5), false, false).expect_err("error");
        assert!(err.contains("out of range"));
    }
}
