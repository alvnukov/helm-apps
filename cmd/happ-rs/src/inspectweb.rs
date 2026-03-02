use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const VUE_GLOBAL_PROD_JS: &str = include_str!("../assets/vue.global.prod.js");
const CODEMIRROR_BUNDLE_JS: &str = include_str!("../assets/codemirror.bundle.js");
const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024 * 1024;

pub fn serve(
    addr: &str,
    open_browser: bool,
    source_yaml: String,
    generated_values_yaml: String,
) -> Result<(), String> {
    serve_with_renderer(
        addr,
        open_browser,
        Box::new(move || render_page_html(&source_yaml, &generated_values_yaml)),
        None,
    )
}

pub fn serve_tools(
    addr: &str,
    open_browser: bool,
    stdin_text: Option<String>,
) -> Result<(), String> {
    serve_with_renderer(
        addr,
        open_browser,
        Box::new(move || render_tools_page_html(stdin_text.as_deref())),
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
        Box::new(move || {
            render_compose_page_html(
                &source_compose_yaml,
                &compose_report_yaml,
                &generated_values_yaml,
            )
        }),
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
        let url = format!("http://{addr}");
        std::thread::spawn(move || {
            let _ = open_in_browser(&url);
        });
    }
    while running.load(Ordering::SeqCst) {
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(e) => return Err(format!("accept error: {e}")),
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(220)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        if let Err(e) = handle_connection(
            &mut stream,
            &running,
            &html_renderer,
            json_renderer.as_ref().map(|f| f.as_ref()),
        ) {
            if e == "read timeout" || e == "request closed before headers" {
                continue;
            }
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
    let route_path = path.split('?').next().unwrap_or(path);
    let body = http_body(&req);

    if route_path == "/exit" {
        running.store(false, Ordering::SeqCst);
        return write_response(stream, 200, "text/plain; charset=utf-8", b"shutting down")
            .map_err(|e| e.to_string());
    }

    if route_path == "/api/model" {
        let body = match json_renderer {
            Some(render_json) => render_json(),
            None => serde_json::json!({}).to_string(),
        };
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            body.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/assets/vue.global.prod.js" {
        return write_response(
            stream,
            200,
            "application/javascript; charset=utf-8",
            VUE_GLOBAL_PROD_JS.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/assets/codemirror.bundle.js" {
        return write_response(
            stream,
            200,
            "application/javascript; charset=utf-8",
            CODEMIRROR_BUNDLE_JS.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/convert" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let mode = payload
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let input = payload
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let doc_mode = payload
            .get("docMode")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let doc_index = payload
            .get("docIndex")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize);
        let (ok, output) = match convert_payload(mode, input, doc_mode, doc_index) {
            Ok(v) => (true, v),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({ "ok": ok, "output": output }).to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/jq" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or(".");
        let input = payload
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let doc_mode = payload
            .get("docMode")
            .and_then(|v| v.as_str())
            .unwrap_or("first");
        let doc_index = payload
            .get("docIndex")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize);
        let compact = payload
            .get("compact")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let raw_output = payload
            .get("rawOutput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let (ok, output) = match jq_payload(query, input, doc_mode, doc_index, compact, raw_output)
        {
            Ok(v) => (true, v),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({ "ok": ok, "output": output }).to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/yq" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or(".");
        let input = payload
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let doc_mode = payload
            .get("docMode")
            .and_then(|v| v.as_str())
            .unwrap_or("first");
        let doc_index = payload
            .get("docIndex")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize);
        let compact = payload
            .get("compact")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let raw_output = payload
            .get("rawOutput")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let (ok, output) = match yq_payload(query, input, doc_mode, doc_index, compact, raw_output)
        {
            Ok(v) => (true, v),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({ "ok": ok, "output": output }).to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/dyff" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let from = payload
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let to = payload
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let ignore_order = payload
            .get("ignoreOrder")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let ignore_whitespace = payload
            .get("ignoreWhitespace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let (ok, output) = match dyff_payload(from, to, ignore_order, ignore_whitespace) {
            Ok(v) => (true, v),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({ "ok": ok, "output": output }).to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/semantic-map" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let source = payload
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let output = payload
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let source_kind = payload
            .get("sourceKind")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let output_kind = payload
            .get("outputKind")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let from_utf16 = payload
            .get("from")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize)
            .unwrap_or(0);
        let to_utf16 = payload
            .get("to")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize)
            .unwrap_or(from_utf16);
        let selected_text = payload
            .get("selectedText")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let path_hint: Vec<String> = payload
            .get("pathHint")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let (ok, ranges, message) = match semantic_map_payload(
            source,
            output,
            source_kind,
            output_kind,
            from_utf16,
            to_utf16,
            selected_text,
            &path_hint,
        ) {
            Ok(r) => (true, r, String::new()),
            Err(e) => (false, Vec::<serde_json::Value>::new(), e),
        };
        let resp = serde_json::json!({
            "ok": ok,
            "ranges": ranges,
            "message": message
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/import" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let source_type = payload
            .get("sourceType")
            .and_then(|v| v.as_str())
            .unwrap_or("chart");
        let path = payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let env = payload.get("env").and_then(|v| v.as_str()).unwrap_or("dev");
        let group_name = payload
            .get("groupName")
            .and_then(|v| v.as_str())
            .unwrap_or("apps-k8s-manifests");
        let group_type = payload
            .get("groupType")
            .and_then(|v| v.as_str())
            .unwrap_or("apps-k8s-manifests");
        let import_strategy = payload
            .get("importStrategy")
            .and_then(|v| v.as_str())
            .unwrap_or("helpers");
        let release_name = payload
            .get("releaseName")
            .and_then(|v| v.as_str())
            .unwrap_or("imported");
        let namespace = payload
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let min_include_bytes = payload
            .get("minIncludeBytes")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize)
            .unwrap_or(24);
        let include_status = payload
            .get("includeStatus")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let values_files = payload_string_list(&payload, "valuesFiles");
        let set_values = payload_string_list(&payload, "setValues");
        let set_string_values = payload_string_list(&payload, "setStringValues");
        let set_file_values = payload_string_list(&payload, "setFileValues");
        let set_json_values = payload_string_list(&payload, "setJsonValues");
        let api_versions = payload_string_list(&payload, "apiVersions");
        let kube_version = payload
            .get("kubeVersion")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let include_crds = payload
            .get("includeCrds")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let chart_values_yaml = payload
            .get("chartValuesYaml")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let (ok, values_yaml, message, source_count) = match import_payload(
            source_type,
            path,
            env,
            group_name,
            group_type,
            import_strategy,
            release_name,
            namespace,
            min_include_bytes,
            include_status,
            values_files,
            set_values,
            set_string_values,
            set_file_values,
            set_json_values,
            kube_version,
            api_versions,
            include_crds,
            chart_values_yaml,
        ) {
            Ok((values, msg, cnt)) => (true, values, msg, cnt),
            Err(e) => (false, String::new(), e, 0usize),
        };
        let resp = serde_json::json!({
            "ok": ok,
            "valuesYaml": values_yaml,
            "message": message,
            "sourceCount": source_count,
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/compare-renders" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let source_type = payload
            .get("sourceType")
            .and_then(|v| v.as_str())
            .unwrap_or("chart");
        let path = payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let env = payload.get("env").and_then(|v| v.as_str()).unwrap_or("dev");
        let group_name = payload
            .get("groupName")
            .and_then(|v| v.as_str())
            .unwrap_or("apps-k8s-manifests");
        let group_type = payload
            .get("groupType")
            .and_then(|v| v.as_str())
            .unwrap_or("apps-k8s-manifests");
        let import_strategy = payload
            .get("importStrategy")
            .and_then(|v| v.as_str())
            .unwrap_or("helpers");
        let release_name = payload
            .get("releaseName")
            .and_then(|v| v.as_str())
            .unwrap_or("imported");
        let namespace = payload
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let min_include_bytes = payload
            .get("minIncludeBytes")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize)
            .unwrap_or(24);
        let include_status = payload
            .get("includeStatus")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let values_files = payload_string_list(&payload, "valuesFiles");
        let set_values = payload_string_list(&payload, "setValues");
        let set_string_values = payload_string_list(&payload, "setStringValues");
        let set_file_values = payload_string_list(&payload, "setFileValues");
        let set_json_values = payload_string_list(&payload, "setJsonValues");
        let api_versions = payload_string_list(&payload, "apiVersions");
        let kube_version = payload
            .get("kubeVersion")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let include_crds = payload
            .get("includeCrds")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let chart_values_yaml = payload
            .get("chartValuesYaml")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let generated_values_yaml = payload
            .get("valuesYaml")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let library_chart_path = payload
            .get("libraryChartPath")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());

        let (ok, equal, summary, message, source_count, generated_count) =
            match compare_render_payload(
                source_type,
                path,
                env,
                group_name,
                group_type,
                import_strategy,
                release_name,
                namespace,
                min_include_bytes,
                include_status,
                values_files,
                set_values,
                set_string_values,
                set_file_values,
                set_json_values,
                kube_version,
                api_versions,
                include_crds,
                chart_values_yaml,
                generated_values_yaml,
                library_chart_path.as_deref(),
            ) {
                Ok((eq, sum, src_cnt, gen_cnt)) => (
                    true,
                    eq,
                    sum.clone(),
                    if eq {
                        format!("Render compare OK: {sum}")
                    } else {
                        format!("Render compare mismatch: {sum}")
                    },
                    src_cnt,
                    gen_cnt,
                ),
                Err(e) => (false, false, String::new(), e, 0usize, 0usize),
            };
        let resp = serde_json::json!({
            "ok": ok,
            "equal": equal,
            "summary": summary,
            "message": message,
            "sourceCount": source_count,
            "generatedCount": generated_count,
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/save-chart" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let source_type = payload
            .get("sourceType")
            .and_then(|v| v.as_str())
            .unwrap_or("chart");
        let source_path = payload
            .get("sourcePath")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let out_chart_dir = payload
            .get("outChartDir")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let chart_name = payload
            .get("chartName")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let library_chart_path = payload
            .get("libraryChartPath")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let values_yaml = payload
            .get("valuesYaml")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let (ok, message) = match save_chart_payload(
            source_type,
            source_path,
            out_chart_dir,
            chart_name.as_deref(),
            library_chart_path.as_deref(),
            values_yaml,
        ) {
            Ok(msg) => (true, msg),
            Err(e) => (false, e),
        };
        let resp = serde_json::json!({
            "ok": ok,
            "message": message,
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/chart-values" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let chart_path = payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let (ok, values_yaml, message) = match load_chart_values_from_path(chart_path) {
            Ok(v) => (true, v, String::new()),
            Err(e) => (false, String::new(), e),
        };
        let resp = serde_json::json!({
            "ok": ok,
            "valuesYaml": values_yaml,
            "message": message
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/import-upload" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let source_type = payload
            .get("sourceType")
            .and_then(|v| v.as_str())
            .unwrap_or("chart");
        let env = payload.get("env").and_then(|v| v.as_str()).unwrap_or("dev");
        let group_name = payload
            .get("groupName")
            .and_then(|v| v.as_str())
            .unwrap_or("apps-k8s-manifests");
        let group_type = payload
            .get("groupType")
            .and_then(|v| v.as_str())
            .unwrap_or("apps-k8s-manifests");
        let import_strategy = payload
            .get("importStrategy")
            .and_then(|v| v.as_str())
            .unwrap_or("helpers");
        let release_name = payload
            .get("releaseName")
            .and_then(|v| v.as_str())
            .unwrap_or("imported");
        let namespace = payload
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let min_include_bytes = payload
            .get("minIncludeBytes")
            .and_then(|v| v.as_u64())
            .map(|x| x as usize)
            .unwrap_or(24);
        let include_status = payload
            .get("includeStatus")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let values_files = payload_string_list(&payload, "valuesFiles");
        let set_values = payload_string_list(&payload, "setValues");
        let set_string_values = payload_string_list(&payload, "setStringValues");
        let set_file_values = payload_string_list(&payload, "setFileValues");
        let set_json_values = payload_string_list(&payload, "setJsonValues");
        let api_versions = payload_string_list(&payload, "apiVersions");
        let kube_version = payload
            .get("kubeVersion")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());
        let include_crds = payload
            .get("includeCrds")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let chart_values_yaml = payload
            .get("chartValuesYaml")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|x| !x.trim().is_empty());

        let files = payload
            .get("files")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "files array is required for upload import".to_string())?;

        let tmp_root = create_upload_temp_dir()?;
        let import_path = match write_uploaded_files(&tmp_root, source_type, files) {
            Ok(p) => p,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp_root);
                return Err(e);
            }
        };

        let (ok, values_yaml, message, source_count) = match import_payload(
            source_type,
            &import_path.to_string_lossy(),
            env,
            group_name,
            group_type,
            import_strategy,
            release_name,
            namespace,
            min_include_bytes,
            include_status,
            values_files,
            set_values,
            set_string_values,
            set_file_values,
            set_json_values,
            kube_version,
            api_versions,
            include_crds,
            chart_values_yaml,
        ) {
            Ok((values, msg, cnt)) => (true, values, msg, cnt),
            Err(e) => (false, String::new(), e, 0usize),
        };
        let _ = std::fs::remove_dir_all(&tmp_root);
        let resp = serde_json::json!({
            "ok": ok,
            "valuesYaml": values_yaml,
            "message": message,
            "sourceCount": source_count,
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }
    if route_path == "/api/fs-list" && method == "POST" {
        let payload: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("invalid JSON request: {e}"))?;
        let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let (ok, current, parent, entries, message) = match list_fs_entries(path) {
            Ok((current, parent, entries)) => (true, current, parent, entries, String::new()),
            Err(e) => (false, String::new(), String::new(), Vec::new(), e),
        };
        let resp = serde_json::json!({
            "ok": ok,
            "path": current,
            "parent": parent,
            "entries": entries,
            "message": message
        })
        .to_string();
        return write_response(
            stream,
            200,
            "application/json; charset=utf-8",
            resp.as_bytes(),
        )
        .map_err(|e| e.to_string());
    }

    let html = html_renderer();
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes())
        .map_err(|e| e.to_string())
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let mut header_end = None;
    let mut content_length = 0usize;

    loop {
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if header_end.is_some() {
                    break;
                }
                return Err("read timeout".to_string());
            }
            Err(e) => return Err(e.to_string()),
        };
        if n == 0 {
            if data.is_empty() {
                return Err("request closed before headers".to_string());
            }
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
    if data.is_empty() {
        return Err("request closed before headers".to_string());
    }
    String::from_utf8(data).map_err(|e| e.to_string())
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_content_length(header: &str) -> usize {
    for line in header.lines() {
        if let Some(v) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            return v.trim().parse::<usize>().unwrap_or(0);
        }
    }
    0
}

fn http_body(req: &str) -> &str {
    req.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("")
}

fn payload_string_list(payload: &serde_json::Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn create_upload_temp_dir() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("happ-upload-{nanos}"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    Ok(dir)
}

fn sanitize_relative_path(p: &str) -> Result<PathBuf, String> {
    let path = Path::new(p);
    if path.is_absolute() {
        return Err(format!("absolute path is not allowed in upload: {p}"));
    }
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::Normal(v) => out.push(v),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(format!("parent path '..' is not allowed in upload: {p}"));
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(format!("invalid path component in upload: {p}"));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err("empty upload path".to_string());
    }
    Ok(out)
}

fn list_fs_entries(input_path: &str) -> Result<(String, String, Vec<serde_json::Value>), String> {
    let current = if input_path.trim().is_empty() {
        std::env::current_dir().map_err(|e| format!("current_dir: {e}"))?
    } else {
        PathBuf::from(input_path)
    };
    let current = current
        .canonicalize()
        .map_err(|e| format!("resolve path '{}': {e}", current.display()))?;
    if !current.is_dir() {
        return Err(format!("'{}' is not a directory", current.display()));
    }
    let parent = current
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut entries = Vec::new();
    for ent in
        std::fs::read_dir(&current).map_err(|e| format!("read_dir '{}': {e}", current.display()))?
    {
        let ent = ent.map_err(|e| format!("read_dir entry '{}': {e}", current.display()))?;
        let path = ent.path();
        let name = ent.file_name().to_string_lossy().to_string();
        let ty = ent
            .file_type()
            .map_err(|e| format!("file_type '{}': {e}", path.display()))?;
        entries.push(serde_json::json!({
            "name": name,
            "path": path.to_string_lossy(),
            "isDir": ty.is_dir(),
        }));
    }
    entries.sort_by(|a, b| {
        let ad = a.get("isDir").and_then(|v| v.as_bool()).unwrap_or(false);
        let bd = b.get("isDir").and_then(|v| v.as_bool()).unwrap_or(false);
        if ad != bd {
            return bd.cmp(&ad);
        }
        let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        an.cmp(bn)
    });
    Ok((current.to_string_lossy().to_string(), parent, entries))
}

fn load_chart_values_from_path(chart_path: &str) -> Result<String, String> {
    if chart_path.trim().is_empty() {
        return Err("chart path is required".to_string());
    }
    let root = PathBuf::from(chart_path)
        .canonicalize()
        .map_err(|e| format!("resolve chart path '{}': {e}", chart_path))?;
    if !root.is_dir() {
        return Err(format!(
            "chart path '{}' is not a directory",
            root.display()
        ));
    }
    let values_path = root.join("values.yaml");
    let content = std::fs::read_to_string(&values_path)
        .map_err(|e| format!("read '{}': {e}", values_path.display()))?;
    Ok(content)
}

fn write_uploaded_files(
    tmp_root: &Path,
    source_type: &str,
    files: &[serde_json::Value],
) -> Result<PathBuf, String> {
    if files.is_empty() {
        return Err("no files selected".to_string());
    }
    let mut compose_file: Option<PathBuf> = None;
    for item in files {
        let rel = item
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "upload file.path is required".to_string())?;
        let b64 = item
            .get("contentB64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "upload file.contentB64 is required".to_string())?;
        let safe_rel = sanitize_relative_path(rel)?;
        let full = tmp_root.join(&safe_rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create upload parent dir: {e}"))?;
        }
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .map_err(|e| format!("base64 decode for '{rel}': {e}"))?;
        std::fs::write(&full, bytes).map_err(|e| format!("write upload file '{rel}': {e}"))?;
        if source_type.eq_ignore_ascii_case("compose") {
            let name = rel.to_ascii_lowercase();
            if name.ends_with(".yml") || name.ends_with(".yaml") {
                if compose_file.is_none() {
                    compose_file = Some(full.clone());
                }
            }
        }
    }
    if source_type.eq_ignore_ascii_case("compose") {
        compose_file
            .ok_or_else(|| "compose upload requires at least one .yml/.yaml file".to_string())
    } else {
        Ok(tmp_root.to_path_buf())
    }
}

#[allow(clippy::too_many_arguments)]
fn import_payload(
    source_type: &str,
    path: &str,
    env: &str,
    group_name: &str,
    group_type: &str,
    import_strategy: &str,
    release_name: &str,
    namespace: Option<String>,
    min_include_bytes: usize,
    include_status: bool,
    values_files: Vec<String>,
    set_values: Vec<String>,
    set_string_values: Vec<String>,
    set_file_values: Vec<String>,
    set_json_values: Vec<String>,
    kube_version: Option<String>,
    api_versions: Vec<String>,
    include_crds: bool,
    chart_values_yaml: Option<String>,
) -> Result<(String, String, usize), String> {
    if path.trim().is_empty() {
        return Err("path is required".to_string());
    }
    let mut values_files = values_files;
    let mut inline_chart_values_temp: Option<PathBuf> = None;
    if let Some(inline) = chart_values_yaml {
        let tmp_root = create_upload_temp_dir()?;
        let fp = tmp_root.join("chart-inline-values.yaml");
        std::fs::write(&fp, inline).map_err(|e| format!("write inline chart values: {e}"))?;
        values_files.insert(0, fp.to_string_lossy().to_string());
        inline_chart_values_temp = Some(tmp_root);
    }
    let args = crate::cli::ImportArgs {
        path: path.to_string(),
        env: env.to_string(),
        group_name: group_name.to_string(),
        group_type: group_type.to_string(),
        min_include_bytes,
        include_status,
        output: None,
        out_chart_dir: None,
        chart_name: None,
        library_chart_path: None,
        import_strategy: import_strategy.to_string(),
        verify_equivalence: false,
        release_name: release_name.to_string(),
        namespace,
        values_files,
        set_values,
        set_string_values,
        set_file_values,
        set_json_values,
        kube_version,
        api_versions,
        include_crds,
        write_rendered_output: None,
    };
    let result = match source_type.trim().to_ascii_lowercase().as_str() {
        "chart" => {
            let docs = crate::source::load_documents_for_chart(&args)
                .map_err(|e| format!("chart load error: {e}"))?;
            let values = crate::convert::build_values(&args, &docs)
                .map_err(|e| format!("convert error: {e}"))?;
            let out = crate::output::values_yaml(&values)
                .map_err(|e| format!("values encode error: {e}"))?;
            Ok((
                out,
                format!("Imported {} rendered document(s) from chart.", docs.len()),
                docs.len(),
            ))
        }
        "manifests" => {
            let docs = crate::source::load_documents_for_manifests(&args.path)
                .map_err(|e| format!("manifest load error: {e}"))?;
            let values = crate::convert::build_values(&args, &docs)
                .map_err(|e| format!("convert error: {e}"))?;
            let out = crate::output::values_yaml(&values)
                .map_err(|e| format!("values encode error: {e}"))?;
            Ok((
                out,
                format!("Imported {} document(s) from manifests.", docs.len()),
                docs.len(),
            ))
        }
        "compose" => {
            let rep = crate::composeinspect::load(&args.path)
                .map_err(|e| format!("compose inspect error: {e}"))?;
            let values = crate::composeimport::build_values(&args, &rep);
            let out = crate::output::values_yaml(&values)
                .map_err(|e| format!("values encode error: {e}"))?;
            let count = rep.services.len();
            Ok((
                out,
                format!("Imported {} compose service(s).", count),
                count,
            ))
        }
        other => Err(format!(
            "unsupported sourceType '{}' (expected chart|manifests|compose)",
            other
        )),
    };
    if let Some(tmp) = inline_chart_values_temp {
        let _ = std::fs::remove_dir_all(tmp);
    }
    result
}

fn save_chart_payload(
    source_type: &str,
    source_path: &str,
    out_chart_dir: &str,
    chart_name: Option<&str>,
    library_chart_path: Option<&str>,
    values_yaml: &str,
) -> Result<String, String> {
    if out_chart_dir.trim().is_empty() {
        return Err("outChartDir is required".to_string());
    }
    if values_yaml.trim().is_empty() {
        return Err("valuesYaml is empty, run import first".to_string());
    }
    let values: serde_yaml::Value =
        serde_yaml::from_str(values_yaml).map_err(|e| format!("values yaml parse error: {e}"))?;
    crate::output::generate_consumer_chart(out_chart_dir, chart_name, &values, library_chart_path)
        .map_err(|e| format!("save chart error: {e}"))?;
    let mut copied_crds = false;
    if source_type.trim().eq_ignore_ascii_case("chart") && !source_path.trim().is_empty() {
        copied_crds = crate::output::copy_chart_crds_if_any(source_path, out_chart_dir)
            .map_err(|e| format!("copy crds error: {e}"))?;
    }
    if copied_crds {
        Ok(format!("Chart saved: {} (CRDs copied)", out_chart_dir))
    } else {
        Ok(format!("Chart saved: {}", out_chart_dir))
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_render_payload(
    source_type: &str,
    path: &str,
    env: &str,
    group_name: &str,
    group_type: &str,
    import_strategy: &str,
    release_name: &str,
    namespace: Option<String>,
    min_include_bytes: usize,
    include_status: bool,
    values_files: Vec<String>,
    set_values: Vec<String>,
    set_string_values: Vec<String>,
    set_file_values: Vec<String>,
    set_json_values: Vec<String>,
    kube_version: Option<String>,
    api_versions: Vec<String>,
    include_crds: bool,
    chart_values_yaml: Option<String>,
    generated_values_yaml: &str,
    library_chart_path: Option<&str>,
) -> Result<(bool, String, usize, usize), String> {
    if !source_type.trim().eq_ignore_ascii_case("chart") {
        return Err("render compare is supported only for sourceType=chart".to_string());
    }
    if path.trim().is_empty() {
        return Err("path is required".to_string());
    }
    if generated_values_yaml.trim().is_empty() {
        return Err("generated values are empty, run import first".to_string());
    }
    let mut values_files = values_files;
    let mut inline_chart_values_temp: Option<PathBuf> = None;
    if let Some(inline) = chart_values_yaml {
        let tmp_root = create_upload_temp_dir()?;
        let fp = tmp_root.join("chart-inline-values.yaml");
        std::fs::write(&fp, inline).map_err(|e| format!("write inline chart values: {e}"))?;
        values_files.insert(0, fp.to_string_lossy().to_string());
        inline_chart_values_temp = Some(tmp_root);
    }
    let mut args = crate::cli::ImportArgs {
        path: path.to_string(),
        env: env.to_string(),
        group_name: group_name.to_string(),
        group_type: group_type.to_string(),
        min_include_bytes,
        include_status,
        output: None,
        out_chart_dir: None,
        chart_name: None,
        library_chart_path: None,
        import_strategy: import_strategy.to_string(),
        verify_equivalence: false,
        release_name: release_name.to_string(),
        namespace,
        values_files,
        set_values,
        set_string_values,
        set_file_values,
        set_json_values,
        kube_version,
        api_versions,
        include_crds,
        write_rendered_output: None,
    };
    let source_docs = crate::source::load_documents_for_chart(&args)
        .map_err(|e| format!("source chart render error: {e}"))?;

    let generated_values: serde_yaml::Value = serde_yaml::from_str(generated_values_yaml)
        .map_err(|e| format!("generated values parse error: {e}"))?;
    let tmp_chart_root = tempfile::Builder::new()
        .prefix("happ-compare-renders-")
        .tempdir()
        .map_err(|e| format!("create compare temp dir: {e}"))?;
    let generated_chart_dir = tmp_chart_root.path().join("generated-chart");
    let generated_chart_dir_text = generated_chart_dir.to_string_lossy().to_string();
    crate::output::generate_consumer_chart(
        &generated_chart_dir_text,
        Some("imported-chart"),
        &generated_values,
        library_chart_path,
    )
    .map_err(|e| format!("generate chart for compare error: {e}"))?;
    let _ = crate::output::copy_chart_crds_if_any(path, &generated_chart_dir_text)
        .map_err(|e| format!("copy CRDs for compare error: {e}"))?;

    args.path = generated_chart_dir_text;
    args.values_files.clear();
    args.set_values.clear();
    args.set_string_values.clear();
    args.set_file_values.clear();
    args.set_json_values.clear();
    let generated_docs = crate::source::load_documents_for_chart(&args)
        .map_err(|e| format!("generated chart render error: {e}"))?;

    if let Some(tmp) = inline_chart_values_temp {
        let _ = std::fs::remove_dir_all(tmp);
    }
    let result = crate::verify::equivalent(&source_docs, &generated_docs);
    Ok((
        result.equal,
        result.summary,
        source_docs.len(),
        generated_docs.len(),
    ))
}

fn convert_payload(
    mode: &str,
    input: &str,
    doc_mode: &str,
    doc_index: Option<usize>,
) -> Result<String, String> {
    match mode {
        "yaml-to-json" => {
            let docs = crate::yamlmerge::normalize_documents(input)
                .map_err(|e| format!("YAML parse error: {e}"))?;
            let json_docs: Vec<serde_json::Value> = docs
                .into_iter()
                .map(|y| {
                    serde_json::to_value(y).map_err(|e| format!("YAML->JSON conversion error: {e}"))
                })
                .collect::<Result<_, _>>()?;
            match doc_mode.trim().to_ascii_lowercase().as_str() {
                "all" | "" => serde_json::to_string_pretty(&serde_json::Value::Array(json_docs))
                    .map_err(|e| format!("JSON format error: {e}")),
                "first" => {
                    let first = json_docs
                        .into_iter()
                        .next()
                        .unwrap_or(serde_json::Value::Null);
                    serde_json::to_string_pretty(&first)
                        .map_err(|e| format!("JSON format error: {e}"))
                }
                "index" => {
                    let idx = doc_index
                        .ok_or_else(|| "doc index is required for doc mode 'index'".to_string())?;
                    if idx >= json_docs.len() {
                        return Err(format!(
                            "doc index {} is out of range for {} document(s)",
                            idx,
                            json_docs.len()
                        ));
                    }
                    serde_json::to_string_pretty(&json_docs[idx])
                        .map_err(|e| format!("JSON format error: {e}"))
                }
                other => Err(format!("unsupported doc mode: {other}")),
            }
        }
        "json-to-yaml" => {
            let j: serde_json::Value =
                serde_json::from_str(input).map_err(|e| format!("JSON parse error: {e}"))?;
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
    let docs = crate::query::parse_input_docs_prefer_json(input)
        .map_err(|e| format!("jq parse error: {e}"))?;
    let selected = select_docs_for_web(docs, doc_mode, doc_index, "jq")?;
    let out = crate::query::run_query_stream(query, selected)
        .map_err(|e| format!("jq query error: {e}"))?;
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

fn yq_payload(
    query: &str,
    input: &str,
    doc_mode: &str,
    doc_index: Option<usize>,
    compact: bool,
    raw_output: bool,
) -> Result<String, String> {
    let docs = crate::query::parse_input_docs_prefer_yaml(input)
        .map_err(|e| format!("yq parse error: {e}"))?;
    let selected = select_docs_for_web(docs, doc_mode, doc_index, "yq")?;
    let out = crate::query::run_query_stream(query, selected)
        .map_err(|e| format!("yq query error: {e}"))?;
    let mut lines = Vec::with_capacity(out.len());
    for v in out {
        if raw_output {
            if let Some(s) = v.as_str() {
                lines.push(s.to_string());
                continue;
            }
        }
        let line = if compact {
            serde_json::to_string(&v).map_err(|e| format!("yq output encode error: {e}"))?
        } else {
            serde_json::to_string_pretty(&v).map_err(|e| format!("yq output encode error: {e}"))?
        };
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

fn dyff_payload(
    from: &str,
    to: &str,
    ignore_order: bool,
    ignore_whitespace: bool,
) -> Result<String, String> {
    let diff = crate::dyfflike::between_yaml(
        from,
        to,
        crate::dyfflike::DiffOptions {
            ignore_order_changes: ignore_order,
            ignore_whitespace_change: ignore_whitespace,
        },
    )
    .map_err(|e| format!("dyff parse error: {e}"))?;
    if diff.trim().is_empty() {
        return Ok("No differences".to_string());
    }
    Ok(diff)
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
            let idx = doc_index
                .ok_or_else(|| format!("{tool}: doc index is required for doc mode 'index'"))?;
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

fn utf16_to_byte_idx(s: &str, utf16_idx: usize) -> usize {
    let mut units = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if units >= utf16_idx {
            return byte_idx;
        }
        units += ch.len_utf16();
        if units > utf16_idx {
            return byte_idx;
        }
    }
    s.len()
}

fn byte_to_utf16_idx(s: &str, byte_idx: usize) -> usize {
    let mut units = 0usize;
    for (i, ch) in s.char_indices() {
        if i >= byte_idx {
            break;
        }
        units += ch.len_utf16();
    }
    units
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | ':' | '@' | '/' | '-')
}

fn token_at_utf16_offset(s: &str, utf16_off: usize) -> String {
    let byte_off = utf16_to_byte_idx(s, utf16_off);
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut idx = chars
        .iter()
        .position(|(b, _)| *b >= byte_off)
        .unwrap_or(chars.len());
    if idx > 0 && idx == chars.len() {
        idx -= 1;
    }
    let mut l = idx;
    while l > 0 && is_token_char(chars[l - 1].1) {
        l -= 1;
    }
    let mut r = idx;
    while r < chars.len() && is_token_char(chars[r].1) {
        r += 1;
    }
    let from = if l < chars.len() { chars[l].0 } else { s.len() };
    let to = if r < chars.len() { chars[r].0 } else { s.len() };
    s[from..to].trim().to_string()
}

fn extract_yaml_path_at_utf16(s: &str, utf16_off: usize) -> Vec<String> {
    let byte_off = utf16_to_byte_idx(s, utf16_off);
    let lines: Vec<&str> = s.split('\n').collect();
    let mut target_line = 0usize;
    let mut acc = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let len = line.len() + 1;
        if byte_off < acc + len {
            target_line = i;
            break;
        }
        acc += len;
    }
    let mut stack: Vec<(usize, String)> = Vec::new();
    for line in lines.iter().take(target_line + 1) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.as_bytes().iter().take_while(|&&b| b == b' ').count();
        let rest = &line[indent..];
        if let Some(colon) = rest.find(':') {
            let mut key = rest[..colon].trim().to_string();
            if key.is_empty() {
                continue;
            }
            if (key.starts_with('"') && key.ends_with('"'))
                || (key.starts_with('\'') && key.ends_with('\''))
            {
                key = key[1..key.len().saturating_sub(1)].to_string();
            }
            while let Some((ind, _)) = stack.last() {
                if *ind >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }
            stack.push((indent, key));
        }
    }
    stack.into_iter().map(|(_, k)| k).collect()
}

fn find_yaml_ranges_by_path_utf16(s: &str, path: &[String]) -> Vec<(usize, usize)> {
    if path.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = s.split('\n').collect();
    let mut starts = Vec::with_capacity(lines.len());
    let mut acc = 0usize;
    for line in &lines {
        starts.push(acc);
        acc += line.len() + 1;
    }
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut out = Vec::new();
    for i in 0..lines.len() {
        let line = lines[i];
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.as_bytes().iter().take_while(|&&b| b == b' ').count();
        let rest = &line[indent..];
        let Some(colon) = rest.find(':') else {
            continue;
        };
        let mut key = rest[..colon].trim().to_string();
        if key.is_empty() {
            continue;
        }
        if (key.starts_with('"') && key.ends_with('"'))
            || (key.starts_with('\'') && key.ends_with('\''))
        {
            key = key[1..key.len().saturating_sub(1)].to_string();
        }
        while let Some((ind, _)) = stack.last() {
            if *ind >= indent {
                stack.pop();
            } else {
                break;
            }
        }
        let mut next_path: Vec<String> = stack.iter().map(|(_, k)| k.clone()).collect();
        next_path.push(key.clone());
        if next_path == path {
            let mut j = i + 1;
            while j < lines.len() {
                let ln = lines[j];
                if ln.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let ind = ln.as_bytes().iter().take_while(|&&b| b == b' ').count();
                if ind <= indent {
                    break;
                }
                j += 1;
            }
            let from_b = starts[i];
            let to_b = if j < lines.len() { starts[j] } else { s.len() };
            if to_b > from_b {
                out.push((byte_to_utf16_idx(s, from_b), byte_to_utf16_idx(s, to_b)));
            }
        }
        stack.push((indent, key));
    }
    out
}

fn is_num_boundary(ch: Option<char>) -> bool {
    match ch {
        None => true,
        Some(c) => !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')),
    }
}

fn find_value_ranges_utf16(output: &str, needle: &str, kind: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let add_match = |out: &mut Vec<(usize, usize)>, from_b: usize, to_b: usize| {
        out.push((
            byte_to_utf16_idx(output, from_b),
            byte_to_utf16_idx(output, to_b),
        ));
    };
    match kind {
        "bool" | "null" => {
            let mut pos = 0usize;
            while let Some(found) = output[pos..].find(needle) {
                let from_b = pos + found;
                let to_b = from_b + needle.len();
                let prev = output[..from_b].chars().next_back();
                let next = output[to_b..].chars().next();
                let ok = !prev
                    .map(|c| c.is_ascii_alphanumeric() || c == '_')
                    .unwrap_or(false)
                    && !next
                        .map(|c| c.is_ascii_alphanumeric() || c == '_')
                        .unwrap_or(false);
                if ok {
                    add_match(&mut out, from_b, to_b);
                }
                pos = to_b.min(output.len());
                if pos >= output.len() {
                    break;
                }
            }
        }
        "num" => {
            let mut pos = 0usize;
            while let Some(found) = output[pos..].find(needle) {
                let from_b = pos + found;
                let to_b = from_b + needle.len();
                let prev = output[..from_b].chars().next_back();
                let next = output[to_b..].chars().next();
                if is_num_boundary(prev) && is_num_boundary(next) {
                    add_match(&mut out, from_b, to_b);
                }
                pos = to_b.min(output.len());
                if pos >= output.len() {
                    break;
                }
            }
        }
        _ => {
            for candidate in [
                format!("\"{needle}\""),
                format!("'{needle}'"),
                needle.to_string(),
            ] {
                let mut pos = 0usize;
                while let Some(found) = output[pos..].find(&candidate) {
                    let from_b = pos + found;
                    let to_b = from_b + candidate.len();
                    add_match(&mut out, from_b, to_b);
                    pos = to_b.min(output.len());
                    if pos >= output.len() {
                        break;
                    }
                }
            }
        }
    }
    out
}

fn semantic_map_payload(
    source: &str,
    output: &str,
    source_kind: &str,
    output_kind: &str,
    from_utf16: usize,
    to_utf16: usize,
    selected_text: &str,
    path_hint: &[String],
) -> Result<Vec<serde_json::Value>, String> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut path: Vec<String> = if !path_hint.is_empty() {
        path_hint.to_vec()
    } else {
        Vec::new()
    };
    let sk = source_kind.trim().to_ascii_lowercase();
    let ok_yaml_source = sk.is_empty() || sk == "auto" || sk == "yaml";
    if path.is_empty() && ok_yaml_source {
        path = extract_yaml_path_at_utf16(source, from_utf16);
    }
    let ok_yaml_output = matches!(
        output_kind.trim().to_ascii_lowercase().as_str(),
        "" | "auto" | "yaml"
    );
    if ok_yaml_output && !path.is_empty() {
        ranges.extend(find_yaml_ranges_by_path_utf16(output, &path));
    }
    let mut needle = selected_text.trim().to_string();
    if needle.is_empty() {
        needle = token_at_utf16_offset(source, from_utf16.min(to_utf16));
    }
    if !needle.is_empty() {
        let (kind, val) =
            if needle.eq_ignore_ascii_case("true") || needle.eq_ignore_ascii_case("false") {
                ("bool", needle.to_ascii_lowercase())
            } else if needle.eq_ignore_ascii_case("null") {
                ("null", "null".to_string())
            } else if needle.parse::<f64>().is_ok() {
                let n = needle.parse::<f64>().unwrap_or(0.0);
                ("num", format!("{n}").trim_end_matches(".0").to_string())
            } else if (needle.starts_with('"') && needle.ends_with('"'))
                || (needle.starts_with('\'') && needle.ends_with('\''))
            {
                ("str", needle[1..needle.len().saturating_sub(1)].to_string())
            } else {
                ("str", needle)
            };
        ranges.extend(find_value_ranges_utf16(output, &val, kind));
    }
    ranges.sort_unstable();
    ranges.dedup();
    if ranges.len() > 128 {
        ranges.truncate(128);
    }
    Ok(ranges
        .into_iter()
        .filter(|(f, t)| t > f)
        .map(|(f, t)| serde_json::json!({ "from": f, "to": t }))
        .collect())
}

fn write_response(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
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
                "title": "Converters",
                "description": "Useful developer converters: YAML/JSON, JWT, Base64, URL, time and hex."
            },
            {
                "id": "jq-playground",
                "title": "jq Playground",
                "description": "Run jq queries on JSON or YAML input."
            },
            {
                "id": "yq-playground",
                "title": "yq Playground",
                "description": "Run yq queries on YAML or JSON input."
            },
            {
                "id": "dyff-compare",
                "title": "dyff Compare",
                "description": "Compare two YAML payloads with dyff-like output."
            }
        ]
    });
    render_vue_page_html("happ inspect", &model.to_string())
}

pub fn render_compose_page_html(
    source_compose_yaml: &str,
    compose_report_yaml: &str,
    generated_values_yaml: &str,
) -> String {
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
                "title": "Converters",
                "description": "Useful developer converters: YAML/JSON, JWT, Base64, URL, time and hex."
            },
            {
                "id": "jq-playground",
                "title": "jq Playground",
                "description": "Run jq queries on JSON or YAML input."
            },
            {
                "id": "yq-playground",
                "title": "yq Playground",
                "description": "Run yq queries on YAML or JSON input."
            },
            {
                "id": "dyff-compare",
                "title": "dyff Compare",
                "description": "Compare two YAML payloads with dyff-like output."
            }
        ]
    });
    render_vue_page_html("happ compose-inspect", &model.to_string())
}

