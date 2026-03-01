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
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or_default().to_string();
    let path = first
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");

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

    let html = html_renderer();
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes()).map_err(|e| e.to_string())
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
        "panes": [
            {"title": "Source render", "content": source_yaml},
            {"title": "Generated values.yaml", "content": generated_values_yaml},
        ],
    });
    render_vue_page_html("happ inspect", &model.to_string())
}

pub fn render_compose_page_html(source_compose_yaml: &str, compose_report_yaml: &str, generated_values_yaml: &str) -> String {
    let model = serde_json::json!({
        "title": "happ compose-inspect",
        "panes": [
            {"title": "Source docker-compose", "content": source_compose_yaml},
            {"title": "Compose report", "content": compose_report_yaml},
            {"title": "Generated values.yaml", "content": generated_values_yaml},
        ],
    });
    render_vue_page_html("happ compose-inspect", &model.to_string())
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
.top {{ display:flex; gap:8px; align-items:center; margin-bottom:12px; flex-wrap:wrap; }}
.title {{ margin:0; flex:1; min-width:280px; }}
.toolbar {{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; }}
button {{ border:0; background:#0f172a; color:#fff; padding:8px 12px; border-radius:8px; cursor:pointer; }}
button.secondary {{ background:#374151; }}
input[type='text'] {{ border:1px solid #cbd5e1; border-radius:8px; padding:7px 10px; min-width:220px; }}
label.chk {{ display:flex; gap:6px; align-items:center; font-size:13px; }}
.grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(380px,1fr)); gap:12px; }}
.card {{ border:1px solid #d0dae8; border-radius:12px; padding:12px; background:#ffffffa8; }}
.cardhead {{ display:flex; align-items:center; justify-content:space-between; gap:8px; margin-bottom:8px; }}
.cardhead h3 {{ margin:0; font-size:16px; }}
.cardbtns {{ display:flex; gap:6px; }}
pre {{ background:#081126; color:#d8e6ff; padding:12px; border-radius:12px; overflow:auto; min-height:280px; margin:0; white-space:pre; font-size:13px; line-height:1.45; }}
pre.wrap {{ white-space:pre-wrap; word-break:break-word; }}
.muted {{ color:#475569; font-size:12px; }}
@media (prefers-color-scheme: dark) {{
 body {{ background:#0b1220; color:#dbe7ff; }}
 .card {{ border-color:#243247; background:#0f172ab3; }}
 input[type='text'] {{ border-color:#334155; background:#0f172a; color:#dbe7ff; }}
 .muted {{ color:#9fb0ca; }}
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
      <input type='text' v-model='query' placeholder='Search'/>
      <label class='chk'><input type='checkbox' v-model='wrapLines'/> Wrap lines</label>
      <label class='chk'>Font
        <input type='range' min='11' max='18' step='1' v-model.number='fontSize'/>
      </label>
      <button class='secondary' @click='expandAll'>Expand all</button>
      <button class='secondary' @click='collapseAll'>Collapse all</button>
      <button @click='exitUi'>Exit</button>
    </div>
  </div>
  <div class='muted' style='margin:0 0 10px 0;'>Settings are persisted in localStorage.</div>
  <div class='grid'>
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
</div>
<script>
window.__HAPP_MODEL__ = {};
const APP_STORE_KEY = 'happ.inspect.ui.v1';
const app = Vue.createApp({{
  data() {{
    return {{
      model: window.__HAPP_MODEL__ || {{ title: 'happ', panes: [] }},
      query: '',
      wrapLines: false,
      fontSize: 13,
      collapsedTitles: {{}},
    }};
  }},
  computed: {{
    filteredPanes() {{
      const q = (this.query || '').toLowerCase().trim();
      if(!q) return this.model.panes || [];
      return (this.model.panes || []).filter(p =>
        (p.title || '').toLowerCase().includes(q) || (p.content || '').toLowerCase().includes(q)
      );
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
      }}
    }} catch(_) {{}}
  }},
  watch: {{
    wrapLines: 'saveSettings',
    fontSize: 'saveSettings',
    collapsedTitles: {{ handler: 'saveSettings', deep: true }}
  }},
  methods: {{
    saveSettings() {{
      try {{
        localStorage.setItem(APP_STORE_KEY, JSON.stringify({{
          wrapLines: this.wrapLines,
          fontSize: this.fontSize,
          collapsedTitles: this.collapsedTitles
        }}));
      }} catch(_) {{}}
    }},
    paneKey(pane) {{ return pane.title || ''; }},
    isCollapsed(idx) {{
      const pane = this.filteredPanes[idx];
      return !!this.collapsedTitles[this.paneKey(pane)];
    }},
    togglePane(idx) {{
      const pane = this.filteredPanes[idx];
      const k = this.paneKey(pane);
      this.collapsedTitles[k] = !this.collapsedTitles[k];
    }},
    expandAll() {{
      this.collapsedTitles = {{}};
    }},
    collapseAll() {{
      const out = {{}};
      (this.filteredPanes || []).forEach(p => out[this.paneKey(p)] = true);
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
    }
}