pub fn render_tools_page_html(stdin_text: Option<&str>) -> String {
    let model = serde_json::json!({
        "title": "happ web",
        "stdinText": stdin_text.unwrap_or(""),
        "utilities": [
            {
                "id": "main-import",
                "title": "Main Import",
                "description": "Import chart/manifests/compose into helm-apps values.yaml."
            },
            {
                "id": "converter",
                "title": "Converters",
                "description": "Useful developer converters: YAML/JSON, JWT, Base64, URL, time and hex."
            },
            {
                "id": "jq-playground",
                "title": "jq Playground",
                "description": "Run jq queries on JSON or YAML input."
            },
            {
                "id": "yq-playground",
                "title": "yq Playground",
                "description": "Run yq queries on YAML or JSON input."
            },
            {
                "id": "dyff-compare",
                "title": "dyff Compare",
                "description": "Compare two YAML payloads with dyff-like output."
            }
        ]
    });
    render_vue_page_html("happ web", &model.to_string())
}

fn json_html_escape(s: &str) -> String {
    s.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

fn render_vue_page_html(page_title: &str, model_json: &str) -> String {
    let model_json = json_html_escape(model_json);
    let cm_bundle_version = CODEMIRROR_BUNDLE_JS.len();
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset='utf-8'/>
<title>{}</title>
<link rel='icon' type='image/svg+xml' href='data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 64 64%22%3E%3Cdefs%3E%3ClinearGradient id=%22g%22 x1=%220%22 y1=%220%22 x2=%221%22 y2=%221%22%3E%3Cstop offset=%220%25%22 stop-color=%22%235a81e6%22/%3E%3Cstop offset=%22100%25%22 stop-color=%22%236ed1bb%22/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect x=%224%22 y=%224%22 width=%2256%22 height=%2256%22 rx=%2214%22 fill=%22%231a1d22%22 stroke=%22url(%23g)%22 stroke-width=%223%22/%3E%3Cpath d=%22M19 19h8v10h10V19h8v26h-8V35H27v10h-8z%22 fill=%22%23e9edf7%22/%3E%3C/svg%3E'/>
<style>
:root {{
  --bg:#1e1f22;
  --surface:#2b2d30;
  --surface-2:#323437;
  --surface-3:#25272a;
  --surface-4:#2f3238;
  --text:#bcbec4;
  --muted:#7e8288;
  --accent:#7aa2ff;
  --accent-2:#6ed1bb;
  --border:#3c3f41;
  --danger:#ff8f8f;
  --ok:#7ad8ab;
}}
* {{ box-sizing:border-box; }}
body {{
  margin:0;
  padding:16px;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  background: #1e1f22;
  color:var(--text);
}}
#app {{ max-width: 1380px; margin: 0 auto; }}
.workspace {{ display:flex; flex-direction:column; gap:12px; }}
.top {{
  display:flex;
  align-items:flex-end;
  justify-content:space-between;
  gap:16px;
  padding:8px 4px 0 4px;
}}
.brand {{ display:flex; flex-direction:column; gap:2px; }}
.title {{ margin:0; font-size:42px; line-height:1.04; letter-spacing:-0.03em; font-weight:800; color:#f3f4f7; }}
.subtitle {{ margin:0; color:var(--muted); font-size:14px; }}
.top-actions {{ display:flex; align-items:center; gap:10px; }}
button {{
  border:1px solid var(--border);
  background:#232833;
  color:#e2e5eb;
  padding:8px 13px;
  border-radius:10px;
  cursor:pointer;
  font-weight:600;
  transition: background-color .16s ease, border-color .16s ease, box-shadow .16s ease, transform .12s ease, color .16s ease;
  will-change: transform;
}}
button.primary {{
  background:#4e74d6;
  border-color:#6f90e8;
  color:#f7faff;
}}
button.primary:hover {{
  background:#5a81e6;
  border-color:#83a1f0;
  box-shadow:0 6px 18px rgba(80,119,216,.28);
  transform: translateY(-1px);
}}
button:hover {{ border-color:#6a7890; background:#2b3240; transform: translateY(-1px); }}
button:active {{ transform: translateY(0); }}
button:disabled {{
  opacity:.55;
  cursor:not-allowed;
  transform:none;
  box-shadow:none;
}}
button:focus-visible {{
  outline:none;
  border-color:#89a7ea;
  box-shadow:0 0 0 2px rgba(126,156,233,.35);
}}
button.secondary {{ background:#242b36; }}
button.tab {{ background:#20242b; border-color:#353c48; color:#cdd3dd; }}
button.tab.active {{
  background:linear-gradient(180deg,#303844 0%, #28303b 100%);
  border-color:#6b7d9b;
  color:#f4f5f8;
  box-shadow:0 8px 20px rgba(76,98,141,.22);
}}
.tabs-row {{
  display:flex;
  flex-direction:column;
  align-items:stretch;
  gap:10px;
  padding:10px;
  background:var(--surface);
  border:1px solid var(--border);
  border-radius:14px;
  box-shadow:0 10px 26px rgba(0, 0, 0, 0.35);
}}
.tabs {{ display:flex; flex-wrap:wrap; gap:8px; }}
.view-controls {{
  display:flex;
  gap:8px;
  flex-wrap:wrap;
  align-items:center;
  padding-top:2px;
  border-top:1px solid rgba(255,255,255,.04);
}}
input[type='text'], select {{
  border:1px solid var(--border);
  border-radius:10px;
  padding:8px 10px;
  min-width:210px;
  background:var(--surface);
  color:var(--text);
  transition: border-color .16s ease, box-shadow .16s ease, background-color .16s ease;
}}
input[type='text']:hover, select:hover, textarea:hover {{
  border-color:#455368;
}}
input[type='text']:focus, select:focus, textarea:focus {{
  outline:none;
  border-color:#7f9de2;
  box-shadow:0 0 0 2px rgba(126,156,233,.24);
}}
input[type='range'] {{ accent-color: var(--accent); }}
textarea {{
  border:1px solid var(--border);
  border-radius:12px;
  padding:10px;
  min-height:240px;
  width:100%;
  font-family: ui-monospace, Menlo, monospace;
  font-size:14px;
  line-height:1.45;
  background:#101317;
  color:#e7e9ee;
  resize:vertical;
  max-height:760px;
  transition:border-color .16s ease, box-shadow .16s ease, background-color .16s ease;
}}
.code-output {{
  border:1px solid var(--border);
  border-radius:12px;
  padding:10px;
  min-height:240px;
  max-height:760px;
  overflow:auto;
  white-space:pre-wrap;
  word-break:break-word;
  font-family: ui-monospace, Menlo, monospace;
  font-size:14px;
  line-height:1.45;
  background:#2b2d30;
  color:#bcbec4;
  margin:0;
}}
.sync-sel {{
  background: #365880;
  color: #f4f7ff;
  border-radius: 4px;
  box-shadow: inset 0 0 0 1px rgba(186, 210, 255, 0.28);
}}
.sync-cursor {{
  display:inline-block;
  width:0;
  height:1.2em;
  margin-left:-1px;
  border-left:2px solid rgba(126, 174, 255, 0.96);
  box-shadow:0 0 0 1px rgba(126, 174, 255, 0.2);
  vertical-align:text-bottom;
  animation: happVirtualCursorBlink 1.1s steps(1, end) infinite;
}}
@keyframes happVirtualCursorBlink {{
  0%,48% {{ opacity:1; }}
  49%,100% {{ opacity:0.2; }}
}}
.hex-output {{
  white-space:pre;
  word-break:normal;
  overflow-wrap:normal;
  tab-size:2;
}}
.hexdump-view {{
  min-height:240px;
  max-height:760px;
  overflow:auto;
  font-family: ui-monospace, Menlo, monospace;
  font-size:14px;
  line-height:1.45;
  user-select:none;
  -webkit-user-select:none;
  padding:10px;
}}
.hexdump-view * {{
  user-select:none;
  -webkit-user-select:none;
}}
.hexdump-row {{
  display:grid;
  grid-template-columns: 10ch max-content max-content max-content;
  column-gap:12px;
  align-items:flex-start;
  margin-bottom:2px;
  width:max-content;
}}
.hexdump-offset {{
  color:#7b8aa3;
  font-weight:700;
  width:10ch;
  min-width:10ch;
}}
.hexdump-hex, .hexdump-ascii {{
  display:inline-flex;
  flex-wrap:nowrap;
  white-space:nowrap;
  font-size:0;
  min-width:max-content;
}}
.hexdump-utf8 {{
  color:#c8ceda;
  display:inline-flex;
  flex-wrap:nowrap;
  white-space:nowrap;
  font-size:0;
  min-width:max-content;
  padding-left:10px;
  border-left:1px solid rgba(255,255,255,.08);
}}
.hexdump-byte, .hexdump-char, .hexdump-utf8-token {{
  border-radius:4px;
  padding:0;
  cursor:default;
  display:inline-block;
  font-size:14px;
  line-height:1.45;
  vertical-align:top;
}}
.hexdump-byte {{ color:#d1a76f; }}
.hexdump-hex {{
  overflow:visible;
}}
.hexdump-byte {{
  width:2ch;
  min-width:2ch;
  text-align:center;
}}
.hexdump-byte + .hexdump-byte {{ margin-left:1ch; }}
.hexdump-byte.sep8 {{ margin-left:2ch; }}
.hexdump-byte.pad {{
  color:transparent;
  pointer-events:none;
}}
.hexdump-char {{ color:#c8ceda; }}
.hexdump-char + .hexdump-char {{ margin-left:0; }}
.hexdump-byte.sel, .hexdump-char.sel, .hexdump-utf8-token.sel {{
  background:rgba(78,108,151,.42);
  color:#eaf0ff;
}}
.output-tools {{
  display:flex;
  gap:8px;
  align-items:center;
  flex-wrap:wrap;
  margin:0 0 8px 0;
}}
.hex-line .hex-offset {{ color:#7b8aa3; font-weight:700; }}
.hex-line .hex-bytes {{ color:#d1a76f; }}
.hex-line .hex-ascii {{ color:#c8ceda; }}
.hex-line .hex-sep {{ color:#6f7787; }}
.hex-plain {{ color:#d1a76f; }}
label.chk {{ display:flex; gap:6px; align-items:center; font-size:13px; color:var(--muted); }}
.util-head {{ margin:0; padding:2px 4px 0 4px; }}
.muted {{ color:var(--muted); font-size:14px; }}
.muted code {{
  font-family:ui-monospace, Menlo, monospace;
  font-size:12px;
  padding:1px 4px;
  border:1px solid #3b4659;
  border-radius:6px;
  color:#c9d6eb;
  background:#202734;
}}
.grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(380px,1fr)); gap:12px; }}
.card {{
  border:1px solid var(--border);
  border-radius:16px;
  padding:12px;
  background:linear-gradient(180deg,var(--surface) 0%,var(--surface-2) 100%);
  box-shadow:0 14px 30px rgba(0, 0, 0, 0.32);
}}
.cardhead {{ display:flex; align-items:center; justify-content:space-between; gap:8px; margin-bottom:8px; }}
.cardhead h3 {{ margin:0; font-size:28px; letter-spacing:-0.02em; color:#f1f2f5; }}
.cardbtns {{ display:flex; gap:6px; }}
pre {{ background:#101317; color:#dce0e8; padding:12px; border-radius:12px; overflow:auto; min-height:280px; margin:0; white-space:pre; font-size:13px; line-height:1.45; }}
pre.wrap {{ white-space:pre-wrap; word-break:break-word; }}
.conv-grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(420px,1fr)); gap:12px; }}
.converter-controls {{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; margin-bottom:10px; }}
.converter-controls .muted {{ margin-left:auto; }}
.form-grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(220px,1fr)); gap:10px; margin-bottom:10px; }}
.form-field {{ display:flex; flex-direction:column; gap:6px; }}
.form-field > label {{ font-size:12px; color:var(--muted); font-weight:600; }}
.import-shell {{ display:flex; flex-direction:column; gap:12px; }}
.import-layout {{ display:grid; grid-template-columns:minmax(520px, 1fr) minmax(520px, 1fr); gap:12px; align-items:stretch; }}
.import-config {{ display:flex; flex-direction:column; gap:12px; }}
.import-config.compact {{
  max-height:calc(82vh - 56px);
  overflow:auto;
  padding-right:4px;
}}
.import-config.compact .conv-grid {{
  grid-template-columns:repeat(auto-fit,minmax(320px,1fr));
}}
.import-config.compact textarea {{
  min-height:110px;
  max-height:300px;
}}
.import-output {{
  border:1px solid var(--border);
  border-radius:12px;
  background:var(--surface-3);
  padding:10px;
  position:sticky;
  top:12px;
  display:flex;
  flex-direction:column;
  min-height:70vh;
}}
.import-output .code-output {{ min-height:520px; max-height:72vh; }}
.import-title {{
  display:flex;
  align-items:flex-start;
  justify-content:space-between;
  gap:10px;
  margin-bottom:10px;
  padding-bottom:8px;
  border-bottom:1px solid rgba(255,255,255,.05);
}}
.import-title h3 {{
  margin:0;
  font-size:26px;
  letter-spacing:-0.02em;
  color:#f3f5fb;
}}
.import-subtitle {{
  margin:2px 0 0 0;
  font-size:14px;
  color:#9ba6ba;
}}
.import-section {{
  border:1px solid var(--border);
  border-radius:12px;
  background:var(--surface-3);
  padding:10px;
  display:flex;
  flex-direction:column;
  min-height:70vh;
}}
.import-section h4 {{
  margin:0 0 10px 0;
  font-size:14px;
  color:var(--text);
  letter-spacing:0.01em;
}}
.import-fields {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(220px,1fr)); gap:10px; }}
.field-hint {{
  margin-top:2px;
  font-size:11px;
  color:#8f9cb4;
  line-height:1.35;
}}
.field-hint code {{
  font-family:ui-monospace, Menlo, monospace;
  font-size:11px;
  padding:1px 4px;
  border:1px solid #3b4659;
  border-radius:6px;
  color:#c9d6eb;
  background:#202734;
}}
.checks-inline {{
  display:flex;
  align-items:center;
  gap:16px;
  flex-wrap:wrap;
  margin-top:2px;
}}
.path-field {{ grid-column: 1 / -1; }}
.path-row {{ display:flex; gap:8px; align-items:center; }}
.path-row input[type='text'] {{ flex:1; min-width:260px; }}
.path-meta {{ font-size:12px; color:var(--muted); }}
.segmented {{
  display:inline-flex;
  align-items:center;
  gap:4px;
  padding:3px;
  border:1px solid var(--border);
  border-radius:10px;
  background:linear-gradient(180deg,#232934 0%, #1d232c 100%);
}}
.segmented button {{
  border:1px solid transparent;
  background:transparent;
  color:#b8c2d5;
  padding:7px 12px;
  border-radius:8px;
  font-size:12px;
  font-weight:700;
  min-height:30px;
}}
.segmented button:hover {{
  background:#2d3440;
  border-color:#47556d;
}}
.segmented button.active {{
  background:linear-gradient(180deg,#3a4558 0%, #313b4b 100%);
  border-color:#7387aa;
  color:#f3f6fd;
  box-shadow:0 6px 16px rgba(78,101,145,.24);
}}
.advanced-details {{
  border:1px solid var(--border);
  border-radius:10px;
  background:rgba(255,255,255,0.015);
  padding:8px;
}}
.advanced-details > summary {{
  cursor:pointer;
  font-size:13px;
  font-weight:700;
  color:#d4dbea;
  user-select:none;
  list-style:none;
}}
.advanced-details > summary::-webkit-details-marker {{
  display:none;
}}
.advanced-details > summary::before {{
  content:'▸';
  display:inline-block;
  margin-right:6px;
  color:#8ea4cf;
}}
.advanced-details[open] > summary::before {{
  content:'▾';
}}
.advanced-body {{
  margin-top:10px;
  display:flex;
  flex-direction:column;
  gap:12px;
}}
.import-toolbar {{
  display:flex;
  justify-content:space-between;
  align-items:center;
  gap:10px;
  margin-bottom:8px;
}}
.import-toolbar .left,
.import-toolbar .right {{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; }}
.import-meta-line {{
  display:flex;
  gap:8px;
  align-items:center;
  flex-wrap:wrap;
  margin:-4px 0 8px 0;
}}
.path-chip {{
  display:inline-block;
  max-width:100%;
  font-size:12px;
  color:#bcc7da;
  border:1px solid var(--border);
  border-radius:8px;
  padding:6px 8px;
  background:rgba(255,255,255,0.03);
  white-space:nowrap;
  overflow:hidden;
  text-overflow:ellipsis;
}}
.generated-toolbar {{
  display:grid;
  grid-template-columns:repeat(auto-fit,minmax(200px,1fr));
  gap:8px;
  align-items:start;
  flex-wrap:wrap;
  margin:6px 0 10px 0;
}}
.generated-toolbar button {{
  padding:5px 9px;
  border-radius:8px;
  font-size:12px;
  min-height:28px;
}}
.toolbar-group {{
  display:flex;
  align-items:center;
  gap:6px;
  flex-wrap:wrap;
  border:1px solid var(--border);
  border-radius:10px;
  background:linear-gradient(180deg,#232934 0%, #1d232c 100%);
  padding:6px;
}}
.toolbar-group-title {{
  font-size:10px;
  letter-spacing:.06em;
  text-transform:uppercase;
  color:#91a0ba;
  margin-right:2px;
  padding:0 4px;
}}
.toolbar-sep {{
  width:1px;
  align-self:stretch;
  background:var(--border);
  margin:0 2px;
}}
.import-status {{
  border:1px solid var(--border);
  border-radius:8px;
  background:linear-gradient(180deg,#252c36 0%, #212832 100%);
  padding:6px 8px;
  font-size:12px;
  color:#c2ccdd;
}}
.editor-shell {{
  border:1px solid var(--border);
  border-radius:12px;
  overflow:hidden;
  background:#0f1318;
}}
.import-editor-shell {{
  min-height:56vh;
  display:flex;
  flex:1 1 auto;
}}
.import-editor-shell > * {{
  flex:1 1 auto;
}}
.source-editor-area {{
  display:flex;
  flex-direction:column;
  flex:1 1 auto;
  min-height:0;
}}
.yaml-editor {{
  position:relative;
  min-height:320px;
  height:100%;
}}
.yaml-editor-highlight {{
  position:absolute;
  inset:0;
  margin:0;
  border:0;
  border-radius:0;
  background:#101317;
  color:#e7e9ee;
  padding:10px;
  overflow:auto;
  min-height:320px;
  max-height:760px;
  white-space:pre;
  word-break:normal;
  pointer-events:none;
  z-index:1;
}}
.yaml-editor-input {{
  position:relative;
  z-index:2;
  background:transparent;
  color:transparent;
  caret-color:#f4f5f8;
  min-height:320px;
  max-height:760px;
  white-space:pre;
  word-break:normal;
}}
.editor-host {{
  min-height:260px;
  height:45vh;
  border:0;
  background:#101317;
}}
.editor-host.generated {{
  min-height:420px;
  height:65vh;
}}
.import-layout .editor-host,
.import-layout .editor-host.generated {{
  min-height:0;
  height:100%;
}}
.import-layout .yaml-editor,
.import-layout .yaml-editor-highlight,
.import-layout .yaml-editor-input {{
  min-height:0;
  height:100%;
}}
.fallback-fold {{
  padding:10px;
  max-height:72vh;
  overflow:auto;
  font-family:ui-monospace, Menlo, monospace;
  font-size:14px;
  line-height:1.45;
  background:#101317;
  color:#e7e9ee;
}}
.fallback-fold details {{
  border:0;
  border-radius:0;
  background:transparent;
  margin:0;
  padding:0;
}}
.fallback-fold summary {{
  cursor:pointer;
  padding:0;
  margin:0;
  color:#d7deea;
  font-size:14px;
  user-select:none;
  white-space:pre;
  font-family:ui-monospace, Menlo, monospace;
  line-height:1.45;
}}
.fallback-fold summary:hover {{
  color:#ecf1fb;
}}
.fallback-fold summary::marker {{
  color:#7f9de2;
}}
.fallback-fold pre {{
  margin:0;
  border:0;
  border-radius:0;
  padding:0;
  background:transparent;
  min-height:0;
  max-height:none;
  overflow:visible;
  white-space:pre;
  word-break:normal;
}}
.yaml-fold-view {{
  min-height:520px;
  max-height:72vh;
  overflow:auto;
  white-space:pre;
}}
.yamlline {{
  display:block;
  padding:0 4px;
  border-radius:4px;
}}
.yamlline.hidden {{
  display:none;
}}
.foldmark {{
  display:inline-block;
  width:14px;
  color:#94a3b8;
  cursor:pointer;
  user-select:none;
}}
.foldmark.sp {{
  cursor:default;
}}
.fs-modal-backdrop {{
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 40;
}}
.fs-modal {{
  width: min(1180px, 95vw);
  max-height: 82vh;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 14px;
  box-shadow: 0 16px 40px rgba(0,0,0,0.45);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}}
.fs-head {{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; }}
.fs-head input[type='text'] {{ flex:1; min-width:260px; }}
.fs-head strong {{
  font-size:20px;
  letter-spacing:-0.01em;
  margin-right:auto;
}}
.fs-list {{
  border:1px solid var(--border);
  border-radius:10px;
  overflow:auto;
  background:var(--surface-3);
  max-height:52vh;
}}
.fs-subpath {{
  font-size:11px;
  color:var(--muted);
  margin-top:2px;
  white-space:nowrap;
  overflow:hidden;
  text-overflow:ellipsis;
  max-width:58vw;
}}
.fs-badge {{
  display:inline-block;
  padding:2px 8px;
  border:1px solid var(--border);
  border-radius:999px;
  font-size:11px;
  color:var(--muted);
}}
.fs-table {{
  width:100%;
  border-collapse:separate;
  border-spacing:0;
  font-size:13px;
}}
.fs-table th {{
  position:sticky;
  top:0;
  z-index:1;
  text-align:left;
  background:#202630;
  color:#c9d2e0;
  border-bottom:1px solid var(--border);
  padding:9px 10px;
  font-size:12px;
  letter-spacing:.02em;
}}
.fs-table td {{
  border-bottom:1px solid #293240;
  padding:9px 10px;
  vertical-align:top;
}}
.fs-row:hover {{
  background:#252e3a;
}}
.fs-row.clickable {{
  cursor:pointer;
}}
.fs-row.hidden-file td:first-child::after {{
  content:' hidden';
  color:#8f98a7;
  font-size:11px;
  margin-left:6px;
}}
.fs-actions {{
  display:flex;
  gap:6px;
  justify-content:flex-end;
}}
.fs-toolbar {{
  display:flex;
  gap:8px;
  align-items:center;
  flex-wrap:wrap;
}}
.fs-toolbar input[type='text'] {{
  min-width:220px;
}}
.result-meta {{ margin-top:8px; font-size:12px; color:var(--muted); display:flex; justify-content:space-between; gap:8px; flex-wrap:wrap; }}
.panel-label {{ margin-bottom:6px; font-size:13px; color:#c0c7d3; font-weight:600; }}
.helper-note {{
  font-size:12px;
  color:#a5b3cb;
  border:1px dashed #354152;
  border-radius:8px;
  padding:6px 8px;
  background:rgba(122,162,255,0.08);
}}
.jq-query-editor {{ position:relative; }}
.jq-query-highlight {{
  position:absolute; inset:0;
  margin:0;
  box-sizing:border-box;
  border:1px solid var(--border);
  border-radius:12px;
  background:#101317;
  color:#dce0e8;
  padding:10px;
  overflow:auto;
  min-height:72px;
  white-space:pre;
  word-break:normal;
  overflow-wrap:normal;
  tab-size:2;
  font-family:ui-monospace, Menlo, monospace;
  font-size:13px;
  line-height:1.45;
  pointer-events:none;
  z-index:1;
}}
.jq-query-input {{
  position:relative;
  z-index:2;
  box-sizing:border-box;
  width:100%;
  background:transparent;
  color:transparent;
  caret-color:#f4f5f8;
  white-space:pre;
  word-break:normal;
  overflow-wrap:normal;
  tab-size:2;
  font-family:ui-monospace, Menlo, monospace;
  font-size:13px;
  line-height:1.45;
  min-height:72px;
}}
.jq-token-keyword {{ color:#b69cff; font-weight:700; }}
.jq-token-func {{ color:#9ab6ff; font-weight:700; }}
.jq-token-string {{ color:#87d4c3; }}
.jq-token-number {{ color:#e7c47a; }}
.jq-token-op {{ color:#ef9db0; font-weight:700; }}
.jq-token-field {{ color:#9db2e6; }}
.jq-suggest {{
  position:absolute;
  top:calc(100% + 6px);
  left:0;
  width:420px;
  max-width:calc(100% - 12px);
  border:1px solid var(--border);
  border-radius:10px;
  background:#1d2127;
  overflow:auto;
  max-height:220px;
  z-index:40;
  box-shadow:0 14px 30px rgba(0,0,0,.42);
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
.jq-suggest-row.active {{ background:#2a3039; }}
.jq-suggest-label {{ font-weight:700; color:#eef0f4; }}
.jq-suggest-desc {{ color:#a7adb9; font-size:12px; margin-top:2px; }}
.jq-suggest-hint {{
  margin-top:10px;
  border:1px dashed var(--border);
  border-radius:10px;
  padding:8px 10px;
  background:#1d2128;
  font-size:12px;
  color:#b9c0cc;
}}
.chip-row {{ display:flex; gap:6px; flex-wrap:wrap; margin:6px 0 10px 0; }}
.chip {{
  border:1px solid #3b4350;
  background:#242a33;
  color:#d9dde6;
  border-radius:999px;
  padding:4px 10px;
  font-size:12px;
  cursor:pointer;
}}
.chip:hover {{ background:#2d343f; border-color:#556072; }}
.tok-key {{ color:#9fb4e2; font-weight:700; }}
.tok-str {{ color:#8cd3c2; }}
.tok-num {{ color:#e6c786; }}
.tok-bool {{ color:#c2a9ff; font-weight:700; }}
.tok-null {{ color:#e6a7b6; font-weight:700; }}
.tok-op {{ color:#a1acbf; }}
.tok-diff-add {{ color:var(--ok); font-weight:700; }}
.tok-diff-rem {{ color:#ff8e8e; font-weight:700; }}
.tok-diff-chg {{ color:#ffd37b; font-weight:700; }}
.err {{ color:var(--danger); font-weight:600; }}
@media (max-width: 960px) {{
  body {{ padding:12px; }}
  .title {{ font-size:34px; }}
  .top {{ align-items:flex-start; flex-direction:column; }}
  .tabs-row {{ align-items:flex-start; }}
  .view-controls {{ width:100%; justify-content:flex-start; }}
  .conv-grid {{ grid-template-columns:1fr; }}
  .import-layout {{ grid-template-columns:1fr; }}
  .import-output {{ position:static; top:auto; }}
  .import-title h3 {{ font-size:30px; }}
  .segmented {{ width:100%; }}
  .segmented button {{ flex:1; text-align:center; }}
}}
</style>
<script>
window.__happScriptErrors = [];
window.addEventListener('error', function(e) {{
  try {{
    window.__happScriptErrors.push({{
      message: String(e && e.message || ''),
      file: String(e && e.filename || ''),
      line: Number((e && e.lineno) || 0),
      col: Number((e && e.colno) || 0)
    }});
  }} catch (_) {{}}
}});
</script>
<script src='/assets/vue.global.prod.js'></script>
<script src='/assets/codemirror.bundle.js?v={}'
        onload='window.__happCmScriptLoaded = true'
        onerror='window.__happCmScriptError = "load-error"'></script>
<script>window.__happCmAfterScript = !!(window.HappCodeMirror && window.HappCodeMirror.createYamlEditor);</script>
<script id='happ-model' type='application/json'>{}</script>
</head>
<body>
<div id='app'>
  <div class='workspace'>
  <div class='top'>
    <div class='brand'>
      <h2 class='title'>{{{{ model.title }}}}</h2>
      <div class='subtitle'>Fast local toolset for YAML/JSON, jq/yq and dyff.</div>
    </div>
    <div class='top-actions'>
      <button @click='exitUi'>Exit</button>
    </div>
  </div>
  <div class='tabs-row'>
    <div class='tabs'>
      <button class='tab'
              :class='{{ active: activeUtilityKey === u.id }}'
              v-for='u in utilities'
              :key='u.id'
              @click='selectUtility(u.id)'>{{{{ u.title }}}}</button>
    </div>
    <div v-if='activeHasPanes' class='view-controls'>
      <input v-if='activeHasPanes' type='text' v-model='query' placeholder='Search pane content'/>
      <label class='chk'><input type='checkbox' v-model='wrapLines'/> Wrap lines</label>
      <label class='chk'>Font
        <input type='range' min='11' max='18' step='1' v-model.number='fontSize'/>
      </label>
      <button v-if='activeHasPanes' class='secondary' @click='expandAll'>Expand all</button>
      <button v-if='activeHasPanes' class='secondary' @click='collapseAll'>Collapse all</button>
    </div>
  </div>
  <div v-if='activeUtilityKey !== "main-import"' class='util-head'>
    <div><strong>{{{{ currentUtility.title }}}}</strong></div>
    <div class='muted'>{{{{ currentUtility.description || "" }}}}</div>
  </div>
  <div v-if='activeUtilityKey !== "main-import"' class='muted' style='margin:0 0 8px 0;'>Settings are persisted in localStorage.</div>

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

  <div v-else-if='activeUtilityKey === "main-import"' class='card'>
    <div class='import-title'>
      <div>
        <h3>Main Import</h3>
        <div class='import-subtitle'>Import chart/manifests/compose into helm-apps values.yaml.</div>
      </div>
      <div class='helper-note'>Settings are persisted in localStorage.</div>
    </div>
    <div class='import-toolbar'>
      <div class='left'>
        <button class='primary' @click='runMainImport'>Run import</button>
        <button class='secondary' @click='openMainImportConfig'>Import configuration</button>
        <button class='secondary' @click='clearMainImport'>Clear output</button>
      </div>
      <div class='right'>
        <span class='import-status'>{{{{ mainImportMessage || "Ready" }}}}</span>
        <span class='import-status'>docs/services: {{{{ mainImportSourceCount }}}}</span>
        <span class='import-status' :title='cmProbeReason || ""'>editor: {{{{ cmAvailable ? "cm6" : "fallback" }}}}</span>
      </div>
    </div>
    <div class='import-meta-line'>
      <span class='import-status'>source: {{{{ mainImportSourceType }}}}</span>
      <span class='path-chip' :title='mainImportPath || "-"'>path: {{{{ mainImportPath || "-" }}}}</span>
    </div>
    <div class='import-layout'>
      <div class='import-section'>
        <h4>Source chart values.yaml</h4>
        <div v-if='mainImportSourceType !== "chart"' class='muted'>Source type is {{{{ mainImportSourceType }}}}. Source values editor is available only for chart mode.</div>
        <div v-else class='source-editor-area'>
          <div class='import-toolbar' style='margin-bottom:6px;'>
            <div class='left'>
              <button class='secondary' @click='loadChartValuesFromPath'>Load values.yaml</button>
              <button class='secondary' @click='pasteMainImportFromStdin' :disabled='!mainImportStdinText'>Paste stdin</button>
              <button class='secondary' @click='resetChartValuesEditor'>Reset</button>
            </div>
            <div class='right'>
              <label class='chk'><input type='checkbox' v-model='mainImportUseChartValuesEditor'/> use edited chart values</label>
            </div>
          </div>
          <div class='editor-shell import-editor-shell'>
            <div v-if='cmAvailable' class='editor-host' ref='mainImportSourceCmHost'></div>
            <div v-else class='yaml-editor'>
              <pre class='yaml-editor-highlight' aria-hidden='true' v-html='mainImportSourceHighlighted'></pre>
              <textarea
                class='yaml-editor-input'
                v-model='mainImportChartValuesEditor'
                ref='mainImportSourceInput'
                spellcheck='false'
                @scroll='syncMainImportSourceScroll'
                @input='syncMainImportSourceScroll'
                @select='onMainImportTextareaSelect'
                @keyup='onMainImportTextareaSelect'
                @mouseup='onMainImportTextareaSelect'
                style='min-height:320px;'
              ></textarea>
            </div>
          </div>
        </div>
      </div>
      <div class='import-output'>
        <div class='panel-label' style='margin:0;'>Generated values.yaml</div>
        <div class='generated-toolbar'>
          <div class='toolbar-group'>
            <span class='toolbar-group-title'>Analyze</span>
            <button class='primary' @click='runMainImport'>Render + Analyze</button>
            <button
              class='secondary'
              @click='runMainImportCompare'
              :disabled='mainImportCompareRunning || !mainImportOutput || mainImportSourceType !== "chart"'>
              {{{{ mainImportCompareRunning ? 'Comparing…' : 'Compare renders' }}}}
            </button>
          </div>
          <div class='toolbar-group'>
            <span class='toolbar-group-title'>Values</span>
            <button class='secondary' @click='copyMainImportOutput' :disabled='!mainImportOutput'>Copy values</button>
            <button class='secondary' @click='openMainImportSaveChart' :disabled='!mainImportOutput'>Save as chart</button>
          </div>
          <div class='toolbar-group'>
            <span class='toolbar-group-title'>Folding</span>
            <button class='secondary' @click='foldMainImportLevel(1)'>L1</button>
            <button class='secondary' @click='foldMainImportLevel(2)'>L2</button>
            <button class='secondary' @click='foldMainImportLevel(3)'>L3</button>
            <button class='secondary' @click='collapseAllMainImportSections'>Collapse</button>
            <button class='secondary' @click='expandAllMainImportSections'>Expand</button>
          </div>
        </div>
        <div class='editor-shell import-editor-shell'>
          <div v-if='cmAvailable' class='editor-host generated' ref='mainImportGeneratedCmHost'></div>
          <div v-else class='fallback-fold' @click='onMainImportFoldClick'>
            <pre class='code-output yaml-fold-view' v-html='mainImportPreviewHtml'></pre>
          </div>
        </div>
      </div>
    </div>
    <div class='err' v-if='mainImportError' style='margin-top:8px;'>{{{{ mainImportError }}}}</div>
    <div class='muted' v-if='!cmAvailable && cmProbeReason' style='margin-top:6px;'>CodeMirror: {{{{ cmProbeReason }}}}</div>
    <div class='err' v-if='mainImportCompareError' style='margin-top:8px;'>{{{{ mainImportCompareError }}}}</div>
    <div class='muted' v-if='mainImportCompareMessage' style='margin-top:8px;'>{{{{ mainImportCompareMessage }}}}</div>
    <div v-if='mainImportCompareSummary' class='card' style='margin-top:10px;'>
      <div class='cardhead'>
        <h3>Render compare</h3>
      </div>
      <div class='muted'>{{{{ mainImportCompareSummary }}}}</div>
      <div class='stats-line' style='margin-top:8px;'>
        <span>equal: {{{{ mainImportCompareEqual ? "yes" : "no" }}}}</span>
        <span>source docs: {{{{ mainImportCompareSourceCount }}}}</span>
        <span>generated docs: {{{{ mainImportCompareGeneratedCount }}}}</span>
      </div>
    </div>
    <div class='muted' v-if='mainImportSaveChartMessage' style='margin-top:8px;'>{{{{ mainImportSaveChartMessage }}}}</div>
  </div>

  <div v-if='mainImportConfigOpen' class='fs-modal-backdrop'>
    <div class='fs-modal' style='width:min(1280px, 96vw);'>
      <div class='fs-head'>
        <strong>Import configuration</strong>
        <button class='secondary' @click='loadSampleMainImport'>Sample config</button>
        <button class='secondary' @click='resetMainImportConfig'>Reset defaults</button>
        <button class='secondary' @click='closeMainImportConfig'>Close</button>
      </div>
      <div class='import-config compact'>
        <div class='import-section'>
          <h4>Source</h4>
          <div class='import-fields'>
            <div class='form-field path-field'>
              <label>Source type</label>
              <div class='segmented'>
                <button type='button' :class='{{ active: mainImportSourceType === "chart" }}' @click='mainImportSourceType = "chart"'>Chart</button>
                <button type='button' :class='{{ active: mainImportSourceType === "manifests" }}' @click='mainImportSourceType = "manifests"'>Manifests</button>
                <button type='button' :class='{{ active: mainImportSourceType === "compose" }}' @click='mainImportSourceType = "compose"'>Compose</button>
              </div>
              <div class='field-hint'>Select what you import. Path picker on this server adapts to selected source type.</div>
            </div>
            <div class='form-field path-field'>
              <label>Path on server</label>
              <div class='path-row'>
                <input type='text' v-model='mainImportPath' :placeholder='mainImportPathPlaceholder'/>
                <button class='secondary' @click='openMainImportPicker'>Choose…</button>
                <button class='secondary' @click='clearMainImportSelection'>Clear</button>
              </div>
              <div class='path-meta'>{{{{ mainImportPickedFilesLabel || "No selected server path" }}}}</div>
            </div>
          </div>
        </div>

        <div class='import-section'>
          <h4>Render Options</h4>
          <div class='import-fields'>
            <div class='form-field'>
              <label>Environment</label>
              <input type='text' v-model='mainImportEnv' placeholder='dev'/>
              <div class='field-hint'>Mapped to <code>global.env</code> for env-map resolution.</div>
            </div>
            <div class='form-field'>
              <label>Release name</label>
              <input type='text' v-model='mainImportReleaseName' placeholder='imported'/>
              <div class='field-hint'>Used for rendering chart and generated chart validation.</div>
            </div>
            <div class='form-field'>
              <label>Namespace (optional)</label>
              <input type='text' v-model='mainImportNamespace' placeholder='default'/>
              <div class='field-hint'>Set this if source chart relies on release namespace.</div>
            </div>
            <div class='form-field'>
              <label>Kubernetes version (optional)</label>
              <input type='text' v-model='mainImportKubeVersion' placeholder='1.29.0'/>
              <div class='field-hint'>Example: <code>1.29.0</code>. Empty means default Helm capabilities.</div>
            </div>
            <div class='form-field path-field'>
              <label>Include options</label>
              <div class='checks-inline'>
                <label class='chk'><input type='checkbox' v-model='mainImportIncludeStatus'/> include status</label>
                <label class='chk'><input type='checkbox' v-model='mainImportIncludeCrds'/> include CRDs</label>
              </div>
              <div class='field-hint'>Use CRDs when you want generated chart to include source chart CRDs.</div>
            </div>
          </div>
        </div>

        <div class='import-section'>
          <h4>Advanced</h4>
          <details class='advanced-details'>
            <summary>Mapping and Helm flags</summary>
            <div class='advanced-body'>
              <div class='import-fields'>
                <div class='form-field'>
                  <label>Group name</label>
                  <input type='text' v-model='mainImportGroupName' placeholder='apps-k8s-manifests'/>
                  <div class='field-hint'>Top-level section in generated values.yaml.</div>
                </div>
                <div class='form-field'>
                  <label>Group type</label>
                  <input type='text' v-model='mainImportGroupType' placeholder='apps-k8s-manifests'/>
                  <div class='field-hint'>Stored in <code>__GroupVars__.type</code> when custom group type is needed.</div>
                </div>
                <div class='form-field'>
                  <label>Import strategy</label>
                  <select v-model='mainImportImportStrategy'>
                    <option value='helpers'>helpers (default)</option>
                    <option value='raw'>raw</option>
                  </select>
                  <div class='field-hint'><code>helpers</code> maps into supported library entities. <code>raw</code> keeps generic manifests.</div>
                </div>
                <div class='form-field'>
                  <label>Min include bytes</label>
                  <input type='number' min='0' step='1' v-model.number='mainImportMinIncludeBytes'/>
                  <div class='field-hint'>Dedup threshold for include profile extraction.</div>
                </div>
              </div>
              <div class='conv-grid'>
                <div>
                  <div class='panel-label'>values files (--values), one per line</div>
                  <textarea v-model='mainImportValuesFilesText' spellcheck='false' style='min-height:120px;'></textarea>
                </div>
                <div>
                  <div class='panel-label'>set flags (--set), one per line</div>
                  <textarea v-model='mainImportSetText' spellcheck='false' style='min-height:120px;'></textarea>
                </div>
                <div>
                  <div class='panel-label'>set-string / set-file / set-json, one per line</div>
                  <textarea v-model='mainImportExtraSetText' spellcheck='false' style='min-height:120px;'></textarea>
                </div>
                <div>
                  <div class='panel-label'>api versions (--api-version), one per line</div>
                  <textarea v-model='mainImportApiVersionsText' spellcheck='false' style='min-height:120px;'></textarea>
                </div>
              </div>
            </div>
          </details>
        </div>
      </div>
    </div>
  </div>

  <div v-if='mainImportSaveChartOpen' class='fs-modal-backdrop'>
    <div class='fs-modal' style='width:min(860px, 96vw);'>
      <div class='fs-head'>
        <strong>Save chart (helm-apps library)</strong>
        <button class='secondary' @click='closeMainImportSaveChart'>Close</button>
      </div>
      <div class='import-fields'>
        <div class='form-field path-field'>
          <label>Output chart directory (server path)</label>
          <div class='path-row'>
            <input type='text' v-model='mainImportOutChartDir' placeholder='/path/to/new-chart'/>
            <button class='secondary' @click='openMainImportOutChartPicker'>Choose…</button>
          </div>
        </div>
        <div class='form-field'>
          <label>Chart name (optional)</label>
          <input type='text' v-model='mainImportOutChartName' placeholder='my-app'/>
        </div>
        <div class='form-field'>
          <label>Library chart path (optional)</label>
          <input type='text' v-model='mainImportLibraryChartPath' placeholder='charts/helm-apps'/>
        </div>
      </div>
      <div class='import-toolbar' style='margin-top:10px;'>
        <div class='left'>
          <button class='primary' @click='saveMainImportAsChart' :disabled='mainImportSaveChartRunning || !mainImportOutput'>
            {{{{ mainImportSaveChartRunning ? 'Saving…' : 'Save chart' }}}}
          </button>
        </div>
        <div class='right'>
          <span class='import-status'>{{{{ mainImportSaveChartMessage || 'Ready' }}}}</span>
        </div>
      </div>
      <div class='err' v-if='mainImportSaveChartError' style='margin-top:8px;'>{{{{ mainImportSaveChartError }}}}</div>
    </div>
  </div>

  <div v-else-if='activeUtilityKey === "converter"' class='card'>
    <div class='cardhead'>
      <h3>Converters</h3>
      <div class='cardbtns'>
        <button class='secondary' @click='swapConvertMode'>Swap</button>
        <button class='secondary' @click='clearConverter'>Clear</button>
        <button class='secondary' @click='loadSampleConverter'>Sample</button>
      </div>
    </div>
    <div class='converter-controls'>
      <select v-model='converterMode'>
        <optgroup label='YAML/JSON'>
          <option value='yaml-to-json'>YAML → JSON</option>
          <option value='json-to-yaml'>JSON → YAML</option>
        </optgroup>
        <optgroup label='Encoding'>
          <option value='base64-encode'>Base64 encode</option>
          <option value='base64-decode'>Base64 decode</option>
          <option value='url-encode'>URL encode</option>
          <option value='url-decode'>URL decode</option>
          <option value='text-to-hex'>Text → HEX</option>
          <option value='hex-to-text'>HEX → Text</option>
        </optgroup>
        <optgroup label='Time'>
          <option value='unix-to-iso'>Unix → ISO8601</option>
          <option value='iso-to-unix'>ISO8601 → Unix</option>
        </optgroup>
        <optgroup label='Security'>
          <option value='jwt-inspect'>JWT inspect</option>
        </optgroup>
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
      <div class='muted'>{{{{ converterModeLabel }}}}</div>
    </div>
    <div class='conv-grid'>
      <div>
        <div class='panel-label'>Input</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='converterInputCmHost' style='min-height:240px;height:38vh;'></div>
          <textarea v-else v-model='converterInput' spellcheck='false' @select='onConverterTextareaSelect' @keyup='onConverterTextareaSelect' @mouseup='onConverterTextareaSelect'></textarea>
        </div>
      </div>
      <div>
        <div class='panel-label'>Output</div>
        <div class='output-tools'>
          <button class='secondary' @click='copyConverterOutput'>Copy output</button>
          <template v-if='converterMode === "text-to-hex"'>
            <div class='segmented'>
              <button type='button' :class='{{ active: converterHexView === "plain" }}' @click='converterHexView = "plain"'>plain</button>
              <button type='button' :class='{{ active: converterHexView === "0x" }}' @click='converterHexView = "0x"'>0x</button>
              <button type='button' :class='{{ active: converterHexView === "escaped" }}' @click='converterHexView = "escaped"'>\\x</button>
              <button type='button' :class='{{ active: converterHexView === "byte-array" }}' @click='converterHexView = "byte-array"'>byte[]</button>
              <button type='button' :class='{{ active: converterHexView === "c-array" }}' @click='converterHexView = "c-array"'>c-array</button>
              <button type='button' :class='{{ active: converterHexView === "dump" }}' @click='converterHexView = "dump"'>dump</button>
            </div>
            <label class='chk'><input type='checkbox' v-model='converterHexUppercase'/> upper</label>
            <input type='text' v-model='converterHexSeparator' style='width:120px;' placeholder='separator'/>
            <input type='number' min='4' max='64' step='1' v-model.number='converterHexBytesPerLine' style='width:120px;' placeholder='bytes/line'/>
          </template>
          <template v-if='converterMode === "hex-to-text"'>
            <select v-model='converterHexInFormat'>
              <option value='auto'>HEX in: auto</option>
              <option value='plain'>HEX in: plain</option>
              <option value='0x'>HEX in: 0xNN</option>
              <option value='escaped'>HEX in: \\xNN</option>
              <option value='byte-array'>HEX in: [72,101]</option>
              <option value='c-array'>HEX in: {{0x48,0x65}}</option>
              <option value='dump'>HEX in: dump</option>
            </select>
          </template>
        </div>
        <div class='editor-shell'>
          <div v-if='cmAvailable && converterOutputUseCm' class='editor-host' ref='converterOutputCmHost' style='min-height:240px;height:38vh;'></div>
          <div v-else-if='converterHexDumpInteractive' class='hexdump-view' @mouseup='onHexDumpPointerUp' @mouseleave='onHexDumpPointerUp' @selectstart.prevent>
            <div class='hexdump-row' v-for='row in converterHexDumpRows' :key='row.offset'>
              <div class='hexdump-offset'>{{{{ row.offsetHex }}}}</div>
              <div class='hexdump-hex'>
                <span
                  v-for='b in row.bytes'
                  :key="'h'+b.key"
                  class='hexdump-byte'
                  :class='{{ sel: isHexByteSelected(b.idx), sep8: b.sep8, pad: !b.real }}'
                  @mousedown.prevent='onHexByteDown(b.idx)'
                  @mouseenter='onHexByteEnter(b.idx)'>{{{{ b.hex }}}}</span>
              </div>
              <div class='hexdump-ascii'>
                <span
                  v-for='c in row.ascii'
                  :key="'a'+c.key"
                  class='hexdump-char'
                  :class='{{ sel: isHexByteSelected(c.idx), pad: !c.real }}'
                  @mousedown.prevent='onHexByteDown(c.idx)'
                  @mouseenter='onHexByteEnter(c.idx)'>{{{{ c.ch }}}}</span>
              </div>
              <div class='hexdump-utf8'>
                <span
                  v-for='t in row.utf8'
                  :key="'u'+t.start+'-'+t.end"
                  class='hexdump-utf8-token'
                  :class='{{ sel: isHexRangeSelected(t.start, t.end) }}'
                  @mousedown.prevent='onHexByteDown(t.start)'
                  @mouseenter='onHexByteEnter(t.end)'>{{{{ t.text }}}}</span>
              </div>
            </div>
          </div>
          <pre v-else class='code-output hex-output' v-html='converterOutputRich'></pre>
        </div>
      </div>
    </div>
    <div class='result-meta'>
      <span>mode: {{{{ converterModeLabel }}}}</span>
      <span v-if='converterMode === "yaml-to-json"'>docs: {{{{ converterDocMode }}}}</span>
      <span>output chars: {{{{ (converterOutput || '').length }}}}</span>
    </div>
    <div class='err' v-if='converterError' style='margin-top:8px;'>{{{{ converterError }}}}</div>
  </div>

  <div v-else-if='activeUtilityKey === "jq-playground"' class='card'>
    <div class='cardhead'>
      <h3>jq Playground</h3>
      <div class='cardbtns'>
        <button class='secondary' @click='runJq'>Run</button>
        <button class='secondary' @click='clearJq'>Clear</button>
        <button class='secondary' @click='loadSampleJq'>Sample</button>
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
    <div style='margin-bottom:10px; position:relative;'>
      <div class='muted' style='margin-bottom:6px;'>jq query (syntax highlighted)</div>
      <div class='muted' style='margin-bottom:6px; font-size:12px;'>Hints: <code>Ctrl/Cmd+Space</code> open suggestions, <code>Tab</code> apply, <code>Ctrl/Cmd+Enter</code> apply selected.</div>
      <div class='chip-row'>
        <button class='chip' v-for='p in jqPresets' :key='p.label' @click='applyJqPreset(p.query)'>{{{{ p.label }}}}</button>
      </div>
      <div class='jq-query-editor'>
        <pre class='jq-query-highlight' aria-hidden='true' v-html='jqQueryHighlighted'></pre>
        <textarea class='jq-query-input'
                  v-model='jqQuery'
                  wrap='off'
                  spellcheck='false'
                  @input='onJqInput'
                  @click='updateJqSuggestState'
                  @keyup='updateJqSuggestState'
                  @keydown='onJqKeydown'
                  @blur='closeJqSuggestSoon'
                  @scroll='syncJqScroll'
                  ref='jqQueryInput'></textarea>
      </div>
      <div class='jq-suggest' v-if='jqSuggestOpen && jqSuggestions.length' :style='jqSuggestPanelStyle'>
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
        <div class='panel-label'>Input (JSON or YAML)</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='jqInputCmHost' style='min-height:240px;height:38vh;'></div>
          <textarea v-else v-model='jqInput' spellcheck='false' @select='onJqTextareaSelect' @keyup='onJqTextareaSelect' @mouseup='onJqTextareaSelect'></textarea>
        </div>
      </div>
      <div>
        <div class='panel-label'>Output</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='jqOutputCmHost' style='min-height:240px;height:38vh;'></div>
          <pre v-else class='code-output' v-html='jqOutputHighlighted'></pre>
        </div>
      </div>
    </div>
    <div class='result-meta'>
      <span>results: {{{{ jqResultCount }}}}, chars: {{{{ (jqOutput || '').length }}}}</span>
      <span>compact: {{{{ jqCompact ? "on" : "off" }}}}, raw: {{{{ jqRawOutput ? "on" : "off" }}}}</span>
    </div>
    <div class='err' v-if='jqError' style='margin-top:8px;'>{{{{ jqError }}}}</div>
  </div>

  <div v-else-if='activeUtilityKey === "yq-playground"' class='card'>
    <div class='cardhead'>
      <h3>yq Playground</h3>
      <div class='cardbtns'>
        <button class='secondary' @click='runYq'>Run</button>
        <button class='secondary' @click='clearYq'>Clear</button>
        <button class='secondary' @click='loadSampleYq'>Sample</button>
      </div>
    </div>
    <div class='converter-controls'>
      <select v-model='yqDocMode'>
        <option value='first'>Input docs: first</option>
        <option value='all'>Input docs: all</option>
        <option value='index'>Input docs: index</option>
      </select>
      <input v-if='yqDocMode === "index"'
             v-model.number='yqDocIndex'
             type='number'
             min='0'
             step='1'
             style='width:140px;'
             placeholder='doc index' />
      <label class='chk'><input type='checkbox' v-model='yqCompact'/> compact</label>
      <label class='chk'><input type='checkbox' v-model='yqRawOutput'/> raw output</label>
      <button class='secondary' @click='copyYqOutput'>Copy output</button>
      <div class='muted'>Live query execution is enabled</div>
    </div>
    <div style='margin-bottom:10px;'>
      <div class='muted' style='margin-bottom:6px;'>yq query</div>
      <div class='chip-row'>
        <button class='chip' v-for='p in yqPresets' :key='p.label' @click='applyYqPreset(p.query)'>{{{{ p.label }}}}</button>
      </div>
      <div class='editor-shell'>
        <div v-if='cmAvailable' class='editor-host' ref='yqQueryCmHost' style='min-height:120px;height:22vh;'></div>
        <textarea v-else v-model='yqQuery' spellcheck='false' style='min-height:72px;'></textarea>
      </div>
    </div>
    <div class='conv-grid'>
      <div>
        <div class='panel-label'>Input (YAML or JSON)</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='yqInputCmHost' style='min-height:240px;height:38vh;'></div>
          <textarea v-else v-model='yqInput' spellcheck='false' @select='onYqTextareaSelect' @keyup='onYqTextareaSelect' @mouseup='onYqTextareaSelect'></textarea>
        </div>
      </div>
      <div>
        <div class='panel-label'>Output</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='yqOutputCmHost' style='min-height:240px;height:38vh;'></div>
          <pre v-else class='code-output' v-html='yqOutputHighlighted'></pre>
        </div>
      </div>
    </div>
    <div class='result-meta'>
      <span>results: {{{{ yqResultCount }}}}, chars: {{{{ (yqOutput || '').length }}}}</span>
      <span>compact: {{{{ yqCompact ? "on" : "off" }}}}, raw: {{{{ yqRawOutput ? "on" : "off" }}}}</span>
    </div>
    <div class='err' v-if='yqError' style='margin-top:8px;'>{{{{ yqError }}}}</div>
  </div>

  <div v-else-if='activeUtilityKey === "dyff-compare"' class='card'>
    <div class='cardhead'>
      <h3>dyff Compare</h3>
      <div class='cardbtns'>
        <button class='secondary' @click='runDyff'>Run compare</button>
        <button class='secondary' @click='clearDyff'>Clear</button>
        <button class='secondary' @click='loadSampleDyff'>Sample</button>
      </div>
    </div>
    <div class='converter-controls'>
      <label class='chk'><input type='checkbox' v-model='dyffIgnoreOrder'/> ignore order</label>
      <label class='chk'><input type='checkbox' v-model='dyffIgnoreWhitespace'/> ignore whitespace</label>
      <button class='secondary' @click='copyDyffOutput'>Copy output</button>
      <div class='muted'>Live compare is enabled</div>
    </div>
    <div class='conv-grid'>
      <div>
        <div class='panel-label'>From YAML</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='dyffFromCmHost' style='min-height:240px;height:36vh;'></div>
          <textarea v-else v-model='dyffFrom' spellcheck='false' @select='onDyffFromTextareaSelect' @keyup='onDyffFromTextareaSelect' @mouseup='onDyffFromTextareaSelect'></textarea>
        </div>
      </div>
      <div>
        <div class='panel-label'>To YAML</div>
        <div class='editor-shell'>
          <div v-if='cmAvailable' class='editor-host' ref='dyffToCmHost' style='min-height:240px;height:36vh;'></div>
          <textarea v-else v-model='dyffTo' spellcheck='false' @select='onDyffToTextareaSelect' @keyup='onDyffToTextareaSelect' @mouseup='onDyffToTextareaSelect'></textarea>
        </div>
      </div>
    </div>
    <div style='margin-top:10px;'>
      <div class='panel-label'>Diff result</div>
      <div class='editor-shell'>
        <div v-if='cmAvailable' class='editor-host' ref='dyffOutputCmHost' style='min-height:260px;height:40vh;'></div>
        <pre v-else class='code-output' v-html='dyffOutputHighlighted'></pre>
      </div>
    </div>
    <div class='result-meta'>
      <span>changed lines: {{{{ dyffChangedCount }}}}</span>
      <span>ignore order: {{{{ dyffIgnoreOrder ? "on" : "off" }}}}</span>
      <span>ignore whitespace: {{{{ dyffIgnoreWhitespace ? "on" : "off" }}}}</span>
    </div>
    <div class='err' v-if='dyffError' style='margin-top:8px;'>{{{{ dyffError }}}}</div>
  </div>

  <div v-if='fsPickerOpen' class='fs-modal-backdrop'>
    <div class='fs-modal'>
      <div class='fs-head'>
        <strong>{{{{ fsPickerTarget === "chart-output" ? "Choose output chart directory on server" : "Choose source on server" }}}}</strong>
        <span class='fs-badge'>{{{{ fsPickerTarget === "chart-output" ? "chart-output" : mainImportSourceType }}}}</span>
        <button class='secondary' @click='closeFsPicker'>Close</button>
      </div>
      <div class='fs-head'>
        <input type='text' v-model='fsPickerPath' placeholder='Server directory path'/>
        <button class='secondary' @click='loadFsEntries(fsPickerPath)'>Open</button>
        <button class='secondary' @click='goFsParent' :disabled='!fsPickerParent'>Up</button>
      </div>
      <div class='fs-toolbar'>
        <input type='text' v-model='fsPickerQuery' placeholder='Filter by name/path'/>
        <label class='chk'><input type='checkbox' v-model='fsPickerShowHidden'/> show hidden</label>
        <label class='chk'><input type='checkbox' v-model='fsPickerOnlySelectable'/> only selectable</label>
      </div>
      <div class='muted'>Current directory: {{{{ fsPickerCurrent || '-' }}}}</div>
      <div class='fs-list'>
        <table class='fs-table'>
          <thead>
            <tr>
              <th style='width:70px;'>Type</th>
              <th style='width:320px;'>Name</th>
              <th>Path</th>
              <th style='width:160px; text-align:right;'>Action</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for='e in fsPickerFilteredEntries'
              :key='e.path'
              class='fs-row clickable'
              :class='{{ "hidden-file": isHiddenFile(e) }}'
              @dblclick='onFsRowActivate(e)'
            >
              <td>{{{{ e.isDir ? "DIR" : "FILE" }}}}</td>
              <td>
                <div>{{{{ e.name }}}}</div>
                <div class='fs-subpath'>{{{{ e.path }}}}</div>
              </td>
              <td><div class='fs-subpath'>{{{{ e.path }}}}</div></td>
              <td>
                <div class='fs-actions'>
                  <button v-if='e.isDir' class='secondary' @click='loadFsEntries(e.path)'>Open</button>
                  <button v-if='isFsEntrySelectable(e)' class='secondary' @click='selectFsPath(e.path)'>Select</button>
                </div>
              </td>
            </tr>
            <tr v-if='!fsPickerFilteredEntries.length'>
              <td colspan='4' class='muted' style='padding:10px;'>No entries matched current filters</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class='err' v-if='fsPickerError'>{{{{ fsPickerError }}}}</div>
    </div>
  </div>
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
const APP_STORE_KEY = 'happ.inspect.ui.v7';
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
      converterHexOutFormat: 'all',
      converterHexView: 'dump',
      converterHexInFormat: 'auto',
      converterHexUppercase: false,
      converterHexSeparator: '',
      converterHexBytesPerLine: 16,
      converterHexLastBytes: [],
      converterHexSelStart: null,
      converterHexSelEnd: null,
      converterHexSelecting: false,
      converterPlainRanges: [],
      converterPlainCursor: null,
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
      yqQuery: '.',
      yqInput: '',
      yqOutput: '',
      yqError: '',
      yqDocMode: 'first',
      yqDocIndex: 0,
      yqCompact: false,
      yqRawOutput: false,
      yqRequestSeq: 0,
      yqTimer: null,
      dyffFrom: '',
      dyffTo: '',
      dyffOutput: '',
      dyffError: '',
      dyffIgnoreOrder: false,
      dyffIgnoreWhitespace: false,
      dyffRequestSeq: 0,
      dyffTimer: null,
      mainImportSourceType: 'chart',
      mainImportPath: '',
      mainImportEnv: 'dev',
      mainImportGroupName: 'apps-k8s-manifests',
      mainImportGroupType: 'apps-k8s-manifests',
      mainImportImportStrategy: 'helpers',
      mainImportReleaseName: 'imported',
      mainImportNamespace: '',
      mainImportMinIncludeBytes: 24,
      mainImportIncludeStatus: false,
      mainImportIncludeCrds: false,
      mainImportKubeVersion: '',
      mainImportValuesFilesText: '',
      mainImportSetText: '',
      mainImportExtraSetText: '',
      mainImportApiVersionsText: '',
      mainImportOutput: '',
      mainImportError: '',
      mainImportMessage: '',
      mainImportSourceCount: 0,
      mainImportRunning: false,
      mainImportConfigOpen: false,
      mainImportChartValuesEditor: '',
      mainImportLoadedChartValues: '',
      mainImportUseChartValuesEditor: false,
      mainImportStdinText: (model && typeof model.stdinText === 'string') ? model.stdinText : '',
      mainImportSectionCollapsed: {{}},
      mainImportPickedFilesLabel: '',
      mainImportUploadedFiles: [],
      mainImportOutChartDir: '',
      mainImportOutChartName: '',
      mainImportLibraryChartPath: '',
      mainImportSaveChartOpen: false,
      mainImportSaveChartMessage: '',
      mainImportSaveChartError: '',
      mainImportSaveChartRunning: false,
      mainImportCompareRunning: false,
      mainImportCompareError: '',
      mainImportCompareMessage: '',
      mainImportCompareSummary: '',
      mainImportCompareEqual: false,
      mainImportCompareSourceCount: 0,
      mainImportCompareGeneratedCount: 0,
      cmAvailable: false,
      cmProbeReason: '',
      cmLoadAttempted: false,
      mainImportSourceCm: null,
      mainImportGeneratedCm: null,
      mainImportSourceCmUpdating: false,
      converterInputCm: null,
      converterOutputCm: null,
      jqInputCm: null,
      jqOutputCm: null,
      yqQueryCm: null,
      yqInputCm: null,
      yqOutputCm: null,
      dyffFromCm: null,
      dyffToCm: null,
      dyffOutputCm: null,
      mainImportSelection: null,
      converterSelection: null,
      jqSelection: null,
      yqSelection: null,
      dyffFromSelection: null,
      dyffToSelection: null,
      requestAbortControllers: {{}},
      fsPickerOpen: false,
      fsPickerTarget: 'source-path',
      fsPickerPath: '',
      fsPickerCurrent: '',
      fsPickerParent: '',
      fsPickerEntries: [],
      fsPickerQuery: '',
      fsPickerShowHidden: true,
      fsPickerOnlySelectable: false,
      fsPickerError: '',
      yqPresets: [
        {{ label: 'identity', query: '.' }},
        {{ label: 'keys', query: 'keys' }},
        {{ label: 'length', query: 'length' }},
        {{ label: 'select enabled', query: '.[] | select(.enabled == true)' }},
      ],
      jqSuggestOpen: false,
      jqSuggestIndex: 0,
      jqPresets: [
        {{ label: 'identity', query: '.' }},
        {{ label: 'keys', query: 'keys' }},
        {{ label: 'length', query: 'length' }},
        {{ label: 'list names', query: '.[] | .name' }},
        {{ label: 'select enabled', query: '.[] | select(.enabled == true)' }},
        {{ label: 'map image', query: '.[] | .image' }},
        {{ label: 'compact', query: '.[] | tostring' }},
      ],
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
    fsPickerFilteredEntries() {{
      const q = String(this.fsPickerQuery || '').toLowerCase().trim();
      const showHidden = !!this.fsPickerShowHidden;
      const onlySelectable = !!this.fsPickerOnlySelectable;
      return (this.fsPickerEntries || []).filter((e) => {{
        const name = String(e.name || '');
        if (!showHidden && name.startsWith('.')) {{
          return false;
        }}
        if (onlySelectable && !this.isFsEntrySelectable(e)) {{
          return false;
        }}
        if (!q) return true;
        const p = String(e.path || '').toLowerCase();
        return name.toLowerCase().includes(q) || p.includes(q);
      }});
    }},
    converterModeLabel() {{
      const map = {{
        'yaml-to-json': 'YAML → JSON',
        'json-to-yaml': 'JSON → YAML',
        'base64-encode': 'Base64 encode',
        'base64-decode': 'Base64 decode',
        'url-encode': 'URL encode',
        'url-decode': 'URL decode',
        'jwt-inspect': 'JWT inspect',
        'unix-to-iso': 'Unix → ISO8601',
        'iso-to-unix': 'ISO8601 → Unix',
        'text-to-hex': 'Text → HEX',
        'hex-to-text': 'HEX → Text',
      }};
      return map[this.converterMode] || this.converterMode;
    }},
    converterOutputUseCm() {{
      return this.converterMode === 'yaml-to-json'
        || this.converterMode === 'json-to-yaml'
        || this.converterMode === 'jwt-inspect';
    }},
    jqQueryHighlighted() {{
      return this.highlightJq(this.jqQuery || '');
    }},
    converterOutputHighlighted() {{
      return this.highlightStructured(this.converterOutput || '');
    }},
    converterOutputRich() {{
      if (this.converterMode === 'text-to-hex') {{
        return this.highlightHexOutput(this.converterOutput || '', this.converterHexView || 'dump');
      }}
      if (!this.converterOutputUseCm) {{
        return this.renderTextWithSyncOverlay(
          this.converterOutput || '',
          this.converterPlainRanges || [],
          this.converterPlainCursor
        );
      }}
      return this.highlightStructured(this.converterOutput || '');
    }},
    converterHexDumpInteractive() {{
      return this.converterMode === 'text-to-hex' && this.converterHexView === 'dump';
    }},
    converterHexDumpRows() {{
      if (!this.converterHexDumpInteractive) return [];
      const src = Array.isArray(this.converterHexLastBytes) ? this.converterHexLastBytes : [];
      const lineSize = Math.max(4, Math.min(64, Number(this.converterHexBytesPerLine || 16)));
      const rows = [];
      for (let offset = 0; offset < src.length; offset += lineSize) {{
        const chunk = src.slice(offset, offset + lineSize);
        const bytes = [];
        const ascii = [];
        for (let i = 0; i < lineSize; i += 1) {{
          const sep8 = i > 0 && i % 8 === 0;
          if (i < chunk.length) {{
            const idx = offset + i;
            const n = Number(chunk[i]) & 0xff;
            const hex = n.toString(16).padStart(2, '0');
            const asciiCh = (n >= 33 && n <= 126) ? String.fromCharCode(n) : '.';
            bytes.push({{
              key: String(idx),
              idx,
              real: true,
              hex: this.converterHexUppercase ? hex.toUpperCase() : hex,
              sep8,
            }});
            ascii.push({{
              key: String(idx),
              idx,
              real: true,
              ch: asciiCh,
            }});
          }} else {{
            const padKey = String(offset + i) + '-pad';
            bytes.push({{
              key: padKey,
              idx: null,
              real: false,
              hex: '00',
              sep8,
            }});
            ascii.push({{
              key: padKey,
              idx: null,
              real: false,
              ch: '.',
            }});
          }}
        }}
        const utf8 = this.utf8TokensWithRanges(chunk, offset);
        rows.push({{
          offset,
          offsetHex: offset.toString(16).padStart(8, '0'),
          bytes,
          ascii,
          utf8,
        }});
      }}
      return rows;
    }},
    jqOutputHighlighted() {{
      return this.highlightStructured(this.jqOutput || '');
    }},
    yqOutputHighlighted() {{
      return this.highlightStructured(this.yqOutput || '');
    }},
    dyffOutputHighlighted() {{
      return this.highlightDyff(this.dyffOutput || '');
    }},
    mainImportSourceHighlighted() {{
      return this.highlightStructured(this.mainImportChartValuesEditor || '');
    }},
    mainImportPreview() {{
      const _collapsed = this.mainImportSectionCollapsed;
      return this.buildMainImportYamlPreview(this.mainImportOutput || '');
    }},
    mainImportPreviewHtml() {{
      return this.mainImportPreview.html || '';
    }},
    mainImportPreviewMeta() {{
      return this.mainImportPreview.meta || [];
    }},
    jqSuggestions() {{
      const meta = this.currentJqTokenMeta();
      const token = (meta.term || '').toLowerCase();
      const fieldMode = meta.kind === 'field';
      const out = [];
      if(fieldMode) {{
        const keyPrefix = token.includes('.') ? token.split('.').pop() : token;
        for (const key of this.jqInputKeys) {{
          if (!keyPrefix || key.toLowerCase().startsWith(keyPrefix)) {{
            out.push({{
              label: this.jqFieldSnippet(key),
              snippet: this.jqFieldSnippet(key),
              cursor: 0,
              kind: 'field',
              desc: 'Field name from current input'
            }});
          }}
          if (out.length >= 20) break;
        }}
      }}
      const fnSuggestions = (!fieldMode && !token ? this.jqCatalog : this.jqCatalog
        .filter(x => x.label.toLowerCase().startsWith(token) || x.snippet.toLowerCase().startsWith(token)))
        .slice(0, 10);
      return out.concat(fnSuggestions).slice(0, 20);
    }},
    jqSuggestPanelStyle() {{
      const maxPanel = 460;
      const minPanel = 280;
      const editor = this.$el ? this.$el.querySelector('.jq-query-editor') : null;
      const editorWidth = editor && editor.clientWidth ? (editor.clientWidth - 12) : maxPanel;
      const labels = this.jqSuggestions || [];
      let longest = 0;
      for (const s of labels) {{
        const l = String((s && s.label) || '').length;
        if (l > longest) longest = l;
      }}
      const byContent = 220 + (longest * 9);
      const width = Math.max(minPanel, Math.min(maxPanel, Math.min(editorWidth, byContent)));
      return {{ width: width + 'px' }};
    }},
    jqActiveSuggestionHint() {{
      if(!this.jqSuggestions.length) return 'No suggestions';
      const idx = Math.min(this.jqSuggestIndex, this.jqSuggestions.length - 1);
      const s = this.jqSuggestions[idx];
      return s ? (s.label + ' — ' + s.desc) : 'No suggestions';
    }},
    jqInputKeys() {{
      return this.extractInputKeys(this.jqInput || '');
    }},
    jqResultCount() {{
      const t = (this.jqOutput || '').trim();
      if(!t) return 0;
      return t.split(/\n+/).filter(Boolean).length;
    }},
    yqResultCount() {{
      const t = (this.yqOutput || '').trim();
      if(!t) return 0;
      return t.split(/\n+/).filter(Boolean).length;
    }},
    dyffChangedCount() {{
      const t = this.dyffOutput || '';
      if(!t) return 0;
      return t.split('\n').filter(l => /^changed: |^added: |^removed: /.test(l)).length;
    }},
    mainImportPathPlaceholder() {{
      if (this.mainImportSourceType === 'compose') {{
        return 'Path to docker-compose.yaml on server';
      }}
      if (this.mainImportSourceType === 'manifests') {{
        return 'Path to manifests file or directory on server';
      }}
      return 'Path to Helm chart directory on server';
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
        this.converterHexOutFormat = s.converterHexOutFormat || 'all';
        this.converterHexView = s.converterHexView || 'dump';
        this.converterHexInFormat = s.converterHexInFormat || 'auto';
        this.converterHexUppercase = !!s.converterHexUppercase;
        this.converterHexSeparator = s.converterHexSeparator || '';
        this.converterHexBytesPerLine = Number.isFinite(s.converterHexBytesPerLine) ? Number(s.converterHexBytesPerLine) : 16;
        this.jqQuery = s.jqQuery || '.';
        this.jqInput = s.jqInput || '';
        this.jqDocMode = s.jqDocMode || 'first';
        this.jqDocIndex = Number.isFinite(s.jqDocIndex) ? Number(s.jqDocIndex) : 0;
        this.jqCompact = !!s.jqCompact;
        this.jqRawOutput = !!s.jqRawOutput;
        this.yqQuery = s.yqQuery || '.';
        this.yqInput = s.yqInput || '';
        this.yqDocMode = s.yqDocMode || 'first';
        this.yqDocIndex = Number.isFinite(s.yqDocIndex) ? Number(s.yqDocIndex) : 0;
        this.yqCompact = !!s.yqCompact;
        this.yqRawOutput = !!s.yqRawOutput;
        this.dyffFrom = s.dyffFrom || '';
        this.dyffTo = s.dyffTo || '';
        this.dyffIgnoreOrder = !!s.dyffIgnoreOrder;
        this.dyffIgnoreWhitespace = !!s.dyffIgnoreWhitespace;
        this.mainImportSourceType = s.mainImportSourceType || 'chart';
        this.mainImportPath = s.mainImportPath || '';
        this.mainImportEnv = s.mainImportEnv || 'dev';
        this.mainImportGroupName = s.mainImportGroupName || 'apps-k8s-manifests';
        this.mainImportGroupType = s.mainImportGroupType || 'apps-k8s-manifests';
        this.mainImportImportStrategy = (s.mainImportImportStrategy || 'helpers');
        this.mainImportReleaseName = s.mainImportReleaseName || 'imported';
        this.mainImportNamespace = s.mainImportNamespace || '';
        this.mainImportMinIncludeBytes = Number.isFinite(s.mainImportMinIncludeBytes) ? Number(s.mainImportMinIncludeBytes) : 24;
        this.mainImportIncludeStatus = !!s.mainImportIncludeStatus;
        this.mainImportIncludeCrds = !!s.mainImportIncludeCrds;
        this.mainImportKubeVersion = s.mainImportKubeVersion || '';
        this.mainImportValuesFilesText = s.mainImportValuesFilesText || '';
        this.mainImportSetText = s.mainImportSetText || '';
        this.mainImportExtraSetText = s.mainImportExtraSetText || '';
        this.mainImportApiVersionsText = s.mainImportApiVersionsText || '';
        this.mainImportPickedFilesLabel = s.mainImportPickedFilesLabel || '';
        this.mainImportConfigOpen = !!s.mainImportConfigOpen;
        this.mainImportChartValuesEditor = s.mainImportChartValuesEditor || '';
        this.mainImportLoadedChartValues = s.mainImportLoadedChartValues || '';
        this.mainImportUseChartValuesEditor = !!s.mainImportUseChartValuesEditor;
        this.mainImportSectionCollapsed = s.mainImportSectionCollapsed || {{}};
        this.mainImportOutChartDir = s.mainImportOutChartDir || '';
        this.mainImportOutChartName = s.mainImportOutChartName || '';
        this.mainImportLibraryChartPath = s.mainImportLibraryChartPath || '';
        this.mainImportSaveChartOpen = !!s.mainImportSaveChartOpen;
        this.fsPickerQuery = s.fsPickerQuery || '';
        this.fsPickerShowHidden = s.fsPickerShowHidden !== false;
        this.fsPickerOnlySelectable = !!s.fsPickerOnlySelectable;
      }}
    }} catch(_) {{}}
    if(!(this.utilities || []).some(u => u.id === this.activeUtilityId)) {{
      this.activeUtilityId = (this.utilities[0] && this.utilities[0].id) || 'main';
    }}
    this.scheduleConvert();
    this.scheduleJqRun();
    this.scheduleYqRun();
    this.scheduleDyffRun();
    this.refreshCodeMirrorAvailability();
    this.$nextTick(() => {{
      this.syncMainImportSourceScroll();
      this.initMainImportCodeMirror();
      this.initToolCodeMirror();
      this.syncMainImportEditorHeights();
    }});
    setTimeout(() => {{
      this.initMainImportCodeMirror();
      this.initToolCodeMirror();
      this.syncMainImportEditorHeights();
    }}, 120);
    setTimeout(() => {{
      this.initMainImportCodeMirror();
      this.initToolCodeMirror();
      this.syncMainImportEditorHeights();
    }}, 500);
    if (!this.cmAvailable) {{
      this.ensureCodeMirrorScriptLoaded().then(() => {{
        this.$nextTick(() => {{
          this.initMainImportCodeMirror();
          this.initToolCodeMirror();
          this.syncMainImportEditorHeights();
        }});
      }});
    }}
    window.addEventListener('resize', this.syncMainImportEditorHeights);
  }},
  beforeUnmount() {{
    this.abortAllRequests();
    this.destroyMainImportCodeMirror();
    this.destroyToolCodeMirror();
    window.removeEventListener('resize', this.syncMainImportEditorHeights);
  }},
  watch: {{
    wrapLines() {{
      this.saveSettings();
      for (const cm of this.getAllCodeMirrorEditors()) cm.setWrapLines(this.wrapLines);
    }},
    fontSize() {{
      this.saveSettings();
      for (const cm of this.getAllCodeMirrorEditors()) cm.setFontSize(this.fontSize);
    }},
    collapsedTitles: {{ handler: 'saveSettings', deep: true }},
    activeUtilityId() {{
      this.saveSettings();
      this.destroyMainImportCodeMirror();
      this.destroyToolCodeMirror();
      this.$nextTick(() => {{
        this.initMainImportCodeMirror();
        this.initToolCodeMirror();
        this.syncMainImportEditorHeights();
      }});
    }},
    converterMode() {{
      this.saveSettings();
      this.clearHexSelection();
      this.scheduleConvert();
      this.$nextTick(() => this.initToolCodeMirror());
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
      if (this.converterInputCm) this.converterInputCm.setValue(this.converterInput || '');
      this.scheduleConvert();
    }},
    converterHexOutFormat() {{
      this.saveSettings();
      this.clearHexSelection();
      this.scheduleConvert();
    }},
    converterHexView() {{
      this.saveSettings();
      this.clearHexSelection();
      this.scheduleConvert();
    }},
    converterHexInFormat() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    converterHexUppercase() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    converterHexSeparator() {{
      this.saveSettings();
      this.scheduleConvert();
    }},
    converterHexBytesPerLine() {{
      this.saveSettings();
      this.clearHexSelection();
      this.scheduleConvert();
    }},
    converterOutput() {{
      if (this.converterOutputCm) this.converterOutputCm.setValue(this.converterOutput || '');
      this.applyConverterSelectionSync();
    }},
    jqQuery() {{
      this.saveSettings();
      this.scheduleJqRun();
    }},
    jqInput() {{
      this.saveSettings();
      if (this.jqInputCm) this.jqInputCm.setValue(this.jqInput || '');
      this.scheduleJqRun();
    }},
    jqOutput() {{
      if (this.jqOutputCm) this.jqOutputCm.setValue(this.jqOutput || '');
      this.applyJqSelectionSync();
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
    }},
    yqQuery() {{
      this.saveSettings();
      if (this.yqQueryCm) this.yqQueryCm.setValue(this.yqQuery || '');
      this.scheduleYqRun();
    }},
    yqInput() {{
      this.saveSettings();
      if (this.yqInputCm) this.yqInputCm.setValue(this.yqInput || '');
      this.scheduleYqRun();
    }},
    yqOutput() {{
      if (this.yqOutputCm) this.yqOutputCm.setValue(this.yqOutput || '');
      this.applyYqSelectionSync();
    }},
    yqDocMode() {{
      this.saveSettings();
      this.scheduleYqRun();
    }},
    yqDocIndex() {{
      this.saveSettings();
      this.scheduleYqRun();
    }},
    yqCompact() {{
      this.saveSettings();
      this.scheduleYqRun();
    }},
    yqRawOutput() {{
      this.saveSettings();
      this.scheduleYqRun();
    }},
    dyffFrom() {{
      this.saveSettings();
      if (this.dyffFromCm) this.dyffFromCm.setValue(this.dyffFrom || '');
      this.scheduleDyffRun();
    }},
    dyffTo() {{
      this.saveSettings();
      if (this.dyffToCm) this.dyffToCm.setValue(this.dyffTo || '');
      this.scheduleDyffRun();
    }},
    dyffOutput() {{
      if (this.dyffOutputCm) this.dyffOutputCm.setValue(this.dyffOutput || '');
      this.applyDyffSelectionSync();
    }},
    dyffIgnoreOrder() {{
      this.saveSettings();
      this.scheduleDyffRun();
    }},
    dyffIgnoreWhitespace() {{
      this.saveSettings();
      this.scheduleDyffRun();
    }},
    mainImportSourceType() {{
      this.saveSettings();
    }},
    mainImportPath: 'saveSettings',
    mainImportEnv: 'saveSettings',
    mainImportGroupName: 'saveSettings',
    mainImportGroupType: 'saveSettings',
    mainImportImportStrategy: 'saveSettings',
    mainImportReleaseName: 'saveSettings',
    mainImportNamespace: 'saveSettings',
    mainImportMinIncludeBytes: 'saveSettings',
    mainImportIncludeStatus: 'saveSettings',
    mainImportIncludeCrds: 'saveSettings',
    mainImportKubeVersion: 'saveSettings',
    mainImportValuesFilesText: 'saveSettings',
    mainImportSetText: 'saveSettings',
    mainImportExtraSetText: 'saveSettings',
    mainImportApiVersionsText: 'saveSettings',
    mainImportConfigOpen: 'saveSettings',
    mainImportOutput() {{
      this.saveSettings();
      if (this.mainImportGeneratedCm) {{
        this.mainImportGeneratedCm.setValue(this.mainImportOutput || '');
      }}
      this.applyMainImportSelectionSync();
      this.$nextTick(() => this.syncMainImportEditorHeights());
    }},
    mainImportChartValuesEditor() {{
      this.saveSettings();
      if (this.mainImportSourceCm && !this.mainImportSourceCmUpdating) {{
        this.mainImportSourceCm.setValue(this.mainImportChartValuesEditor || '');
      }}
      this.$nextTick(() => this.syncMainImportSourceScroll());
      this.$nextTick(() => this.syncMainImportEditorHeights());
    }},
    mainImportUseChartValuesEditor: 'saveSettings',
    mainImportSectionCollapsed: {{ handler: 'saveSettings', deep: true }},
    mainImportOutChartDir: 'saveSettings',
    mainImportOutChartName: 'saveSettings',
    mainImportLibraryChartPath: 'saveSettings',
    mainImportSaveChartOpen: 'saveSettings',
    fsPickerQuery: 'saveSettings',
    fsPickerShowHidden: 'saveSettings',
    fsPickerOnlySelectable: 'saveSettings',
  }},
  methods: {{
    isAbortError(err) {{
      const name = String(err && err.name ? err.name : '');
      if (name === 'AbortError') return true;
      const msg = String(err || '').toLowerCase();
      return msg.includes('abort');
    }},
    beginAbortableRequest(key) {{
      const k = String(key || '');
      if (!k) return null;
      const map = this.requestAbortControllers || {{}};
      const prev = map[k];
      if (prev && typeof prev.abort === 'function') {{
        try {{ prev.abort(); }} catch(_) {{}}
      }}
      const ctrl = (typeof AbortController !== 'undefined') ? new AbortController() : null;
      map[k] = ctrl;
      this.requestAbortControllers = map;
      return ctrl;
    }},
    finishAbortableRequest(key, ctrl) {{
      const k = String(key || '');
      if (!k) return;
      const map = this.requestAbortControllers || {{}};
      if (map[k] === ctrl) {{
        delete map[k];
        this.requestAbortControllers = map;
      }}
    }},
    abortAllRequests() {{
      const map = this.requestAbortControllers || {{}};
      for (const k of Object.keys(map)) {{
        const ctrl = map[k];
        if (ctrl && typeof ctrl.abort === 'function') {{
          try {{ ctrl.abort(); }} catch(_) {{}}
        }}
      }}
      this.requestAbortControllers = {{}};
    }},
    getCodeMirrorApi() {{
      try {{
        if (typeof window === 'undefined') return null;
        const fromWindow = window.HappCodeMirror;
        if (fromWindow) return fromWindow;
        if (window.globalThis && window.globalThis.HappCodeMirror) return window.globalThis.HappCodeMirror;
        if (typeof globalThis !== 'undefined' && globalThis.HappCodeMirror) return globalThis.HappCodeMirror;
      }} catch(_) {{}}
      return null;
    }},
    refreshCodeMirrorAvailability() {{
      const api = this.getCodeMirrorApi();
      this.cmAvailable = !!(api && typeof api.createYamlEditor === 'function');
      if (this.cmAvailable) {{
        this.cmProbeReason = '';
      }} else if (!api) {{
        let jsErr = '';
        let afterScript = '';
        let scriptLoad = '';
        try {{
          const arr = window.__happScriptErrors || [];
          if (arr.length) {{
            const last = arr[arr.length - 1] || {{}};
            const where = [last.file || '', last.line ? (':' + last.line) : '', last.col ? (':' + last.col) : ''].join('');
            jsErr = (last.message || 'unknown script error') + (where ? (' @ ' + where) : '');
          }}
          if (typeof window.__happCmAfterScript !== 'undefined') {{
            afterScript = ' after-script=' + String(window.__happCmAfterScript);
          }}
          const loaded = typeof window.__happCmScriptLoaded !== 'undefined'
            ? String(window.__happCmScriptLoaded)
            : 'undefined';
          const loadErr = typeof window.__happCmScriptError !== 'undefined'
            ? String(window.__happCmScriptError)
            : 'none';
          const entryReached = typeof window.__happCmEntryReached !== 'undefined'
            ? String(window.__happCmEntryReached)
            : 'false';
          const beforeAssign = typeof window.__happCmBeforeAssign !== 'undefined'
            ? String(window.__happCmBeforeAssign)
            : 'false';
          const afterAssign = typeof window.__happCmAfterAssign !== 'undefined'
            ? String(window.__happCmAfterAssign)
            : 'false';
          scriptLoad = ' loaded=' + loaded + ' load-error=' + loadErr + ' entry=' + entryReached + ' before=' + beforeAssign + ' after=' + afterAssign;
        }} catch(_) {{}}
        this.cmProbeReason = jsErr
          ? ('CodeMirror script error: ' + jsErr + afterScript + scriptLoad)
          : ('CodeMirror API object is missing (window.HappCodeMirror)' + afterScript + scriptLoad);
      }} else {{
        this.cmProbeReason = 'CodeMirror API loaded without createYamlEditor()';
      }}
      return this.cmAvailable;
    }},
    ensureCodeMirrorScriptLoaded() {{
      if (this.refreshCodeMirrorAvailability()) return Promise.resolve(true);
      if (this.cmLoadAttempted) return Promise.resolve(false);
      this.cmLoadAttempted = true;
      return new Promise((resolve) => {{
        try {{
          const script = document.createElement('script');
          script.src = '/assets/codemirror.bundle.js?reload=' + Date.now();
          script.async = true;
          script.onload = () => {{
            const ok = this.refreshCodeMirrorAvailability();
            if (!ok && !this.cmProbeReason) {{
              this.cmProbeReason = 'CodeMirror script loaded but API is unavailable';
            }}
            resolve(ok);
          }};
          script.onerror = () => {{
            this.cmProbeReason = 'Failed to load /assets/codemirror.bundle.js';
            resolve(false);
          }};
          document.head.appendChild(script);
        }} catch(e) {{
          this.cmProbeReason = 'CodeMirror dynamic load error: ' + String(e);
          resolve(false);
        }}
      }});
    }},
    destroyMainImportCodeMirror() {{
      if (this.mainImportSourceCm) {{
        try {{ this.mainImportSourceCm.destroy(); }} catch(_) {{}}
        this.mainImportSourceCm = null;
      }}
      if (this.mainImportGeneratedCm) {{
        try {{ this.mainImportGeneratedCm.destroy(); }} catch(_) {{}}
        this.mainImportGeneratedCm = null;
      }}
    }},
    initMainImportCodeMirror() {{
      if (!this.refreshCodeMirrorAvailability()) return;
      if (this.activeUtilityKey !== 'main-import') return;
      const cmApi = this.getCodeMirrorApi();
      if (!cmApi || typeof cmApi.createYamlEditor !== 'function') return;

      try {{
        if (!this.mainImportSourceCm) {{
          const host = this.$refs.mainImportSourceCmHost;
          if (host) {{
            this.mainImportSourceCm = cmApi.createYamlEditor(host, {{
              value: this.mainImportChartValuesEditor || '',
              readOnly: false,
              wrapLines: this.wrapLines,
              fontSize: this.fontSize,
              onChange: (next) => {{
                this.mainImportSourceCmUpdating = true;
                this.mainImportChartValuesEditor = next;
                this.mainImportSourceCmUpdating = false;
              }},
              onSelectionChange: (sel) => {{
                this.onMainImportSelection(sel);
              }},
            }});
          }}
        }}

        if (!this.mainImportGeneratedCm) {{
          const host = this.$refs.mainImportGeneratedCmHost;
          if (host) {{
            this.mainImportGeneratedCm = cmApi.createYamlEditor(host, {{
              value: this.mainImportOutput || '',
              readOnly: true,
              wrapLines: this.wrapLines,
              fontSize: this.fontSize,
            }});
          }}
        }}
        this.applyMainImportSelectionSync();
      }} catch(e) {{
        this.cmAvailable = false;
        this.destroyMainImportCodeMirror();
        if (!this.mainImportError) {{
          this.mainImportError = 'CodeMirror initialization failed, switched to fallback editor: ' + String(e);
        }}
      }}
    }},
    getAllCodeMirrorEditors() {{
      return [
        this.mainImportSourceCm,
        this.mainImportGeneratedCm,
        this.converterInputCm,
        this.converterOutputCm,
        this.jqInputCm,
        this.jqOutputCm,
        this.yqQueryCm,
        this.yqInputCm,
        this.yqOutputCm,
        this.dyffFromCm,
        this.dyffToCm,
        this.dyffOutputCm,
      ].filter(Boolean);
    }},
    destroyToolCodeMirror() {{
      const keys = [
        'converterInputCm',
        'converterOutputCm',
        'jqInputCm',
        'jqOutputCm',
        'yqQueryCm',
        'yqInputCm',
        'yqOutputCm',
        'dyffFromCm',
        'dyffToCm',
        'dyffOutputCm',
      ];
      for (const key of keys) {{
        const inst = this[key];
        if (!inst) continue;
        try {{ inst.destroy(); }} catch(_) {{}}
        this[key] = null;
      }}
    }},
    ensureToolEditor(instanceKey, hostRef, options) {{
      const cmApi = this.getCodeMirrorApi();
      if (!cmApi || typeof cmApi.createYamlEditor !== 'function') return null;
      if (this[instanceKey]) return this[instanceKey];
      const host = this.$refs[hostRef];
      if (!host) return null;
      this[instanceKey] = cmApi.createYamlEditor(host, options || {{}});
      return this[instanceKey];
    }},
    initConverterCodeMirror() {{
      if (!this.cmAvailable) return;
      if (this.activeUtilityKey !== 'converter') return;
      this.ensureToolEditor('converterInputCm', 'converterInputCmHost', {{
        value: this.converterInput || '',
        readOnly: false,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
        onChange: (next) => {{ this.converterInput = next; }},
        onSelectionChange: (sel) => {{ this.onConverterSelection(sel); }},
      }});
      if (this.converterOutputUseCm) {{
        this.ensureToolEditor('converterOutputCm', 'converterOutputCmHost', {{
          value: this.converterOutput || '',
          readOnly: true,
          wrapLines: this.wrapLines,
          fontSize: this.fontSize,
        }});
        this.applyConverterSelectionSync();
      }} else if (this.converterOutputCm) {{
        try {{ this.converterOutputCm.destroy(); }} catch(_) {{}}
        this.converterOutputCm = null;
      }}
    }},
    initJqCodeMirror() {{
      if (!this.cmAvailable) return;
      if (this.activeUtilityKey !== 'jq-playground') return;
      this.ensureToolEditor('jqInputCm', 'jqInputCmHost', {{
        value: this.jqInput || '',
        readOnly: false,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
        onChange: (next) => {{ this.jqInput = next; }},
        onSelectionChange: (sel) => {{ this.onJqSelection(sel); }},
      }});
      this.ensureToolEditor('jqOutputCm', 'jqOutputCmHost', {{
        value: this.jqOutput || '',
        readOnly: true,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
      }});
      this.applyJqSelectionSync();
    }},
    initYqCodeMirror() {{
      if (!this.cmAvailable) return;
      if (this.activeUtilityKey !== 'yq-playground') return;
      this.ensureToolEditor('yqQueryCm', 'yqQueryCmHost', {{
        value: this.yqQuery || '',
        readOnly: false,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
        onChange: (next) => {{ this.yqQuery = next; }},
      }});
      this.ensureToolEditor('yqInputCm', 'yqInputCmHost', {{
        value: this.yqInput || '',
        readOnly: false,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
        onChange: (next) => {{ this.yqInput = next; }},
        onSelectionChange: (sel) => {{ this.onYqSelection(sel); }},
      }});
      this.ensureToolEditor('yqOutputCm', 'yqOutputCmHost', {{
        value: this.yqOutput || '',
        readOnly: true,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
      }});
      this.applyYqSelectionSync();
    }},
    initDyffCodeMirror() {{
      if (!this.cmAvailable) return;
      if (this.activeUtilityKey !== 'dyff-compare') return;
      this.ensureToolEditor('dyffFromCm', 'dyffFromCmHost', {{
        value: this.dyffFrom || '',
        readOnly: false,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
        onChange: (next) => {{ this.dyffFrom = next; }},
        onSelectionChange: (sel) => {{ this.onDyffFromSelection(sel); }},
      }});
      this.ensureToolEditor('dyffToCm', 'dyffToCmHost', {{
        value: this.dyffTo || '',
        readOnly: false,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
        onChange: (next) => {{ this.dyffTo = next; }},
        onSelectionChange: (sel) => {{ this.onDyffToSelection(sel); }},
      }});
      this.ensureToolEditor('dyffOutputCm', 'dyffOutputCmHost', {{
        value: this.dyffOutput || '',
        readOnly: true,
        wrapLines: this.wrapLines,
        fontSize: this.fontSize,
      }});
      this.applyDyffSelectionSync();
    }},
    initToolCodeMirror() {{
      if (!this.refreshCodeMirrorAvailability()) return;
      try {{
        if (this.activeUtilityKey === 'converter') return this.initConverterCodeMirror();
        if (this.activeUtilityKey === 'jq-playground') return this.initJqCodeMirror();
        if (this.activeUtilityKey === 'yq-playground') return this.initYqCodeMirror();
        if (this.activeUtilityKey === 'dyff-compare') return this.initDyffCodeMirror();
      }} catch(e) {{
        this.destroyToolCodeMirror();
      }}
    }},
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
          converterHexOutFormat: this.converterHexOutFormat,
          converterHexView: this.converterHexView,
          converterHexInFormat: this.converterHexInFormat,
          converterHexUppercase: this.converterHexUppercase,
          converterHexSeparator: this.converterHexSeparator,
          converterHexBytesPerLine: this.converterHexBytesPerLine,
          jqQuery: this.jqQuery,
          jqInput: this.jqInput,
          jqDocMode: this.jqDocMode,
          jqDocIndex: this.jqDocIndex,
          jqCompact: this.jqCompact,
          jqRawOutput: this.jqRawOutput,
          yqQuery: this.yqQuery,
          yqInput: this.yqInput,
          yqDocMode: this.yqDocMode,
          yqDocIndex: this.yqDocIndex,
          yqCompact: this.yqCompact,
          yqRawOutput: this.yqRawOutput,
          dyffFrom: this.dyffFrom,
          dyffTo: this.dyffTo,
          dyffIgnoreOrder: this.dyffIgnoreOrder,
          dyffIgnoreWhitespace: this.dyffIgnoreWhitespace,
          mainImportSourceType: this.mainImportSourceType,
          mainImportPath: this.mainImportPath,
          mainImportEnv: this.mainImportEnv,
          mainImportGroupName: this.mainImportGroupName,
          mainImportGroupType: this.mainImportGroupType,
          mainImportImportStrategy: this.mainImportImportStrategy,
          mainImportReleaseName: this.mainImportReleaseName,
          mainImportNamespace: this.mainImportNamespace,
          mainImportMinIncludeBytes: this.mainImportMinIncludeBytes,
          mainImportIncludeStatus: this.mainImportIncludeStatus,
          mainImportIncludeCrds: this.mainImportIncludeCrds,
          mainImportKubeVersion: this.mainImportKubeVersion,
          mainImportValuesFilesText: this.mainImportValuesFilesText,
          mainImportSetText: this.mainImportSetText,
          mainImportExtraSetText: this.mainImportExtraSetText,
          mainImportApiVersionsText: this.mainImportApiVersionsText,
          mainImportConfigOpen: this.mainImportConfigOpen,
          mainImportPickedFilesLabel: this.mainImportPickedFilesLabel,
          mainImportChartValuesEditor: this.mainImportChartValuesEditor,
          mainImportLoadedChartValues: this.mainImportLoadedChartValues,
          mainImportUseChartValuesEditor: this.mainImportUseChartValuesEditor,
          mainImportSectionCollapsed: this.mainImportSectionCollapsed,
          mainImportOutChartDir: this.mainImportOutChartDir,
          mainImportOutChartName: this.mainImportOutChartName,
          mainImportLibraryChartPath: this.mainImportLibraryChartPath,
          mainImportSaveChartOpen: this.mainImportSaveChartOpen,
          fsPickerQuery: this.fsPickerQuery,
          fsPickerShowHidden: this.fsPickerShowHidden,
          fsPickerOnlySelectable: this.fsPickerOnlySelectable
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
    parseLines(v) {{
      return String(v || '')
        .split(/\r?\n/)
        .map(s => s.trim())
        .filter(Boolean);
    }},
    parseSetBlocks() {{
      const out = {{
        setStringValues: [],
        setFileValues: [],
        setJsonValues: [],
      }};
      for (const line of this.parseLines(this.mainImportExtraSetText)) {{
        if (line.startsWith('set-string:')) {{
          const v = line.slice('set-string:'.length).trim();
          if (v) out.setStringValues.push(v);
          continue;
        }}
        if (line.startsWith('set-file:')) {{
          const v = line.slice('set-file:'.length).trim();
          if (v) out.setFileValues.push(v);
          continue;
        }}
        if (line.startsWith('set-json:')) {{
          const v = line.slice('set-json:'.length).trim();
          if (v) out.setJsonValues.push(v);
          continue;
        }}
      }}
      return out;
    }},
    countIndent(line) {{
      const m = /^(\s*)/.exec(String(line || ''));
      return m ? m[1].length : 0;
    }},
    buildMainImportYamlPreview(src) {{
      const lines = String(src || '').split('\n');
      const meta = lines.map((line, idx) => ({{
        lineNo: idx + 1,
        indent: this.countIndent(line),
        raw: line,
        collapsible: false,
      }}));
      for (let i = 0; i < lines.length; i++) {{
        if (!lines[i].trim() || /^\s*#/.test(lines[i])) continue;
        const curIndent = meta[i].indent;
        for (let j = i + 1; j < lines.length; j++) {{
          if (!lines[j].trim()) continue;
          if (meta[j].indent > curIndent) meta[i].collapsible = true;
          break;
        }}
      }}

      const collapsed = new Set(
        Object.keys(this.mainImportSectionCollapsed || {{}})
          .filter((k) => !!this.mainImportSectionCollapsed[k])
          .map((k) => Number(k))
          .filter((n) => Number.isFinite(n) && n > 0)
      );
      const hidden = new Set();
      for (let i = 0; i < meta.length; i++) {{
        const m = meta[i];
        if (!collapsed.has(m.lineNo)) continue;
        for (let j = i + 1; j < meta.length; j++) {{
          if (meta[j].indent <= m.indent && meta[j].raw.trim() !== '') break;
          hidden.add(meta[j].lineNo);
        }}
      }}
      const html = lines.map((line, idx) => {{
        const m = meta[idx];
        const cls = ['yamlline'];
        if (hidden.has(m.lineNo)) cls.push('hidden');
        const mark = m.collapsible
          ? '<span class="foldmark" data-fold-line="' + String(m.lineNo) + '" title="Toggle fold">' + (collapsed.has(m.lineNo) ? '▸' : '▾') + '</span>'
          : '<span class="foldmark sp"> </span>';
        return '<span id="main-yaml-line-' + String(m.lineNo) + '" data-line="' + String(m.lineNo) + '" data-indent="' + String(m.indent) + '" class="' + cls.join(' ') + '" title="' + this.escapeAttr(line) + '">' + mark + this.highlightStructured(line) + '</span>';
      }}).join('');
      return {{ meta, html }};
    }},
    onMainImportFoldClick(event) {{
      if (this.mainImportGeneratedCm) return;
      const marker = event && event.target && event.target.closest
        ? event.target.closest('.foldmark[data-fold-line]')
        : null;
      if (!marker) return;
      event.preventDefault();
      event.stopPropagation();
      const lineNo = Number(marker.getAttribute('data-fold-line') || 0);
      if (!lineNo) return;
      const key = String(lineNo);
      const out = {{ ...this.mainImportSectionCollapsed }};
      if (out[key]) delete out[key];
      else out[key] = true;
      this.mainImportSectionCollapsed = out;
    }},
    setMainImportFoldLevel(level) {{
      const threshold = Math.max(0, Number(level || 1) - 1);
      const out = {{}};
      for (const m of (this.mainImportPreviewMeta || [])) {{
        if (!m.collapsible) continue;
        if (Math.floor((Number(m.indent) || 0) / 2) >= threshold) {{
          out[String(m.lineNo)] = true;
        }}
      }}
      this.mainImportSectionCollapsed = out;
    }},
    openMainImportConfig() {{
      this.mainImportConfigOpen = true;
    }},
    closeMainImportConfig() {{
      this.mainImportConfigOpen = false;
    }},
    foldMainImportLevel(level) {{
      if (this.mainImportGeneratedCm) {{
        this.mainImportGeneratedCm.foldLevel(level);
        return;
      }}
      this.setMainImportFoldLevel(level);
    }},
    expandAllMainImportSections() {{
      if (this.mainImportGeneratedCm) {{
        this.mainImportGeneratedCm.unfoldAll();
        return;
      }}
      this.mainImportSectionCollapsed = {{}};
    }},
    collapseAllMainImportSections() {{
      if (this.mainImportGeneratedCm) {{
        this.mainImportGeneratedCm.foldAll();
        return;
      }}
      const out = {{}};
      for (const m of (this.mainImportPreviewMeta || [])) {{
        if (m.collapsible) out[String(m.lineNo)] = true;
      }}
      this.mainImportSectionCollapsed = out;
    }},
    async loadChartValuesFromPath() {{
      this.mainImportError = '';
      if (this.mainImportSourceType !== 'chart') {{
        this.mainImportError = 'values.yaml loader is available only for sourceType=chart';
        return;
      }}
      if (!this.mainImportPath || !this.mainImportPath.trim()) {{
        this.mainImportError = 'Select chart path first';
        return;
      }}
      const ctrl = this.beginAbortableRequest('chart-values');
      try {{
        const res = await fetch('/api/chart-values', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{ path: this.mainImportPath }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('chart-values API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if (!res.ok) {{
          throw new Error(data.message || ('chart-values API HTTP ' + res.status));
        }}
        if (!data.ok) {{
          throw new Error(data.message || 'Failed to load chart values');
        }}
        this.mainImportLoadedChartValues = data.valuesYaml || '';
        this.mainImportChartValuesEditor = this.mainImportLoadedChartValues;
        this.mainImportUseChartValuesEditor = true;
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        this.mainImportError = String(e);
      }} finally {{
        this.finishAbortableRequest('chart-values', ctrl);
      }}
    }},
    resetChartValuesEditor() {{
      if (this.mainImportLoadedChartValues) {{
        this.mainImportChartValuesEditor = this.mainImportLoadedChartValues;
      }}
    }},
    pasteMainImportFromStdin() {{
      if (!this.mainImportStdinText) return;
      this.mainImportChartValuesEditor = String(this.mainImportStdinText);
      this.mainImportUseChartValuesEditor = true;
      this.mainImportMessage = 'Loaded values from stdin';
      this.mainImportError = '';
    }},
    async copyMainImportOutput() {{
      if (!this.mainImportOutput) return;
      try {{
        await navigator.clipboard.writeText(this.mainImportOutput);
        this.mainImportSaveChartMessage = 'Generated values copied to clipboard';
      }} catch(e) {{
        this.mainImportError = String(e);
      }}
    }},
    openMainImportSaveChart() {{
      this.mainImportSaveChartError = '';
      this.mainImportSaveChartMessage = '';
      if (!this.mainImportOutChartDir && this.mainImportPath) {{
        this.mainImportOutChartDir = String(this.mainImportPath).replace(/\/+$/, '') + '-imported';
      }}
      this.mainImportSaveChartOpen = true;
    }},
    closeMainImportSaveChart() {{
      this.mainImportSaveChartOpen = false;
    }},
    async openMainImportPicker() {{
      this.mainImportError = '';
      this.fsPickerError = '';
      this.fsPickerTarget = 'source-path';
      this.fsPickerOpen = true;
      const initial = this.mainImportPath && this.mainImportPath.trim()
        ? this.mainImportPath.trim()
        : '';
      await this.loadFsEntries(initial);
    }},
    async openMainImportOutChartPicker() {{
      this.mainImportSaveChartError = '';
      this.fsPickerError = '';
      this.fsPickerTarget = 'chart-output';
      this.fsPickerOpen = true;
      const initial = this.mainImportOutChartDir && this.mainImportOutChartDir.trim()
        ? this.mainImportOutChartDir.trim()
        : '';
      await this.loadFsEntries(initial);
    }},
    closeFsPicker() {{
      this.fsPickerOpen = false;
    }},
    isHiddenFile(e) {{
      return String((e && e.name) || '').startsWith('.');
    }},
    isFsEntrySelectable(e) {{
      if (!e) return false;
      if (this.fsPickerTarget === 'chart-output') {{
        return !!e.isDir;
      }}
      if (this.mainImportSourceType === 'compose') {{
        return !e.isDir && this.isComposeFile(e.name);
      }}
      return !!e.isDir;
    }},
    onFsRowActivate(e) {{
      if (!e) return;
      if (e.isDir) {{
        this.loadFsEntries(e.path);
        return;
      }}
      if (this.isFsEntrySelectable(e)) {{
        this.selectFsPath(e.path);
      }}
    }},
    async loadFsEntries(path) {{
      this.fsPickerError = '';
      const ctrl = this.beginAbortableRequest('fs-list');
      try {{
        const res = await fetch('/api/fs-list', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{ path: path || '' }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('fs-list API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.message || ('fs-list API HTTP ' + res.status));
        }}
        if(!data.ok) {{
          throw new Error(data.message || 'Failed to list server directory');
        }}
        this.fsPickerCurrent = data.path || '';
        this.fsPickerPath = data.path || '';
        this.fsPickerParent = data.parent || '';
        this.fsPickerEntries = Array.isArray(data.entries) ? data.entries : [];
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        this.fsPickerError = String(e);
      }} finally {{
        this.finishAbortableRequest('fs-list', ctrl);
      }}
    }},
    goFsParent() {{
      if(!this.fsPickerParent) return;
      this.loadFsEntries(this.fsPickerParent);
    }},
    isComposeFile(name) {{
      const s = String(name || '').toLowerCase();
      return s.endsWith('.yml') || s.endsWith('.yaml');
    }},
    selectFsPath(path) {{
      if (this.fsPickerTarget === 'chart-output') {{
        this.mainImportOutChartDir = path || '';
      }} else {{
        this.mainImportPath = path || '';
        this.mainImportUploadedFiles = [];
        this.mainImportPickedFilesLabel = this.mainImportPath ? ('Selected: ' + this.mainImportPath) : '';
      }}
      this.closeFsPicker();
      if (this.fsPickerTarget !== 'chart-output' && this.mainImportSourceType === 'chart') {{
        this.$nextTick(() => this.loadChartValuesFromPath());
      }}
    }},
    clearMainImportSelection() {{
      this.mainImportPath = '';
      this.mainImportPickedFilesLabel = '';
      this.mainImportUploadedFiles = [];
    }},
    async runMainImport() {{
      this.mainImportError = '';
      this.mainImportMessage = '';
      this.mainImportCompareError = '';
      this.mainImportCompareMessage = '';
      this.mainImportCompareSummary = '';
      this.mainImportCompareEqual = false;
      this.mainImportCompareSourceCount = 0;
      this.mainImportCompareGeneratedCount = 0;
      this.mainImportRunning = true;
      const ctrl = this.beginAbortableRequest('import');
      try {{
        const extra = this.parseSetBlocks();
        const res = await fetch('/api/import', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            sourceType: this.mainImportSourceType,
            path: this.mainImportPath,
            env: this.mainImportEnv,
            groupName: this.mainImportGroupName,
            groupType: this.mainImportGroupType,
            importStrategy: this.mainImportImportStrategy,
            releaseName: this.mainImportReleaseName,
            namespace: this.mainImportNamespace,
            minIncludeBytes: Number(this.mainImportMinIncludeBytes || 24),
            includeStatus: !!this.mainImportIncludeStatus,
            includeCrds: !!this.mainImportIncludeCrds,
            kubeVersion: this.mainImportKubeVersion,
            valuesFiles: this.parseLines(this.mainImportValuesFilesText),
            setValues: this.parseLines(this.mainImportSetText),
            setStringValues: extra.setStringValues,
            setFileValues: extra.setFileValues,
            setJsonValues: extra.setJsonValues,
            apiVersions: this.parseLines(this.mainImportApiVersionsText),
            chartValuesYaml: this.mainImportUseChartValuesEditor ? (this.mainImportChartValuesEditor || '') : undefined,
          }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('import API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.message || ('import API HTTP ' + res.status));
        }}
        if(!data.ok) {{
          this.mainImportOutput = '';
          this.mainImportSourceCount = Number(data.sourceCount || 0);
          this.mainImportError = data.message || 'Import failed';
          return;
        }}
        this.mainImportOutput = data.valuesYaml || '';
        this.mainImportSourceCount = Number(data.sourceCount || 0);
        this.mainImportMessage = data.message || 'Import completed';
        this.mainImportConfigOpen = false;
        this.finishAbortableRequest('import', ctrl);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        this.mainImportOutput = '';
        this.mainImportSourceCount = 0;
        this.mainImportError = String(e);
      }} finally {{
        this.finishAbortableRequest('import', ctrl);
        this.mainImportRunning = false;
      }}
    }},
    async runMainImportCompare() {{
      this.mainImportCompareError = '';
      this.mainImportCompareMessage = '';
      this.mainImportCompareSummary = '';
      this.mainImportCompareEqual = false;
      this.mainImportCompareSourceCount = 0;
      this.mainImportCompareGeneratedCount = 0;
      if (this.mainImportSourceType !== 'chart') {{
        this.mainImportCompareError = 'render compare is available only for sourceType=chart';
        return;
      }}
      if (!this.mainImportOutput || !String(this.mainImportOutput).trim()) {{
        this.mainImportCompareError = 'Generated values are empty, run import first';
        return;
      }}
      this.mainImportCompareRunning = true;
      const ctrl = this.beginAbortableRequest('compare-renders');
      try {{
        const extra = this.parseSetBlocks();
        const res = await fetch('/api/compare-renders', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            sourceType: this.mainImportSourceType,
            path: this.mainImportPath,
            env: this.mainImportEnv,
            groupName: this.mainImportGroupName,
            groupType: this.mainImportGroupType,
            importStrategy: this.mainImportImportStrategy,
            releaseName: this.mainImportReleaseName,
            namespace: this.mainImportNamespace,
            minIncludeBytes: Number(this.mainImportMinIncludeBytes || 24),
            includeStatus: !!this.mainImportIncludeStatus,
            includeCrds: !!this.mainImportIncludeCrds,
            kubeVersion: this.mainImportKubeVersion,
            valuesFiles: this.parseLines(this.mainImportValuesFilesText),
            setValues: this.parseLines(this.mainImportSetText),
            setStringValues: extra.setStringValues,
            setFileValues: extra.setFileValues,
            setJsonValues: extra.setJsonValues,
            apiVersions: this.parseLines(this.mainImportApiVersionsText),
            chartValuesYaml: this.mainImportUseChartValuesEditor ? (this.mainImportChartValuesEditor || '') : undefined,
            valuesYaml: this.mainImportOutput,
            libraryChartPath: this.mainImportLibraryChartPath || undefined,
          }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('compare-renders API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.message || ('compare-renders API HTTP ' + res.status));
        }}
        if(!data.ok) {{
          this.mainImportCompareError = data.message || 'Render compare failed';
          return;
        }}
        this.mainImportCompareEqual = !!data.equal;
        this.mainImportCompareSummary = data.summary || '';
        this.mainImportCompareSourceCount = Number(data.sourceCount || 0);
        this.mainImportCompareGeneratedCount = Number(data.generatedCount || 0);
        this.mainImportCompareMessage = data.message || 'Render compare completed';
        this.finishAbortableRequest('compare-renders', ctrl);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        this.mainImportCompareError = String(e);
      }} finally {{
        this.finishAbortableRequest('compare-renders', ctrl);
        this.mainImportCompareRunning = false;
      }}
    }},
    clearMainImport() {{
      this.mainImportOutput = '';
      this.mainImportError = '';
      this.mainImportMessage = '';
      this.mainImportSourceCount = 0;
      this.mainImportCompareError = '';
      this.mainImportCompareMessage = '';
      this.mainImportCompareSummary = '';
      this.mainImportCompareEqual = false;
      this.mainImportCompareSourceCount = 0;
      this.mainImportCompareGeneratedCount = 0;
    }},
    async saveMainImportAsChart() {{
      this.mainImportSaveChartError = '';
      this.mainImportSaveChartMessage = '';
      if (!this.mainImportOutput || !String(this.mainImportOutput).trim()) {{
        this.mainImportSaveChartError = 'Generated values are empty, run import first';
        return;
      }}
      if (!this.mainImportOutChartDir || !String(this.mainImportOutChartDir).trim()) {{
        this.mainImportSaveChartError = 'Output chart directory is required';
        return;
      }}
      this.mainImportSaveChartRunning = true;
      const ctrl = this.beginAbortableRequest('save-chart');
      try {{
        const res = await fetch('/api/save-chart', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            sourceType: this.mainImportSourceType,
            sourcePath: this.mainImportPath,
            outChartDir: this.mainImportOutChartDir,
            chartName: this.mainImportOutChartName || undefined,
            libraryChartPath: this.mainImportLibraryChartPath || undefined,
            valuesYaml: this.mainImportOutput,
          }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('save-chart API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.message || ('save-chart API HTTP ' + res.status));
        }}
        if(!data.ok) {{
          this.mainImportSaveChartError = data.message || 'Save chart failed';
          return;
        }}
        this.mainImportSaveChartMessage = data.message || 'Chart saved';
        this.mainImportSaveChartOpen = false;
        this.finishAbortableRequest('save-chart', ctrl);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        this.mainImportSaveChartError = String(e);
      }} finally {{
        this.finishAbortableRequest('save-chart', ctrl);
        this.mainImportSaveChartRunning = false;
      }}
    }},
    loadSampleMainImport() {{
      this.mainImportSourceType = 'chart';
      this.mainImportPath = './tmp/chart-samples/nginx';
      this.mainImportEnv = 'dev';
      this.mainImportGroupName = 'apps-k8s-manifests';
      this.mainImportGroupType = 'apps-k8s-manifests';
      this.mainImportImportStrategy = 'helpers';
      this.mainImportReleaseName = 'inspect';
      this.mainImportNamespace = 'default';
      this.mainImportMinIncludeBytes = 24;
      this.mainImportIncludeStatus = false;
      this.mainImportIncludeCrds = false;
      this.mainImportKubeVersion = '';
      this.mainImportValuesFilesText = '';
      this.mainImportSetText = '';
      this.mainImportExtraSetText = '';
      this.mainImportApiVersionsText = '';
    }},
    resetMainImportConfig() {{
      this.mainImportSourceType = 'chart';
      this.mainImportPath = '';
      this.mainImportPickedFilesLabel = '';
      this.mainImportEnv = 'dev';
      this.mainImportGroupName = 'apps-k8s-manifests';
      this.mainImportGroupType = 'apps-k8s-manifests';
      this.mainImportImportStrategy = 'helpers';
      this.mainImportReleaseName = 'imported';
      this.mainImportNamespace = '';
      this.mainImportMinIncludeBytes = 24;
      this.mainImportIncludeStatus = false;
      this.mainImportIncludeCrds = false;
      this.mainImportKubeVersion = '';
      this.mainImportValuesFilesText = '';
      this.mainImportSetText = '';
      this.mainImportExtraSetText = '';
      this.mainImportApiVersionsText = '';
      this.mainImportLibraryChartPath = '';
    }},
    decodeBase64Url(s) {{
      const text = String(s || '').replace(/-/g, '+').replace(/_/g, '/');
      const padded = text + '='.repeat((4 - (text.length % 4 || 4)) % 4);
      const bin = atob(padded);
      const bytes = Uint8Array.from(bin, ch => ch.charCodeAt(0));
      return new TextDecoder().decode(bytes);
    }},
    encodeUtf8Base64(s) {{
      const bytes = new TextEncoder().encode(String(s || ''));
      let bin = '';
      for (let i = 0; i < bytes.length; i += 1) bin += String.fromCharCode(bytes[i]);
      return btoa(bin);
    }},
    decodeUtf8Base64(s) {{
      const bin = atob(String(s || '').trim());
      const bytes = Uint8Array.from(bin, ch => ch.charCodeAt(0));
      return new TextDecoder().decode(bytes);
    }},
    formatJwtTimestamp(v) {{
      const n = Number(v);
      if (!Number.isFinite(n)) return null;
      const ms = n > 1e12 ? n : (n * 1000);
      const d = new Date(ms);
      if (Number.isNaN(d.getTime())) return null;
      return d.toISOString();
    }},
    inspectJwt(token) {{
      const raw = String(token || '').trim();
      const parts = raw.split('.');
      if (parts.length < 2) throw new Error('JWT must contain at least header.payload');
      const headerText = this.decodeBase64Url(parts[0] || '');
      const payloadText = this.decodeBase64Url(parts[1] || '');
      let headerObj = null;
      let payloadObj = null;
      try {{ headerObj = JSON.parse(headerText); }} catch(_) {{}}
      try {{ payloadObj = JSON.parse(payloadText); }} catch(_) {{}}
      const nowSec = Math.floor(Date.now() / 1000);
      const exp = payloadObj && Number(payloadObj.exp);
      const nbf = payloadObj && Number(payloadObj.nbf);
      const iat = payloadObj && Number(payloadObj.iat);
      const status = [];
      if (Number.isFinite(nbf) && nowSec < nbf) status.push('not active yet');
      if (Number.isFinite(exp) && nowSec >= exp) status.push('expired');
      if (!status.length) status.push('valid by time claims');
      const out = {{
        jwt: {{
          algorithm: headerObj && headerObj.alg ? headerObj.alg : null,
          typ: headerObj && headerObj.typ ? headerObj.typ : null,
          signaturePresent: parts.length > 2 && String(parts[2] || '').length > 0,
        }},
        timing: {{
          nowUnix: nowSec,
          nowISO: new Date(nowSec * 1000).toISOString(),
          status: status.join(', '),
          exp: Number.isFinite(exp) ? {{ unix: exp, iso: this.formatJwtTimestamp(exp) }} : null,
          nbf: Number.isFinite(nbf) ? {{ unix: nbf, iso: this.formatJwtTimestamp(nbf) }} : null,
          iat: Number.isFinite(iat) ? {{ unix: iat, iso: this.formatJwtTimestamp(iat) }} : null,
        }},
        header: headerObj || headerText,
        payload: payloadObj || payloadText,
      }};
      return JSON.stringify(out, null, 2);
    }},
    toHexPair(n, upper) {{
      const v = Number(n) & 0xff;
      const t = v.toString(16).padStart(2, '0');
      return upper ? t.toUpperCase() : t.toLowerCase();
    }},
    utf8TokensWithRanges(chunkBytes, baseOffset) {{
      const bytes = Array.isArray(chunkBytes) ? chunkBytes : [];
      const out = [];
      let i = 0;
      while (i < bytes.length) {{
        const b0 = Number(bytes[i]) & 0xff;
        let len = 1;
        if ((b0 & 0b1110_0000) === 0b1100_0000) len = 2;
        else if ((b0 & 0b1111_0000) === 0b1110_0000) len = 3;
        else if ((b0 & 0b1111_1000) === 0b1111_0000) len = 4;
        if (i + len > bytes.length) len = 1;
        let valid = len === 1 ? (b0 < 0x80) : true;
        if (len > 1) {{
          for (let j = 1; j < len; j += 1) {{
            const bj = Number(bytes[i + j]) & 0xff;
            if ((bj & 0b1100_0000) !== 0b1000_0000) {{
              valid = false;
              len = 1;
              break;
            }}
          }}
        }}
        const part = bytes.slice(i, i + len);
        let text = '.';
        if (valid) {{
          try {{
            text = new TextDecoder().decode(Uint8Array.from(part));
            if (!text || Array.from(text).some((ch) => {{
              const cp = ch.codePointAt(0) || 0;
              return cp < 32 || cp === 127;
            }})) text = '.';
          }} catch(_) {{
            text = '.';
          }}
        }}
        out.push({{
          start: baseOffset + i,
          end: baseOffset + i + len - 1,
          text,
        }});
        i += len;
      }}
      return out;
    }},
    bytesFromText(value) {{
      return Array.from(new TextEncoder().encode(String(value || '')));
    }},
    clearHexSelection() {{
      this.converterHexSelStart = null;
      this.converterHexSelEnd = null;
      this.converterHexSelecting = false;
    }},
    normalizeHexSelRange() {{
      if (this.converterHexSelStart === null || this.converterHexSelEnd === null) return null;
      const a = Number(this.converterHexSelStart);
      const b = Number(this.converterHexSelEnd);
      if (!Number.isFinite(a) || !Number.isFinite(b)) return null;
      return {{ from: Math.min(a, b), to: Math.max(a, b) }};
    }},
    isHexByteSelected(idx) {{
      if (idx === null || idx === undefined) return false;
      const r = this.normalizeHexSelRange();
      if (!r) return false;
      const n = Number(idx);
      return Number.isFinite(n) && n >= r.from && n <= r.to;
    }},
    isHexRangeSelected(start, end) {{
      const r = this.normalizeHexSelRange();
      if (!r) return false;
      const a = Number(start);
      const b = Number(end);
      if (!Number.isFinite(a) || !Number.isFinite(b)) return false;
      return !(b < r.from || a > r.to);
    }},
    clearNativeSelection() {{
      try {{
        const sel = window.getSelection ? window.getSelection() : null;
        if (sel && sel.removeAllRanges) sel.removeAllRanges();
      }} catch(_) {{}}
    }},
    onHexByteDown(idx) {{
      if (idx === null || idx === undefined) return;
      const n = Number(idx);
      if (!Number.isFinite(n)) return;
      this.clearNativeSelection();
      this.converterHexSelStart = n;
      this.converterHexSelEnd = n;
      this.converterHexSelecting = true;
    }},
    onHexByteEnter(idx) {{
      if (idx === null || idx === undefined) return;
      if (!this.converterHexSelecting) return;
      const n = Number(idx);
      if (!Number.isFinite(n)) return;
      this.clearNativeSelection();
      this.converterHexSelEnd = n;
    }},
    onHexDumpPointerUp() {{
      this.clearNativeSelection();
      this.converterHexSelecting = false;
    }},
    parseHexBytesFromInput(raw, formatMode) {{
      const src = String(raw || '');
      const parsePlain = (s) => {{
        const clean = s.replace(/[\s:_-]/g, '');
        if (!clean) return [];
        if (!/^[0-9a-fA-F]+$/.test(clean) || clean.length % 2 !== 0) {{
          throw new Error('Expected even-length plain HEX');
        }}
        const out = [];
        for (let i = 0; i < clean.length; i += 2) out.push(parseInt(clean.slice(i, i + 2), 16));
        return out;
      }};
      const parseOx = (s) => {{
        const arr = [];
        const re = /0x([0-9a-fA-F]{{2}})/g;
        let m = null;
        while ((m = re.exec(s)) !== null) arr.push(parseInt(m[1], 16));
        if (!arr.length) throw new Error('Expected 0xNN bytes');
        return arr;
      }};
      const parseEsc = (s) => {{
        const arr = [];
        const re = /\\x([0-9a-fA-F]{{2}})/g;
        let m = null;
        while ((m = re.exec(s)) !== null) arr.push(parseInt(m[1], 16));
        if (!arr.length) throw new Error('Expected \\\\xNN bytes');
        return arr;
      }};
      const parseByteArray = (s) => {{
        const nums = (s.match(/-?\d+/g) || []).map(x => Number(x)).filter(n => Number.isFinite(n));
        if (!nums.length) throw new Error('Expected byte array values');
        for (const n of nums) {{
          if (n < 0 || n > 255) throw new Error('Byte array values must be 0..255');
        }}
        return nums;
      }};
      const parseDump = (s) => {{
        const lines = s.split(/\r?\n/);
        const out = [];
        const pair = /\b([0-9a-fA-F]{{2}})\b/g;
        for (const line of lines) {{
          const rhs = line.includes('|') ? line.split('|')[0] : line;
          let m = null;
          while ((m = pair.exec(rhs)) !== null) {{
            out.push(parseInt(m[1], 16));
          }}
        }}
        if (!out.length) throw new Error('Expected hex dump bytes');
        return out;
      }};
      const mode = String(formatMode || 'auto');
      if (mode === 'plain') return parsePlain(src);
      if (mode === '0x' || mode === 'c-array') return parseOx(src);
      if (mode === 'escaped') return parseEsc(src);
      if (mode === 'byte-array') return parseByteArray(src);
      if (mode === 'dump') return parseDump(src);
      const tries = [
        () => parseOx(src),
        () => parseEsc(src),
        () => parseByteArray(src),
        () => parseDump(src),
        () => parsePlain(src),
      ];
      for (const t of tries) {{
        try {{ return t(); }} catch(_) {{}}
      }}
      throw new Error('Could not detect HEX format (expected plain, 0xNN, \\\\xNN, byte array or dump)');
    }},
    formatHexDump(bytes, upper, perLine) {{
      const src = Array.isArray(bytes) ? bytes : [];
      const lineSize = Math.max(4, Math.min(64, Number(perLine || 16)));
      const out = [];
      const toUtf8Preview = (chunk) => {{
        try {{
          const txt = new TextDecoder().decode(Uint8Array.from(chunk));
          return Array.from(txt).map((ch) => {{
            const cp = ch.codePointAt(0) || 0;
            if (cp >= 32 && cp !== 127) return ch;
            return '.';
          }}).join('');
        }} catch(_) {{
          return '';
        }}
      }};
      for (let i = 0; i < src.length; i += lineSize) {{
        const chunk = src.slice(i, i + lineSize);
        const hex = chunk.map(b => this.toHexPair(b, upper)).join(' ');
        const ascii = chunk.map((b) => (b >= 32 && b <= 126 ? String.fromCharCode(b) : '.')).join('');
        const utf8 = toUtf8Preview(chunk);
        const off = i.toString(16).padStart(8, '0');
        out.push(off + '  ' + hex.padEnd(lineSize * 3 - 1, ' ') + '  |' + ascii + '|  utf8:' + utf8);
      }}
      return out.join('\n');
    }},
    formatHexBytes(bytes, formatMode, upper, separator, perLine) {{
      const sep = String(separator || '');
      const pairs = bytes.map(b => this.toHexPair(b, upper));
      if (formatMode === 'plain') return pairs.join(sep);
      if (formatMode === '0x') return pairs.map(x => '0x' + x).join(sep || ' ');
      if (formatMode === 'escaped') return pairs.map(x => '\\x' + x).join(sep || '');
      if (formatMode === 'byte-array') return '[' + bytes.join(', ') + ']';
      if (formatMode === 'c-array') return '{{ ' + pairs.map(x => '0x' + x).join(', ') + ' }}';
      if (formatMode === 'dump') return this.formatHexDump(bytes, upper, perLine);
      const all = [];
      all.push('plain:\n' + this.formatHexBytes(bytes, 'plain', upper, sep || '', perLine));
      all.push('0x:\n' + this.formatHexBytes(bytes, '0x', upper, sep || ' ', perLine));
      all.push('escaped:\n' + this.formatHexBytes(bytes, 'escaped', upper, sep || '', perLine));
      all.push('byte-array:\n' + this.formatHexBytes(bytes, 'byte-array', upper, sep, perLine));
      all.push('c-array:\n' + this.formatHexBytes(bytes, 'c-array', upper, sep, perLine));
      all.push('dump:\n' + this.formatHexBytes(bytes, 'dump', upper, sep, perLine));
      return all.join('\n\n');
    }},
    runConvertLocal(mode, payload) {{
      const value = String(payload || '');
      switch (mode) {{
        case 'base64-encode':
          return this.encodeUtf8Base64(value);
        case 'base64-decode':
          return this.decodeUtf8Base64(value);
        case 'url-encode':
          return encodeURIComponent(value);
        case 'url-decode':
          return decodeURIComponent(value);
        case 'jwt-inspect':
          return this.inspectJwt(value);
        case 'unix-to-iso': {{
          const n = Number(value.trim());
          if (!Number.isFinite(n)) throw new Error('Expected unix timestamp');
          const ms = n > 1e12 ? n : (n * 1000);
          const d = new Date(ms);
          if (Number.isNaN(d.getTime())) throw new Error('Invalid timestamp');
          return d.toISOString();
        }}
        case 'iso-to-unix': {{
          const d = new Date(value.trim());
          if (Number.isNaN(d.getTime())) throw new Error('Expected ISO8601 datetime');
          return String(Math.floor(d.getTime() / 1000));
        }}
        case 'text-to-hex': {{
          const bytes = this.bytesFromText(value);
          this.converterHexLastBytes = bytes.slice();
          return this.formatHexBytes(
            bytes,
            this.converterHexView || 'dump',
            !!this.converterHexUppercase,
            this.converterHexSeparator || '',
            this.converterHexBytesPerLine || 16
          );
        }}
        case 'hex-to-text': {{
          const bytes = this.parseHexBytesFromInput(value, this.converterHexInFormat || 'auto');
          this.converterHexLastBytes = bytes.slice();
          return new TextDecoder().decode(Uint8Array.from(bytes));
        }}
        default:
          throw new Error('Unsupported converter mode: ' + mode);
      }}
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
      const ctrl = this.beginAbortableRequest('convert');
      try {{
        if (this.converterMode === 'yaml-to-json' || this.converterMode === 'json-to-yaml') {{
          const res = await fetch('/api/convert', {{
            method: 'POST',
            headers: {{ 'content-type': 'application/json' }},
            body: JSON.stringify({{
              mode: this.converterMode,
              docMode: this.converterDocMode,
              docIndex: this.converterDocMode === 'index' ? Number(this.converterDocIndex) : undefined,
              input: payload
            }}),
            signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
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
        }} else {{
          const localOut = this.runConvertLocal(this.converterMode, payload);
          if(reqId !== this.converterRequestSeq) return;
          this.converterOutput = String(localOut || '');
        }}
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        if(reqId !== this.converterRequestSeq) return;
        this.converterError = String(e);
        this.converterOutput = '';
      }} finally {{
        this.finishAbortableRequest('convert', ctrl);
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
      const pairs = {{
        'yaml-to-json': 'json-to-yaml',
        'json-to-yaml': 'yaml-to-json',
        'base64-encode': 'base64-decode',
        'base64-decode': 'base64-encode',
        'url-encode': 'url-decode',
        'url-decode': 'url-encode',
        'unix-to-iso': 'iso-to-unix',
        'iso-to-unix': 'unix-to-iso',
        'text-to-hex': 'hex-to-text',
        'hex-to-text': 'text-to-hex',
      }};
      this.converterMode = pairs[this.converterMode] || this.converterMode;
    }},
    clearConverter() {{
      this.converterInput = '';
      this.converterOutput = '';
      this.converterError = '';
      this.converterHexLastBytes = [];
      this.clearHexSelection();
    }},
    loadSampleConverter() {{
      const m = this.converterMode;
      if (m === 'yaml-to-json' || m === 'json-to-yaml') {{
        this.converterMode = 'yaml-to-json';
        this.converterDocMode = 'all';
        this.converterInput = "global:\n  env: dev\napps-stateless:\n  app-1:\n    enabled: true\n";
      }} else if (m === 'base64-encode' || m === 'base64-decode') {{
        this.converterInput = m === 'base64-encode' ? 'hello happ' : 'aGVsbG8gaGFwcA==';
      }} else if (m === 'url-encode' || m === 'url-decode') {{
        this.converterInput = m === 'url-encode' ? 'name=alex v&scope=dev ops' : 'name%3Dalex%20v%26scope%3Ddev%20ops';
      }} else if (m === 'jwt-inspect') {{
        this.converterInput = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMiLCJuYW1lIjoiZGVtbyIsImlhdCI6MTUxNjIzOTAyMiwiZXhwIjo0MTAyNDQ0ODAwfQ.signature';
      }} else if (m === 'unix-to-iso' || m === 'iso-to-unix') {{
        this.converterInput = m === 'unix-to-iso' ? '1700000000' : '2026-03-02T12:00:00Z';
      }} else if (m === 'text-to-hex' || m === 'hex-to-text') {{
        this.converterInput = m === 'text-to-hex' ? 'happ' : '68617070';
      }} else {{
        this.converterInput = '';
      }}
    }},
    async copyConverterOutput() {{
      if(!this.converterOutput) return;
      try {{ await navigator.clipboard.writeText(this.converterOutput); }} catch(_) {{}}
    }},
    isJqSimpleIdent(s) {{
      return /^[A-Za-z_][A-Za-z0-9_]*$/.test(String(s || ''));
    }},
    jqFieldSnippet(key) {{
      const k = String(key || '');
      if (!k) return '.';
      if (this.isJqSimpleIdent(k)) return '.' + k;
      return '.[' + JSON.stringify(k) + ']';
    }},
    onJqInput() {{
      this.updateJqSuggestState(false);
    }},
    applyJqPreset(query) {{
      this.jqQuery = query || '.';
      this.$nextTick(() => {{
        const area = this.$refs.jqQueryInput;
        if(!area) return;
        const p = (this.jqQuery || '').length;
        area.focus();
        area.setSelectionRange(p, p);
        this.syncJqScroll();
      }});
    }},
    currentJqTokenMeta() {{
      const ta = this.$refs.jqQueryInput;
      const src = this.jqQuery || '';
      const pos = ta && Number.isFinite(ta.selectionStart) ? ta.selectionStart : src.length;
      const left = src.slice(0, pos);
      const bracketField = left.match(/(\.\[\s*"([^"]*))$/);
      if (bracketField) {{
        return {{
          kind: 'field',
          raw: bracketField[1],
          term: bracketField[2],
          start: pos - bracketField[1].length,
          end: pos,
        }};
      }}
      const field = left.match(/(\.[A-Za-z0-9_.-]*)$/);
      if (field) {{
        return {{
          kind: 'field',
          raw: field[1],
          term: field[1].slice(1),
          start: pos - field[1].length,
          end: pos,
        }};
      }}
      const fn = left.match(/([A-Za-z_][A-Za-z0-9_]*)$/);
      if (fn) {{
        return {{
          kind: 'func',
          raw: fn[1],
          term: fn[1],
          start: pos - fn[1].length,
          end: pos,
        }};
      }}
      return {{ kind: 'none', raw: '', term: '', start: pos, end: pos }};
    }},
    updateJqSuggestState(forceOpen) {{
      const meta = this.currentJqTokenMeta();
      const canOpen = this.activeUtilityKey === 'jq-playground' &&
        (meta.kind === 'field' || meta.kind === 'func') &&
        String(meta.raw || '').length > 0;
      this.jqSuggestOpen = !!forceOpen ? this.activeUtilityKey === 'jq-playground' : canOpen;
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
      const meta = this.currentJqTokenMeta();
      const start = Math.max(0, meta.start || pos);
      const end = Math.max(start, meta.end || pos);
      this.jqQuery = src.slice(0, start) + text + src.slice(end);
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
    extractInputKeys(src) {{
      const map = Object.create(null);
      const add = (k) => {{
        if(!k) return;
        const s = String(k).trim();
        if(!s) return;
        map[s] = true;
      }};
      const yamlLines = src.split(/\r?\n/);
      for (const line of yamlLines) {{
        const m = line.match(/^\s*([A-Za-z0-9_.-]+)\s*:/);
        if (m) add(m[1]);
      }}
      const jsonKey = /"([^"\\]+)"\s*:/g;
      let m = null;
      while ((m = jsonKey.exec(src)) !== null) {{
        add(m[1]);
      }}
      return Object.keys(map).sort((a,b) => a.localeCompare(b)).slice(0, 300);
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
          this.updateJqSuggestState(true);
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
      if(e.key === 'Tab' || (e.key === 'Enter' && (e.ctrlKey || e.metaKey))) {{
        e.preventDefault();
        this.pickJqSuggestion(this.jqSuggestIndex);
        return;
      }}
      if(e.key === 'Enter') {{
        this.jqSuggestOpen = false;
        return;
      }}
      if(e.key === 'Escape') {{
        e.preventDefault();
        this.jqSuggestOpen = false;
      }}
    }},
    syncMainImportSourceScroll() {{
      if (this.mainImportSourceCm) return;
      const ta = this.$refs.mainImportSourceInput;
      const pre = this.$el && this.$el.querySelector('.yaml-editor-highlight');
      if(!ta || !pre) return;
      pre.scrollTop = ta.scrollTop;
      pre.scrollLeft = ta.scrollLeft;
    }},
    syncMainImportEditorHeights() {{
      if (this.activeUtilityKey !== 'main-import') return;
      this.$nextTick(() => {{
        const sourceShell = this.$el && this.$el.querySelector('.import-layout .import-section .import-editor-shell');
        const outputShell = this.$el && this.$el.querySelector('.import-layout .import-output .import-editor-shell');
        if (!sourceShell || !outputShell) return;
        const sourceMin = Number(sourceShell.style.minHeight.replace('px', '') || 0);
        const outputMin = Number(outputShell.style.minHeight.replace('px', '') || 0);
        const base = Math.max(
          sourceShell.clientHeight || 0,
          outputShell.clientHeight || 0,
          sourceMin,
          outputMin,
          420
        );
        sourceShell.style.minHeight = base + 'px';
        outputShell.style.minHeight = base + 'px';
      }});
    }},
    syncJqScroll() {{
      const ta = this.$refs.jqQueryInput;
      const pre = this.$el && this.$el.querySelector('.jq-query-highlight');
      if(!ta || !pre) return;
      pre.scrollTop = ta.scrollTop;
      pre.scrollLeft = ta.scrollLeft;
    }},
    makeSelectionInfo(src, sel) {{
      const text = String(src || '');
      const fromRaw = Number(sel && sel.from);
      const toRaw = Number(sel && sel.to);
      const from = Number.isFinite(fromRaw) ? Math.max(0, Math.min(text.length, fromRaw)) : 0;
      const to = Number.isFinite(toRaw) ? Math.max(0, Math.min(text.length, toRaw)) : from;
      const a = Math.min(from, to);
      const b = Math.max(from, to);
      const selected = text.slice(a, b);
      return {{ from: a, to: b, text: selected }};
    }},
    tokenAtOffset(src, offset) {{
      const text = String(src || '');
      const pos = Math.max(0, Math.min(text.length, Number(offset) || 0));
      let l = pos;
      let r = pos;
      const isToken = (ch) => /[0-9A-Za-z_.:@/-]/.test(ch);
      while (l > 0 && isToken(text.charAt(l - 1))) l -= 1;
      while (r < text.length && isToken(text.charAt(r))) r += 1;
      return text.slice(l, r).trim();
    }},
    normalizeSemanticNeedle(raw) {{
      const s = String(raw || '').trim();
      if (!s) return null;
      if (/^(true|false)$/i.test(s)) return {{ kind: 'bool', value: s.toLowerCase() }};
      if (/^null$/i.test(s)) return {{ kind: 'null', value: 'null' }};
      if (/^-?\d+(?:\.\d+)?$/.test(s)) return {{ kind: 'num', value: String(Number(s)) }};
      if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {{
        return {{ kind: 'str', value: s.slice(1, -1) }};
      }}
      return {{ kind: 'str', value: s }};
    }},
    extractYamlPathAt(text, offset) {{
      const src = String(text || '');
      const off = Math.max(0, Math.min(src.length, Number(offset) || 0));
      const lines = src.split('\n');
      let acc = 0;
      let targetLine = 0;
      for (let i = 0; i < lines.length; i += 1) {{
        const len = lines[i].length + 1;
        if (off < acc + len) {{
          targetLine = i;
          break;
        }}
        acc += len;
      }}
      const stack = [];
      for (let i = 0; i <= targetLine; i += 1) {{
        const line = lines[i];
        if (!line || /^\s*#/.test(line) || /^\s*$/.test(line)) continue;
        const m = /^(\s*)(["']?[A-Za-z0-9_.-]+["']?)\s*:/.exec(line);
        if (!m) continue;
        const indent = m[1].length;
        let key = m[2] || '';
        if ((key.startsWith('"') && key.endsWith('"')) || (key.startsWith("'") && key.endsWith("'"))) {{
          key = key.slice(1, -1);
        }}
        while (stack.length && stack[stack.length - 1].indent >= indent) stack.pop();
        stack.push({{ indent, key }});
      }}
      return stack.map((x) => x.key).filter(Boolean);
    }},
    findYamlRangesByPath(text, path) {{
      const src = String(text || '');
      const keys = Array.isArray(path) ? path.filter(Boolean) : [];
      if (!keys.length) return [];
      const lines = src.split('\n');
      const starts = [];
      let acc = 0;
      for (let i = 0; i < lines.length; i += 1) {{
        starts.push(acc);
        acc += lines[i].length + 1;
      }}
      const stack = [];
      const ranges = [];
      for (let i = 0; i < lines.length; i += 1) {{
        const line = lines[i];
        if (!line || /^\s*#/.test(line) || /^\s*$/.test(line)) continue;
        const m = /^(\s*)(["']?[A-Za-z0-9_.-]+["']?)\s*:/.exec(line);
        if (!m) continue;
        const indent = m[1].length;
        let key = m[2] || '';
        if ((key.startsWith('"') && key.endsWith('"')) || (key.startsWith("'") && key.endsWith("'"))) {{
          key = key.slice(1, -1);
        }}
        while (stack.length && stack[stack.length - 1].indent >= indent) stack.pop();
        const nextPath = stack.map((x) => x.key).concat([key]);
        if (nextPath.length === keys.length && nextPath.every((v, idx) => v === keys[idx])) {{
          let j = i + 1;
          while (j < lines.length) {{
            const ln = lines[j];
            if (!ln || /^\s*$/.test(ln)) {{
              j += 1;
              continue;
            }}
            const ind = (/^(\s*)/.exec(ln) || ['', ''])[1].length;
            if (ind <= indent) break;
            j += 1;
          }}
          const from = starts[i];
          const to = j < lines.length ? starts[j] : src.length;
          if (to > from) ranges.push({{ from, to }});
        }}
        stack.push({{ indent, key }});
      }}
      return ranges;
    }},
    findSemanticRangesLocal(output, info, pathHint) {{
      const out = String(output || '');
      if (!out) return [];
      const ranges = [];
      if (Array.isArray(pathHint) && pathHint.length) {{
        ranges.push(...this.findYamlRangesByPath(out, pathHint));
      }}
      const selectedRaw = (info && info.text ? info.text : '').trim();
      const tokenRaw = selectedRaw || this.tokenAtOffset(info && info.sourceText ? info.sourceText : '', info && info.from);
      const needleMeta = this.normalizeSemanticNeedle(tokenRaw);
      if (!needleMeta || !needleMeta.value) return ranges.slice(0, 64);
      const escaped = needleMeta.value.replace(/[\\^$.*+?()[\]|]/g, '\\$&');
      const regexes = [];
      if (needleMeta.kind === 'bool' || needleMeta.kind === 'null') {{
        regexes.push(new RegExp('\\\\b' + escaped + '\\\\b', 'g'));
      }} else if (needleMeta.kind === 'num') {{
        regexes.push(new RegExp('(^|[^0-9A-Za-z_.-])(' + escaped + ')(?=$|[^0-9A-Za-z_.-])', 'g'));
      }} else {{
        regexes.push(new RegExp('"' + escaped + '"', 'g'));
        regexes.push(new RegExp("'" + escaped + "'", 'g'));
        regexes.push(new RegExp(escaped, 'g'));
      }}
      const seen = new Set(ranges.map((r) => String(r.from) + ':' + String(r.to)));
      for (const re of regexes) {{
        let m = null;
        while ((m = re.exec(out)) !== null) {{
          const whole = String(m[0] || '');
          const needle = String(m[2] || whole);
          const from = whole === needle ? m.index : (m.index + whole.indexOf(needle));
          const to = from + needle.length;
          const key = String(from) + ':' + String(to);
          if (!seen.has(key)) {{
            seen.add(key);
            ranges.push({{ from, to }});
          }}
          if (ranges.length >= 64) return ranges;
          if (m[0].length === 0) re.lastIndex += 1;
        }}
      }}
      return ranges;
    }},
    async requestSemanticRanges(mapKey, payload) {{
      const ctrl = this.beginAbortableRequest('semantic-' + String(mapKey || 'default'));
      try {{
        const res = await fetch('/api/semantic-map', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify(payload || {{}}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('semantic-map API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if (!res.ok) {{
          throw new Error(data.message || ('semantic-map API HTTP ' + res.status));
        }}
        if (!data.ok) {{
          throw new Error(data.message || 'semantic-map failed');
        }}
        return Array.isArray(data.ranges) ? data.ranges : [];
      }} finally {{
        this.finishAbortableRequest('semantic-' + String(mapKey || 'default'), ctrl);
      }}
    }},
    applySelectionsToEditor(editor, ranges, virtualCursorPos) {{
      if (!editor) return;
      const rs = (Array.isArray(ranges) ? ranges : []).filter((r) => r && Number.isFinite(r.from) && Number.isFinite(r.to) && r.to >= r.from);
      try {{
        const hasNonEmpty = rs.some((r) => Number(r.to) > Number(r.from));
        if (hasNonEmpty) {{
          if (typeof editor.setSelections === 'function') editor.setSelections(rs);
          else if (typeof editor.setSelection === 'function') editor.setSelection(rs[0].from, rs[0].to);
          if (typeof editor.clearVirtualCursor === 'function') editor.clearVirtualCursor();
          return;
        }}
        const vp = Number(virtualCursorPos);
        if (Number.isFinite(vp)) {{
          if (typeof editor.setSelection === 'function') editor.setSelection(vp, vp);
          if (typeof editor.setVirtualCursor === 'function') editor.setVirtualCursor(vp);
          return;
        }}
        if (typeof editor.clearSelections === 'function') editor.clearSelections();
        if (typeof editor.clearVirtualCursor === 'function') editor.clearVirtualCursor();
      }} catch(_) {{}}
    }},
    mapBase64EncodeSelectionToOutput(inputText, from, to) {{
      const src = String(inputText || '');
      const a = Math.max(0, Math.min(src.length, Number(from) || 0));
      const b = Math.max(0, Math.min(src.length, Number(to) || a));
      const lo = Math.min(a, b);
      const hi = Math.max(a, b);
      let fromOut = 0;
      let toOut = 0;
      try {{
        fromOut = this.encodeUtf8Base64(src.slice(0, lo)).length;
        toOut = this.encodeUtf8Base64(src.slice(0, hi)).length;
      }} catch(_) {{
        fromOut = 0;
        toOut = 0;
      }}
      return {{ fromOut, toOut }};
    }},
    mapBase64DecodeSelectionToOutput(inputText, from, to) {{
      const src = String(inputText || '');
      const a = Math.max(0, Math.min(src.length, Number(from) || 0));
      const b = Math.max(0, Math.min(src.length, Number(to) || a));
      const lo = Math.min(a, b);
      const hi = Math.max(a, b);
      const decodeLen = (s) => {{
        const raw = String(s || '').replace(/\s+/g, '');
        for (let i = raw.length; i >= 0; i -= 1) {{
          try {{
            return this.decodeUtf8Base64(raw.slice(0, i)).length;
          }} catch(_) {{}}
        }}
        return 0;
      }};
      return {{
        fromOut: decodeLen(src.slice(0, lo)),
        toOut: decodeLen(src.slice(0, hi)),
      }};
    }},
    mapUrlEncodeSelectionToOutput(inputText, from, to) {{
      const src = String(inputText || '');
      const a = Math.max(0, Math.min(src.length, Number(from) || 0));
      const b = Math.max(0, Math.min(src.length, Number(to) || a));
      const lo = Math.min(a, b);
      const hi = Math.max(a, b);
      return {{
        fromOut: encodeURIComponent(src.slice(0, lo)).length,
        toOut: encodeURIComponent(src.slice(0, hi)).length,
      }};
    }},
    mapUrlDecodeSelectionToOutput(inputText, from, to) {{
      const src = String(inputText || '');
      const a = Math.max(0, Math.min(src.length, Number(from) || 0));
      const b = Math.max(0, Math.min(src.length, Number(to) || a));
      const lo = Math.min(a, b);
      const hi = Math.max(a, b);
      const decodeLen = (s) => {{
        const raw = String(s || '');
        for (let i = raw.length; i >= 0; i -= 1) {{
          try {{
            return decodeURIComponent(raw.slice(0, i)).length;
          }} catch(_) {{}}
        }}
        return 0;
      }};
      return {{
        fromOut: decodeLen(src.slice(0, lo)),
        toOut: decodeLen(src.slice(0, hi)),
      }};
    }},
    renderTextWithSyncOverlay(text, ranges, cursorPos) {{
      const src = String(text || '');
      if (!src) return ' ';
      const len = src.length;
      const rs = (Array.isArray(ranges) ? ranges : [])
        .map((r) => {{
          const from = Math.max(0, Math.min(len, Number(r && r.from) || 0));
          const to = Math.max(0, Math.min(len, Number(r && r.to) || 0));
          return {{ from: Math.min(from, to), to: Math.max(from, to) }};
        }})
        .filter((r) => r.to > r.from)
        .sort((a, b) => a.from - b.from);
      const out = [];
      let pos = 0;
      if (!rs.length && Number.isFinite(Number(cursorPos))) {{
        const cp = Math.max(0, Math.min(len, Number(cursorPos)));
        out.push(this.escapeHtml(src.slice(0, cp)));
        out.push("<span class='sync-cursor' aria-hidden='true'></span>");
        out.push(this.escapeHtml(src.slice(cp)));
        return out.join('') || ' ';
      }}
      for (const r of rs) {{
        const from = Math.max(pos, r.from);
        const to = Math.max(from, r.to);
        if (from > pos) out.push(this.escapeHtml(src.slice(pos, from)));
        if (to > from) out.push("<span class='sync-sel'>" + this.escapeHtml(src.slice(from, to)) + "</span>");
        pos = Math.max(pos, to);
      }}
      if (pos < len) out.push(this.escapeHtml(src.slice(pos)));
      return out.join('') || ' ';
    }},
    selectionFromEventTextArea(ev, src) {{
      const ta = ev && ev.target ? ev.target : null;
      const text = String(src || '');
      if (!ta) return {{ from: 0, to: 0, text: '' }};
      return this.makeSelectionInfo(text, {{
        from: Number.isFinite(ta.selectionStart) ? ta.selectionStart : 0,
        to: Number.isFinite(ta.selectionEnd) ? ta.selectionEnd : 0,
      }});
    }},
    onMainImportSelection(sel) {{
      const info = this.makeSelectionInfo(this.mainImportChartValuesEditor || '', sel || {{}});
      info.sourceText = this.mainImportChartValuesEditor || '';
      this.mainImportSelection = info;
      this.applyMainImportSelectionSync();
    }},
    onConverterSelection(sel) {{
      const info = this.makeSelectionInfo(this.converterInput || '', sel || {{}});
      info.sourceText = this.converterInput || '';
      this.converterSelection = info;
      this.applyConverterSelectionSync();
    }},
    onJqSelection(sel) {{
      const info = this.makeSelectionInfo(this.jqInput || '', sel || {{}});
      info.sourceText = this.jqInput || '';
      this.jqSelection = info;
      this.applyJqSelectionSync();
    }},
    onYqSelection(sel) {{
      const info = this.makeSelectionInfo(this.yqInput || '', sel || {{}});
      info.sourceText = this.yqInput || '';
      this.yqSelection = info;
      this.applyYqSelectionSync();
    }},
    onDyffFromSelection(sel) {{
      const info = this.makeSelectionInfo(this.dyffFrom || '', sel || {{}});
      info.sourceText = this.dyffFrom || '';
      this.dyffFromSelection = info;
      this.applyDyffSelectionSync();
    }},
    onDyffToSelection(sel) {{
      const info = this.makeSelectionInfo(this.dyffTo || '', sel || {{}});
      info.sourceText = this.dyffTo || '';
      this.dyffToSelection = info;
      this.applyDyffSelectionSync();
    }},
    onMainImportTextareaSelect(ev) {{
      this.onMainImportSelection(this.selectionFromEventTextArea(ev, this.mainImportChartValuesEditor || ''));
    }},
    onConverterTextareaSelect(ev) {{
      this.onConverterSelection(this.selectionFromEventTextArea(ev, this.converterInput || ''));
    }},
    onJqTextareaSelect(ev) {{
      this.onJqSelection(this.selectionFromEventTextArea(ev, this.jqInput || ''));
    }},
    onYqTextareaSelect(ev) {{
      this.onYqSelection(this.selectionFromEventTextArea(ev, this.yqInput || ''));
    }},
    onDyffFromTextareaSelect(ev) {{
      this.onDyffFromSelection(this.selectionFromEventTextArea(ev, this.dyffFrom || ''));
    }},
    onDyffToTextareaSelect(ev) {{
      this.onDyffToSelection(this.selectionFromEventTextArea(ev, this.dyffTo || ''));
    }},
    async applyMainImportSelectionSync() {{
      const info = this.mainImportSelection;
      if (!info || !this.mainImportGeneratedCm) return;
      const path = this.extractYamlPathAt(info.sourceText || '', info.from);
      try {{
        const ranges = await this.requestSemanticRanges('main-import', {{
          source: info.sourceText || '',
          output: this.mainImportOutput || '',
          sourceKind: 'yaml',
          outputKind: 'yaml',
          from: info.from,
          to: info.to,
          selectedText: info.text || '',
          pathHint: path,
        }});
        if (!Array.isArray(ranges) || !ranges.length) {{
          const local = this.findSemanticRangesLocal(this.mainImportOutput || '', info, path);
          const cursorPosLocal = Number(info.from) === Number(info.to) && local.length ? Number(local[0].from) : null;
          this.applySelectionsToEditor(this.mainImportGeneratedCm, local, cursorPosLocal);
          return;
        }}
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.mainImportGeneratedCm, ranges, cursorPos);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        const ranges = this.findSemanticRangesLocal(this.mainImportOutput || '', info, path);
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.mainImportGeneratedCm, ranges, cursorPos);
      }}
    }},
    async applyConverterSelectionSync() {{
      const info = this.converterSelection;
      if (!info) return;
      this.converterPlainRanges = [];
      this.converterPlainCursor = null;
      if (this.converterMode === 'text-to-hex' && this.converterHexDumpInteractive) {{
        const src = String(this.converterInput || '');
        const a = Math.min(info.from, info.to);
        const b = Math.max(info.from, info.to);
        const pfx = src.slice(0, a);
        const sel = src.slice(a, b);
        const fromByte = this.bytesFromText(pfx).length;
        const len = this.bytesFromText(sel).length;
        if (len > 0) {{
          this.converterHexSelStart = fromByte;
          this.converterHexSelEnd = fromByte + len - 1;
        }} else {{
          this.clearHexSelection();
        }}
        return;
      }}
      if (!this.converterOutputCm) {{
        let mapped = null;
        if (this.converterMode === 'base64-encode') mapped = this.mapBase64EncodeSelectionToOutput(this.converterInput || '', info.from, info.to);
        if (this.converterMode === 'base64-decode') mapped = this.mapBase64DecodeSelectionToOutput(this.converterInput || '', info.from, info.to);
        if (this.converterMode === 'url-encode') mapped = this.mapUrlEncodeSelectionToOutput(this.converterInput || '', info.from, info.to);
        if (this.converterMode === 'url-decode') mapped = this.mapUrlDecodeSelectionToOutput(this.converterInput || '', info.from, info.to);
        if (mapped) {{
          const fromOut = Number(mapped.fromOut) || 0;
          const toOut = Number(mapped.toOut) || fromOut;
          if (toOut > fromOut) this.converterPlainRanges = [{{ from: fromOut, to: toOut }}];
          if (Number(info.from) === Number(info.to)) this.converterPlainCursor = fromOut;
          return;
        }}
        const path = (this.converterMode === 'yaml-to-json' || this.converterMode === 'json-to-yaml')
          ? this.extractYamlPathAt(info.sourceText || '', info.from)
          : [];
        const local = this.findSemanticRangesLocal(this.converterOutput || '', info, path);
        this.converterPlainRanges = local;
        if (Number(info.from) === Number(info.to) && local.length) this.converterPlainCursor = Number(local[0].from);
        return;
      }}
      if (this.converterMode === 'base64-encode') {{
        const mapped = this.mapBase64EncodeSelectionToOutput(this.converterInput || '', info.from, info.to);
        const fromOut = Number(mapped.fromOut) || 0;
        const toOut = Number(mapped.toOut) || fromOut;
        const ranges = (toOut > fromOut) ? [{{ from: fromOut, to: toOut }}] : [];
        const cursorPos = Number(info.from) === Number(info.to) ? fromOut : null;
        this.applySelectionsToEditor(this.converterOutputCm, ranges, cursorPos);
        return;
      }}
      const path = (this.converterMode === 'yaml-to-json' || this.converterMode === 'json-to-yaml')
        ? this.extractYamlPathAt(info.sourceText || '', info.from)
        : [];
      const sourceKind = (this.converterMode === 'yaml-to-json') ? 'yaml' : ((this.converterMode === 'json-to-yaml') ? 'json' : 'text');
      const outputKind = (this.converterMode === 'yaml-to-json') ? 'json' : ((this.converterMode === 'json-to-yaml') ? 'yaml' : 'text');
      try {{
        const ranges = await this.requestSemanticRanges('converter', {{
          source: info.sourceText || '',
          output: this.converterOutput || '',
          sourceKind,
          outputKind,
          from: info.from,
          to: info.to,
          selectedText: info.text || '',
          pathHint: path,
        }});
        if (!Array.isArray(ranges) || !ranges.length) {{
          const local = this.findSemanticRangesLocal(this.converterOutput || '', info, path);
          const cursorPosLocal = Number(info.from) === Number(info.to) && local.length ? Number(local[0].from) : null;
          this.applySelectionsToEditor(this.converterOutputCm, local, cursorPosLocal);
          return;
        }}
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.converterOutputCm, ranges, cursorPos);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        const ranges = this.findSemanticRangesLocal(this.converterOutput || '', info, path);
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.converterOutputCm, ranges, cursorPos);
      }}
    }},
    async applyJqSelectionSync() {{
      const info = this.jqSelection;
      if (!info || !this.jqOutputCm) return;
      const path = this.extractYamlPathAt(info.sourceText || '', info.from);
      try {{
        const ranges = await this.requestSemanticRanges('jq', {{
          source: info.sourceText || '',
          output: this.jqOutput || '',
          sourceKind: 'auto',
          outputKind: 'json',
          from: info.from,
          to: info.to,
          selectedText: info.text || '',
          pathHint: path,
        }});
        if (!Array.isArray(ranges) || !ranges.length) {{
          const local = this.findSemanticRangesLocal(this.jqOutput || '', info, path);
          const cursorPosLocal = Number(info.from) === Number(info.to) && local.length ? Number(local[0].from) : null;
          this.applySelectionsToEditor(this.jqOutputCm, local, cursorPosLocal);
          return;
        }}
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.jqOutputCm, ranges, cursorPos);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        const ranges = this.findSemanticRangesLocal(this.jqOutput || '', info, path);
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.jqOutputCm, ranges, cursorPos);
      }}
    }},
    async applyYqSelectionSync() {{
      const info = this.yqSelection;
      if (!info || !this.yqOutputCm) return;
      const path = this.extractYamlPathAt(info.sourceText || '', info.from);
      try {{
        const ranges = await this.requestSemanticRanges('yq', {{
          source: info.sourceText || '',
          output: this.yqOutput || '',
          sourceKind: 'auto',
          outputKind: 'json',
          from: info.from,
          to: info.to,
          selectedText: info.text || '',
          pathHint: path,
        }});
        if (!Array.isArray(ranges) || !ranges.length) {{
          const local = this.findSemanticRangesLocal(this.yqOutput || '', info, path);
          const cursorPosLocal = Number(info.from) === Number(info.to) && local.length ? Number(local[0].from) : null;
          this.applySelectionsToEditor(this.yqOutputCm, local, cursorPosLocal);
          return;
        }}
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.yqOutputCm, ranges, cursorPos);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        const ranges = this.findSemanticRangesLocal(this.yqOutput || '', info, path);
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.yqOutputCm, ranges, cursorPos);
      }}
    }},
    async applyDyffSelectionSync() {{
      if (!this.dyffOutputCm) return;
      const info = this.dyffFromSelection || this.dyffToSelection;
      if (!info) return;
      try {{
        const ranges = await this.requestSemanticRanges('dyff', {{
          source: info.sourceText || '',
          output: this.dyffOutput || '',
          sourceKind: 'auto',
          outputKind: 'text',
          from: info.from,
          to: info.to,
          selectedText: info.text || '',
          pathHint: [],
        }});
        if (!Array.isArray(ranges) || !ranges.length) {{
          const local = this.findSemanticRangesLocal(this.dyffOutput || '', info, []);
          const cursorPosLocal = Number(info.from) === Number(info.to) && local.length ? Number(local[0].from) : null;
          this.applySelectionsToEditor(this.dyffOutputCm, local, cursorPosLocal);
          return;
        }}
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.dyffOutputCm, ranges, cursorPos);
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        const ranges = this.findSemanticRangesLocal(this.dyffOutput || '', info, []);
        const cursorPos = Number(info.from) === Number(info.to) && ranges.length ? Number(ranges[0].from) : null;
        this.applySelectionsToEditor(this.dyffOutputCm, ranges, cursorPos);
      }}
    }},
    escapeHtml(s) {{
      return String(s || '')
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
    }},
    escapeAttr(s) {{
      return this.escapeHtml(s)
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
    }},
    highlightStructured(src) {{
      let out = this.escapeHtml(src);
      out = out.replace(/^(\s*)([A-Za-z0-9_.-]+)(\s*:)/gm, "$1<span class='tok-key'>$2</span><span class='tok-op'>$3</span>");
      out = out.replace(/("([^"\\]|\\.)*")(\s*:)?/g, (m, q, _inner, c) => {{
        if (c) return "<span class='tok-key'>" + q + "</span><span class='tok-op'>" + c + "</span>";
        return "<span class='tok-str'>" + q + "</span>";
      }});
      out = out.replace(/\b(true|false)\b/g, "<span class='tok-bool'>$1</span>");
      out = out.replace(/\bnull\b/g, "<span class='tok-null'>$1</span>");
      out = out.replace(/\b(-?\d+(?:\.\d+)?)\b/g, "<span class='tok-num'>$1</span>");
      return out || ' ';
    }},
    highlightDyff(src) {{
      const lines = String(src || '').split('\n');
      const html = lines.map((line) => {{
        const safe = this.escapeHtml(line);
        if (safe.startsWith('added: ')) return "<span class='tok-diff-add'>" + safe + "</span>";
        if (safe.startsWith('removed: ')) return "<span class='tok-diff-rem'>" + safe + "</span>";
        if (safe.startsWith('changed: ')) return "<span class='tok-diff-chg'>" + safe + "</span>";
        return safe;
      }});
      return html.join('\n');
    }},
    highlightHexOutput(src, viewMode) {{
      const text = String(src || '');
      if (!text) return ' ';
      if (viewMode !== 'dump') {{
        return "<span class='hex-plain'>" + this.escapeHtml(text) + "</span>";
      }}
      const lines = text.split('\n');
      const out = lines.map((line) => {{
        const m = /^([0-9a-fA-F]{{8}})(\s+)(.*?)(\s+\|.*\|)?$/.exec(line);
        if (!m) return this.escapeHtml(line);
        const off = this.escapeHtml(m[1]);
        const sep = this.escapeHtml(m[2] || '  ');
        const hex = this.escapeHtml(m[3] || '');
        const ascii = this.escapeHtml(m[4] || '');
        return "<span class='hex-line'><span class='hex-offset'>" + off + "</span><span class='hex-sep'>" + sep + "</span><span class='hex-bytes'>" + hex + "</span><span class='hex-sep'>" + (ascii ? "  " : "") + "</span><span class='hex-ascii'>" + ascii + "</span></span>";
      }});
      return out.join('\n');
    }},
    highlightJq(src) {{
      let out = this.escapeHtml(src);
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
      const ctrl = this.beginAbortableRequest('jq');
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
          }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
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
        if (this.isAbortError(e)) return;
        if(reqId !== this.jqRequestSeq) return;
        this.jqError = String(e);
        this.jqOutput = '';
      }} finally {{
        this.finishAbortableRequest('jq', ctrl);
      }}
    }},
    applyYqPreset(query) {{
      this.yqQuery = query || '.';
    }},
    async runYq() {{
      this.yqError = '';
      const input = this.yqInput || '';
      const reqId = ++this.yqRequestSeq;
      if(!input.trim()) {{
        this.yqOutput = '';
        return;
      }}
      const ctrl = this.beginAbortableRequest('yq');
      try {{
        const res = await fetch('/api/yq', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            query: this.yqQuery || '.',
            input,
            docMode: this.yqDocMode,
            docIndex: this.yqDocMode === 'index' ? Number(this.yqDocIndex) : undefined,
            compact: this.yqCompact,
            rawOutput: this.yqRawOutput
          }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('yq API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.output || ('yq API HTTP ' + res.status));
        }}
        if(reqId !== this.yqRequestSeq) return;
        if(!data.ok) {{
          this.yqError = data.output || 'yq execution failed';
          this.yqOutput = '';
          return;
        }}
        this.yqOutput = data.output || '';
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        if(reqId !== this.yqRequestSeq) return;
        this.yqError = String(e);
        this.yqOutput = '';
      }} finally {{
        this.finishAbortableRequest('yq', ctrl);
      }}
    }},
    scheduleYqRun() {{
      if(this.yqTimer) {{
        clearTimeout(this.yqTimer);
      }}
      this.yqTimer = setTimeout(() => {{
        this.runYq();
      }}, 120);
    }},
    clearYq() {{
      this.yqInput = '';
      this.yqOutput = '';
      this.yqError = '';
      this.yqQuery = '.';
    }},
    loadSampleYq() {{
      this.yqQuery = '.apps[] | select(.enabled == true) | .name';
      this.yqInput = "apps:\n  - name: api\n    enabled: true\n  - name: worker\n    enabled: false\n  - name: web\n    enabled: true\n";
    }},
    async copyYqOutput() {{
      if(!this.yqOutput) return;
      try {{ await navigator.clipboard.writeText(this.yqOutput); }} catch(_) {{}}
    }},
    async runDyff() {{
      this.dyffError = '';
      const from = this.dyffFrom || '';
      const to = this.dyffTo || '';
      const reqId = ++this.dyffRequestSeq;
      if(!from.trim() && !to.trim()) {{
        this.dyffOutput = '';
        return;
      }}
      const ctrl = this.beginAbortableRequest('dyff');
      try {{
        const res = await fetch('/api/dyff', {{
          method: 'POST',
          headers: {{ 'content-type': 'application/json' }},
          body: JSON.stringify({{
            from,
            to,
            ignoreOrder: this.dyffIgnoreOrder,
            ignoreWhitespace: this.dyffIgnoreWhitespace
          }}),
          signal: ctrl && ctrl.signal ? ctrl.signal : undefined,
        }});
        const raw = await res.text();
        let data = null;
        try {{
          data = JSON.parse(raw);
        }} catch(_) {{
          throw new Error('dyff API returned non-JSON response: ' + raw.slice(0, 300));
        }}
        if(!res.ok) {{
          throw new Error(data.output || ('dyff API HTTP ' + res.status));
        }}
        if(reqId !== this.dyffRequestSeq) return;
        if(!data.ok) {{
          this.dyffError = data.output || 'dyff execution failed';
          this.dyffOutput = '';
          return;
        }}
        this.dyffOutput = data.output || '';
      }} catch(e) {{
        if (this.isAbortError(e)) return;
        if(reqId !== this.dyffRequestSeq) return;
        this.dyffError = String(e);
        this.dyffOutput = '';
      }} finally {{
        this.finishAbortableRequest('dyff', ctrl);
      }}
    }},
    scheduleDyffRun() {{
      if(this.dyffTimer) {{
        clearTimeout(this.dyffTimer);
      }}
      this.dyffTimer = setTimeout(() => {{
        this.runDyff();
      }}, 120);
    }},
    clearDyff() {{
      this.dyffFrom = '';
      this.dyffTo = '';
      this.dyffOutput = '';
      this.dyffError = '';
    }},
    loadSampleDyff() {{
      this.dyffFrom = "apiVersion: v1\nkind: Service\nmetadata:\n  name: app\nspec:\n  ports:\n    - port: 80\n";
      this.dyffTo = "apiVersion: v1\nkind: Service\nmetadata:\n  name: app\nspec:\n  ports:\n    - port: 8080\n";
    }},
    async copyDyffOutput() {{
      if(!this.dyffOutput) return;
      try {{ await navigator.clipboard.writeText(this.dyffOutput); }} catch(_) {{}}
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
    loadSampleJq() {{
      this.jqQuery = '.apps[] | select(.enabled == true) | .name';
      this.jqInput = "apps:\n  - name: api\n    enabled: true\n  - name: worker\n    enabled: false\n  - name: web\n    enabled: true\n";
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
        page_title, cm_bundle_version, model_json
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
    use tempfile::TempDir;

    #[test]
    fn page_contains_exit_button() {
        let html = render_page_html("a: 1", "global:\n  env: dev");
        assert!(html.contains("Exit"));
        assert!(html.contains("/exit"));
        assert!(html.contains("/assets/vue.global.prod.js"));
        assert!(html.contains("/assets/codemirror.bundle.js"));
        assert!(html.contains("id='app'"));
        assert!(html.contains("Wrap lines"));
        assert!(html.contains("Copy"));
        assert!(html.contains("Download"));
        assert!(html.contains("Converters"));
        assert!(html.contains("jq Playground"));
        assert!(html.contains("/api/jq"));
        assert!(html.contains("yq Playground"));
        assert!(html.contains("/api/yq"));
        assert!(html.contains("dyff Compare"));
        assert!(html.contains("/api/dyff"));
        assert!(html.contains("jq-suggest"));
        assert!(html.contains("onJqKeydown"));
        assert!(html.contains("applyJqPreset"));
        assert!(html.contains("chip-row"));
        assert!(html.contains("select enabled"));
        assert!(html.contains("YAML → JSON"));
        assert!(html.contains("localStorage"));
        assert!(html.contains("Copy values"));
        assert!(html.contains("Save as chart"));
        assert!(html.contains("/api/save-chart"));
        assert!(html.contains("Compare renders"));
        assert!(html.contains("/api/compare-renders"));
        assert!(html.contains("/api/semantic-map"));
    }

    #[test]
    fn codemirror_bundle_has_virtual_cursor_support() {
        assert!(CODEMIRROR_BUNDLE_JS.contains("setVirtualCursor"));
        assert!(CODEMIRROR_BUNDLE_JS.contains("clearVirtualCursor"));
        assert!(CODEMIRROR_BUNDLE_JS.contains("happ-virtual-cursor"));
    }

    #[test]
    fn compose_page_has_report_and_preview_sections() {
        let html = render_compose_page_html(
            "services:\n  web: {}",
            "services:\n- name: web",
            "apps-stateless:\n  web: {}",
        );
        assert!(html.contains("/assets/vue.global.prod.js"));
        assert!(html.contains("/assets/codemirror.bundle.js"));
        assert!(html.contains("id='app'"));
        assert!(html.contains("window.__HAPP_MODEL__"));
        assert!(html.contains("Search"));
        assert!(html.contains("Wrap lines"));
        assert!(html.contains("Compose Inspect"));
    }

    #[test]
    fn convert_payload_yaml_to_json_and_back() {
        let j =
            convert_payload("yaml-to-json", "a: 1\nb:\n  - x\n", "all", None).expect("yaml->json");
        assert!(j.contains("\"a\": 1"));
        let y = convert_payload("json-to-yaml", r#"{"a":1,"b":["x"]}"#, "all", None)
            .expect("json->yaml");
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
        let out = jq_payload(
            ".apps[] | .name",
            "apps:\n  - name: a\n  - name: b\n",
            "first",
            None,
            false,
            true,
        )
        .expect("jq");
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn jq_payload_supports_doc_index_mode() {
        let out =
            jq_payload(".a", "a: 1\n---\na: 2\n", "index", Some(1), false, false).expect("jq");
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

    #[test]
    fn yq_payload_runs_query_for_yaml_input() {
        let out = yq_payload(
            ".apps[] | .name",
            "apps:\n  - name: a\n  - name: b\n",
            "first",
            None,
            false,
            true,
        )
        .expect("yq");
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn dyff_payload_finds_changes() {
        let out = dyff_payload("a: 1\n", "a: 2\n", false, false).expect("dyff");
        assert!(out.contains("changed: doc[0].a"));
    }

    #[test]
    fn dyff_payload_no_differences() {
        let out = dyff_payload("a: 1\n", "a: 1\n", false, false).expect("dyff");
        assert_eq!(out, "No differences");
    }

    #[test]
    fn import_payload_rejects_empty_path() {
        let err = import_payload(
            "chart",
            "",
            "dev",
            "apps-k8s-manifests",
            "apps-k8s-manifests",
            "raw",
            "imported",
            None,
            24,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            false,
            None,
        )
        .expect_err("expected error");
        assert!(err.contains("path is required"));
    }

    #[test]
    fn import_payload_rejects_unknown_source_type() {
        let err = import_payload(
            "unknown",
            "/tmp/something",
            "dev",
            "apps-k8s-manifests",
            "apps-k8s-manifests",
            "raw",
            "imported",
            None,
            24,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            false,
            None,
        )
        .expect_err("expected error");
        assert!(err.contains("unsupported sourceType"));
    }

    #[test]
    fn compare_render_payload_rejects_non_chart_source_type() {
        let err = compare_render_payload(
            "manifests",
            "/tmp/something",
            "dev",
            "apps-k8s-manifests",
            "apps-k8s-manifests",
            "helpers",
            "imported",
            None,
            24,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            false,
            None,
            "global:\n  env: dev\n",
            None,
        )
        .expect_err("expected error");
        assert!(err.contains("supported only for sourceType=chart"));
    }

    #[test]
    fn compare_render_payload_rejects_empty_generated_values() {
        let err = compare_render_payload(
            "chart",
            "/tmp/something",
            "dev",
            "apps-k8s-manifests",
            "apps-k8s-manifests",
            "helpers",
            "imported",
            None,
            24,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            false,
            None,
            "",
            None,
        )
        .expect_err("expected error");
        assert!(err.contains("generated values are empty"));
    }

    #[test]
    fn payload_string_list_skips_empty_values() {
        let payload = serde_json::json!({
            "valuesFiles": ["a.yaml", " ", "", "b.yaml"]
        });
        let got = payload_string_list(&payload, "valuesFiles");
        assert_eq!(got, vec!["a.yaml".to_string(), "b.yaml".to_string()]);
    }

    #[test]
    fn save_chart_payload_rejects_empty_output_dir() {
        let err = save_chart_payload(
            "chart",
            "/tmp/chart",
            "",
            None,
            None,
            "global:\n  env: dev\n",
        )
        .expect_err("error");
        assert!(err.contains("outChartDir is required"));
    }

    #[test]
    fn save_chart_payload_writes_consumer_chart() {
        let td = TempDir::new().expect("tmp");
        let out = td.path().join("generated-chart");
        let msg = save_chart_payload(
            "chart",
            td.path().to_str().expect("source"),
            out.to_str().expect("path"),
            Some("demo"),
            None,
            "global:\n  env: dev\napps-stateless:\n  app:\n    enabled: true\n    containers:\n      app:\n        image:\n          name: nginx\n",
        )
        .expect("saved");
        assert!(msg.contains("Chart saved"));
        assert!(out.join("Chart.yaml").exists());
        assert!(out.join("values.yaml").exists());
        assert!(out.join("templates/init-helm-apps-library.yaml").exists());
    }

    #[test]
    fn save_chart_payload_copies_crds_from_source_chart() {
        let td = TempDir::new().expect("tmp");
        let src = td.path().join("source-chart");
        let out = td.path().join("generated-chart");
        std::fs::create_dir_all(src.join("crds")).expect("mkdir");
        std::fs::write(
            src.join("crds/widgets.example.com.yaml"),
            "kind: CustomResourceDefinition\n",
        )
        .expect("write");

        let msg = save_chart_payload(
            "chart",
            src.to_str().expect("src"),
            out.to_str().expect("out"),
            Some("demo"),
            None,
            "global:\n  env: dev\napps-stateless:\n  app:\n    enabled: true\n    containers:\n      app:\n        image:\n          name: nginx\n",
        )
        .expect("save");
        assert!(msg.contains("CRDs copied"));
        assert!(out.join("crds/widgets.example.com.yaml").exists());
    }

    #[test]
    fn semantic_map_payload_matches_yaml_path_block() {
        let source = "apps-stateless:\n  app-1:\n    service:\n      enabled: true\n";
        let output = "global:\n  env: dev\napps-stateless:\n  app-1:\n    service:\n      enabled: true\n      ports:\n        - name: http\n          port: 80\n";
        let from = source.find("service").expect("service");
        let from_utf16 = byte_to_utf16_idx(source, from);
        let ranges = semantic_map_payload(
            source,
            output,
            "yaml",
            "yaml",
            from_utf16,
            from_utf16,
            "",
            &[],
        )
        .expect("semantic map");
        assert!(!ranges.is_empty());
    }

    #[test]
    fn semantic_map_payload_handles_utf16_offsets_for_cyrillic() {
        let source = "name: привет\n";
        let output = "meta:\n  title: \"привет\"\n";
        let from = source.find("привет").expect("utf");
        let from_utf16 = byte_to_utf16_idx(source, from);
        let to_utf16 = from_utf16 + "привет".encode_utf16().count();
        let ranges = semantic_map_payload(
            source,
            output,
            "yaml",
            "yaml",
            from_utf16,
            to_utf16,
            "привет",
            &[],
        )
        .expect("semantic map");
        assert!(!ranges.is_empty());
    }

    #[test]
    fn semantic_map_payload_finds_number_in_text_output() {
        let source = "port: 8080\n";
        let output = "changed: spec.ports[0].port\nsrc: 80\ngen: 8080\n";
        let from = source.find("8080").expect("num");
        let from_utf16 = byte_to_utf16_idx(source, from);
        let ranges = semantic_map_payload(
            source,
            output,
            "yaml",
            "text",
            from_utf16,
            from_utf16 + 4,
            "8080",
            &[],
        )
        .expect("semantic map");
        assert!(!ranges.is_empty());
    }
}
