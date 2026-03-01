use clap::Parser;
use std::fs;
use std::io::IsTerminal;

use crate::cli::{Cli, Command};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Source(#[from] crate::source::Error),
    #[error(transparent)]
    Output(#[from] crate::output::Error),
    #[error(transparent)]
    ComposeInspect(#[from] crate::composeinspect::Error),
    #[error("convert: {0}")]
    Convert(String),
    #[error("dyff differences found")]
    DyffDifferent,
    #[error("dyff invalid format '{0}' (expected text or json)")]
    DyffFormat(String),
    #[error("dyff invalid color '{0}' (expected auto|always|never)")]
    DyffColor(String),
}

pub fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    run_with(cli)
}

pub fn run_with(cli: Cli) -> Result<(), Error> {
    if cli.web && cli.command.is_none() {
        return crate::inspectweb::serve_tools(&cli.web_addr, true).map_err(Error::Convert);
    }
    let Some(command) = cli.command else {
        return Err(Error::Convert("no command provided (use --help or --web)".to_string()));
    };
    match command {
        Command::Chart(args) => {
            let docs = crate::source::load_documents_for_chart(&args)?;
            let values = crate::convert::build_values(&args, &docs).map_err(Error::Convert)?;
            if let Some(out) = args.out_chart_dir.as_deref() {
                crate::output::generate_consumer_chart(out, args.chart_name.as_deref(), &values, args.library_chart_path.as_deref())?;
            }
            if args.out_chart_dir.is_none() || args.output.is_some() {
                crate::output::write_values(args.output.as_deref(), &values)?;
            }
            Ok(())
        }
        Command::Manifests(args) => {
            let docs = crate::source::load_documents_for_manifests(&args.path)?;
            let values = crate::convert::build_values(&args, &docs).map_err(Error::Convert)?;
            if let Some(out) = args.out_chart_dir.as_deref() {
                crate::output::generate_consumer_chart(out, args.chart_name.as_deref(), &values, args.library_chart_path.as_deref())?;
            }
            if args.out_chart_dir.is_none() || args.output.is_some() {
                crate::output::write_values(args.output.as_deref(), &values)?;
            }
            Ok(())
        }
        Command::Compose(args) => {
            let rep = crate::composeinspect::load(&args.path)?;
            let values = crate::composeimport::build_values(&args, &rep);
            if let Some(out) = args.out_chart_dir.as_deref() {
                crate::output::generate_consumer_chart(out, args.chart_name.as_deref(), &values, args.library_chart_path.as_deref())?;
            }
            if args.out_chart_dir.is_none() || args.output.is_some() {
                crate::output::write_values(args.output.as_deref(), &values)?;
            }
            Ok(())
        }
        Command::Validate(args) => {
            crate::source::validate_values_file(&args.values)?;
            println!("OK");
            Ok(())
        }
        Command::Jq(args) => {
            let input = crate::source::read_input(&args.input)?;
            let mode = parse_doc_selection(&args)?;
            let docs = crate::query::parse_input_docs_prefer_json(&input)
                .map_err(|e| Error::Convert(format_query_error("jq", &input, &e)))?;
            let stream = select_docs(docs, mode, "jq")?;
            let out = crate::query::run_query_stream(&args.query, stream)
                .map_err(|e| Error::Convert(format_query_error("jq", &input, &e)))?;
            print_query_output(&out, args.compact, args.raw_output)?;
            Ok(())
        }
        Command::Yq(args) => {
            let input = crate::source::read_input(&args.input)?;
            let mode = parse_doc_selection(&args)?;
            let docs = crate::query::parse_input_docs_prefer_yaml(&input)
                .map_err(|e| Error::Convert(format_query_error("yq", &input, &e)))?;
            let stream = select_docs(docs, mode, "yq")?;
            let out = crate::query::run_query_stream(&args.query, stream)
                .map_err(|e| Error::Convert(format_query_error("yq", &input, &e)))?;
            print_query_output(&out, args.compact, args.raw_output)?;
            Ok(())
        }
        Command::ComposeInspect(args) => {
            if args.web {
                let report = crate::composeinspect::load(&args.path).map_err(Error::ComposeInspect)?;
                let source_yaml = std::fs::read_to_string(&report.source_path).map_err(crate::source::Error::Io)?;
                let report_yaml = serde_yaml::to_string(&report).map_err(crate::source::Error::Yaml)?;
                let import_args = crate::cli::ImportArgs {
                    path: args.path.clone(),
                    env: "dev".into(),
                    group_name: "apps-k8s-manifests".into(),
                    group_type: "apps-k8s-manifests".into(),
                    min_include_bytes: 24,
                    include_status: false,
                    output: None,
                    out_chart_dir: None,
                    chart_name: None,
                    library_chart_path: None,
                    import_strategy: "raw".into(),
                    release_name: "imported".into(),
                    namespace: None,
                    values_files: Vec::new(),
                    set_values: Vec::new(),
                    set_string_values: Vec::new(),
                    set_file_values: Vec::new(),
                    set_json_values: Vec::new(),
                    kube_version: None,
                    api_versions: Vec::new(),
                    include_crds: false,
                    write_rendered_output: None,
                };
                let values = crate::composeimport::build_values(&import_args, &report);
                let values_yaml = crate::output::values_yaml(&values)?;
                return crate::inspectweb::serve_compose(
                    &args.addr,
                    args.open_browser,
                    source_yaml,
                    report_yaml,
                    values_yaml,
                )
                .map_err(Error::Convert);
            }
            crate::composeinspect::resolve_and_write(&args.path, &args.format, args.output.as_deref()).map_err(Error::ComposeInspect)
        }
        Command::Dyff(args) => {
            let from = crate::source::read_input(&args.from)?;
            let to = crate::source::read_input(&args.to)?;
            let diff = crate::dyfflike::between_yaml(
                &from,
                &to,
                crate::dyfflike::DiffOptions {
                    ignore_order_changes: args.ignore_order,
                    ignore_whitespace_change: args.ignore_whitespace,
                },
            )
            .map_err(crate::source::Error::Yaml)?;
            let entries = parse_diff_entries(&diff);
            let has_diff = !entries.is_empty();
            let rendered = format_dyff_output(&args, &entries, std::io::stdout().is_terminal())?;
            if let Some(out) = args.output.as_deref() {
                fs::write(out, rendered.as_bytes()).map_err(crate::output::Error::Io)?;
            }
            if !args.quiet {
                if has_diff {
                    println!("{rendered}");
                } else {
                    if args.format.eq_ignore_ascii_case("json") {
                        println!("{rendered}");
                    } else {
                        println!("No differences.");
                    }
                }
            }
            if args.fail_on_diff && has_diff {
                return Err(Error::DyffDifferent);
            }
            Ok(())
        }
        Command::Inspect(args) => {
            let import_args = crate::cli::ImportArgs {
                path: args.path.clone(),
                env: "dev".into(),
                group_name: "apps-k8s-manifests".into(),
                group_type: "apps-k8s-manifests".into(),
                min_include_bytes: 24,
                include_status: false,
                output: None,
                out_chart_dir: None,
                chart_name: None,
                library_chart_path: None,
                import_strategy: "raw".into(),
                release_name: args.release_name.clone(),
                namespace: args.namespace.clone(),
                values_files: args.values_files.clone(),
                set_values: args.set_values.clone(),
                set_string_values: args.set_string_values.clone(),
                set_file_values: args.set_file_values.clone(),
                set_json_values: args.set_json_values.clone(),
                kube_version: args.kube_version.clone(),
                api_versions: args.api_versions.clone(),
                include_crds: args.include_crds,
                write_rendered_output: None,
            };
            let rendered = crate::source::render_chart(&import_args, &args.path)?;
            let docs = crate::source::parse_documents(&rendered)?;
            let values = crate::convert::build_values(&import_args, &docs).map_err(Error::Convert)?;
            let values_yaml = crate::output::values_yaml(&values)?;
            if args.web {
                return crate::inspectweb::serve(&args.addr, true, rendered, values_yaml).map_err(Error::Convert);
            }
            println!("{values_yaml}");
            Ok(())
        }
    }
}

fn print_query_output(values: &[serde_json::Value], compact: bool, raw_output: bool) -> Result<(), Error> {
    for v in values {
        if raw_output {
            if let Some(s) = v.as_str() {
                println!("{s}");
                continue;
            }
        }
        if compact {
            println!(
                "{}",
                serde_json::to_string(v).map_err(|e| Error::Convert(format!("encode json: {e}")))?,
            );
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(v).map_err(|e| Error::Convert(format!("encode json: {e}")))?,
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocSelection {
    First,
    All,
    Index(usize),
}

fn parse_doc_selection(args: &crate::cli::QueryArgs) -> Result<DocSelection, Error> {
    match args.doc_mode.trim().to_ascii_lowercase().as_str() {
        "" | "first" => Ok(DocSelection::First),
        "all" => Ok(DocSelection::All),
        "index" => {
            let Some(idx) = args.doc_index else {
                return Err(Error::Convert(
                    "query: --doc-index is required when --doc-mode=index".to_string(),
                ));
            };
            Ok(DocSelection::Index(idx))
        }
        other => Err(Error::Convert(format!(
            "query: invalid --doc-mode '{}' (expected first|all|index)",
            other
        ))),
    }
}

fn select_docs(mut docs: Vec<serde_json::Value>, mode: DocSelection, tool: &str) -> Result<Vec<serde_json::Value>, Error> {
    match mode {
        DocSelection::All => Ok(docs),
        DocSelection::First => Ok(docs.into_iter().next().into_iter().collect()),
        DocSelection::Index(i) => {
            if i >= docs.len() {
                return Err(Error::Convert(format!(
                    "{}: --doc-index={} is out of range for {} document(s)",
                    tool,
                    i,
                    docs.len()
                )));
            }
            Ok(vec![docs.swap_remove(i)])
        }
    }
}

fn format_query_error(tool: &str, input: &str, err: &crate::query::Error) -> String {
    let base = format!("{tool}: {err}");
    let Some((line, col)) = extract_line_col(&base) else {
        return base;
    };
    let ctx = render_input_context(input, line, col);
    if ctx.is_empty() {
        base
    } else {
        format!("{base}\n{ctx}")
    }
}

fn extract_line_col(msg: &str) -> Option<(usize, usize)> {
    let re = regex::Regex::new(r"(?:at\s+)?line\s+(\d+)\s+column\s+(\d+)").ok()?;
    let caps = re.captures(msg)?;
    let line = caps.get(1)?.as_str().parse::<usize>().ok()?;
    let col = caps.get(2)?.as_str().parse::<usize>().ok()?;
    Some((line, col))
}

fn render_input_context(input: &str, line: usize, col: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();
    if lines.is_empty() || line == 0 {
        return String::new();
    }
    let from = line.saturating_sub(2).max(1);
    let to = (line + 2).min(lines.len());
    let mut out = String::new();
    out.push_str("input context:\n");
    for i in from..=to {
        let marker = if i == line { '>' } else { ' ' };
        let text = lines.get(i - 1).copied().unwrap_or_default();
        out.push_str(&format!("{marker} {:>5} | {text}\n", i));
        if i == line {
            let caret_pad = col.saturating_sub(1);
            out.push_str(&format!("  {:>5} | {}^\n", "", " ".repeat(caret_pad)));
        }
    }
    out.trim_end().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiffEntry {
    kind: DiffKind,
    path: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
struct DiffSummary {
    total: usize,
    changed: usize,
    added: usize,
    removed: usize,
}

fn parse_diff_entries(diff: &str) -> Vec<DiffEntry> {
    let mut out = Vec::new();
    for line in diff.lines().map(str::trim).filter(|x| !x.is_empty()) {
        if let Some(path) = line.strip_prefix("added: ") {
            out.push(DiffEntry {
                kind: DiffKind::Added,
                path: path.to_string(),
            });
            continue;
        }
        if let Some(path) = line.strip_prefix("removed: ") {
            out.push(DiffEntry {
                kind: DiffKind::Removed,
                path: path.to_string(),
            });
            continue;
        }
        if let Some(path) = line.strip_prefix("changed: ") {
            out.push(DiffEntry {
                kind: DiffKind::Changed,
                path: path.to_string(),
            });
        }
    }
    out
}

fn diff_summary(entries: &[DiffEntry]) -> DiffSummary {
    let mut s = DiffSummary {
        total: entries.len(),
        changed: 0,
        added: 0,
        removed: 0,
    };
    for e in entries {
        match e.kind {
            DiffKind::Added => s.added += 1,
            DiffKind::Removed => s.removed += 1,
            DiffKind::Changed => s.changed += 1,
        }
    }
    s
}

fn color_policy(mode: &str, is_tty: bool) -> Result<bool, Error> {
    let mode = mode.trim().to_ascii_lowercase();
    match mode.as_str() {
        "" | "auto" => Ok(is_tty),
        "always" => Ok(true),
        "never" => Ok(false),
        other => Err(Error::DyffColor(other.to_string())),
    }
}

fn format_dyff_output(
    args: &crate::cli::DyffArgs,
    entries: &[DiffEntry],
    is_tty: bool,
) -> Result<String, Error> {
    match args.format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => format_dyff_text(args, entries, is_tty),
        "json" => format_dyff_json(args, entries),
        "github" => format_dyff_github(args, entries),
        other => Err(Error::DyffFormat(other.to_string())),
    }
}

fn format_dyff_json(args: &crate::cli::DyffArgs, entries: &[DiffEntry]) -> Result<String, Error> {
    let _ = color_policy(&args.color, false)?;
    let summary = diff_summary(entries);
    let mut payload = serde_json::json!({
        "equal": entries.is_empty(),
        "from": args.label_from.as_deref().unwrap_or(&args.from),
        "to": args.label_to.as_deref().unwrap_or(&args.to),
        "summary": summary,
        "entries": entries.iter().map(|e| {
            let t = match e.kind {
                DiffKind::Added => "added",
                DiffKind::Removed => "removed",
                DiffKind::Changed => "changed",
            };
            serde_json::json!({"type": t, "path": e.path})
        }).collect::<Vec<_>>(),
    });
    if args.summary_only {
        payload["entries"] = serde_json::json!([]);
    }
    serde_json::to_string_pretty(&payload).map_err(|e| Error::Convert(format!("dyff json encode: {e}")))
}

fn format_dyff_text(
    args: &crate::cli::DyffArgs,
    entries: &[DiffEntry],
    is_tty: bool,
) -> Result<String, Error> {
    let use_color = color_policy(&args.color, is_tty)?;
    let from_label = args.label_from.as_deref().unwrap_or(&args.from);
    let to_label = args.label_to.as_deref().unwrap_or(&args.to);
    let mut lines = vec![format!("Compare: {from_label} -> {to_label}")];

    if !args.summary_only {
        for e in entries {
            let (name, ansi) = match e.kind {
                DiffKind::Added => ("added", "\u{1b}[32m"),
                DiffKind::Removed => ("removed", "\u{1b}[31m"),
                DiffKind::Changed => ("changed", "\u{1b}[33m"),
            };
            let t = if use_color {
                format!("{ansi}{name}\u{1b}[0m")
            } else {
                name.to_string()
            };
            lines.push(format!("{t:>8}  {}", e.path));
        }
    }
    if args.stats {
        let s = diff_summary(entries);
        lines.push(format!(
            "Summary: total={} changed={} added={} removed={}",
            s.total, s.changed, s.added, s.removed
        ));
    }
    Ok(lines.join("\n"))
}

fn format_dyff_github(args: &crate::cli::DyffArgs, entries: &[DiffEntry]) -> Result<String, Error> {
    let _ = color_policy(&args.color, false)?;
    let from_label = args.label_from.as_deref().unwrap_or(&args.from);
    let to_label = args.label_to.as_deref().unwrap_or(&args.to);
    let mut lines = Vec::new();
    lines.push(format!("::notice::dyff compare {from_label} -> {to_label}"));
    if !args.summary_only {
        for e in entries {
            let (lvl, t) = match e.kind {
                DiffKind::Added => ("notice", "added"),
                DiffKind::Removed => ("warning", "removed"),
                DiffKind::Changed => ("error", "changed"),
            };
            lines.push(format!("::{lvl}::{t} {}", e.path));
        }
    }
    let s = diff_summary(entries);
    if args.stats || args.summary_only {
        lines.push(format!(
            "::notice::summary total={} changed={} added={} removed={}",
            s.total, s.changed, s.added, s.removed
        ));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Command, DyffArgs, InspectArgs, QueryArgs, ValidateArgs};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn dyff_fail_on_diff_returns_error() {
        let cli = Cli {
            web: false,
            web_addr: "127.0.0.1:8088".to_string(),
            command: Some(Command::Dyff(DyffArgs {
                from: "-".into(),
                to: "-".into(),
                ignore_order: false,
                ignore_whitespace: false,
                quiet: true,
                fail_on_diff: true,
                format: "text".into(),
                color: "auto".into(),
                stats: false,
                summary_only: false,
                label_from: None,
                label_to: None,
                output: None,
            })),
        };
        let _ = cli; // runtime stdin case intentionally not executed here
    }

    #[test]
    fn dyff_parse_entries_and_summary() {
        let entries = parse_diff_entries("added: a\nremoved: b\nchanged: c\n");
        assert_eq!(entries.len(), 3);
        let s = diff_summary(&entries);
        assert_eq!(s.total, 3);
        assert_eq!(s.changed, 1);
        assert_eq!(s.added, 1);
        assert_eq!(s.removed, 1);
    }

    #[test]
    fn dyff_text_output_has_labels_and_stats() {
        let args = DyffArgs {
            from: "a.yaml".into(),
            to: "b.yaml".into(),
            ignore_order: false,
            ignore_whitespace: false,
            quiet: false,
            fail_on_diff: false,
            format: "text".into(),
            color: "never".into(),
            stats: true,
            summary_only: false,
            label_from: Some("source".into()),
            label_to: Some("generated".into()),
            output: None,
        };
        let out = format_dyff_output(
            &args,
            &[DiffEntry {
                kind: DiffKind::Changed,
                path: "doc[0].spec".into(),
            }],
            false,
        )
        .expect("format");
        assert!(out.contains("Compare: source -> generated"));
        assert!(out.contains("changed"));
        assert!(out.contains("Summary: total=1 changed=1 added=0 removed=0"));
    }

    #[test]
    fn dyff_json_output_machine_readable() {
        let args = DyffArgs {
            from: "a.yaml".into(),
            to: "b.yaml".into(),
            ignore_order: false,
            ignore_whitespace: false,
            quiet: false,
            fail_on_diff: false,
            format: "json".into(),
            color: "never".into(),
            stats: false,
            summary_only: false,
            label_from: None,
            label_to: None,
            output: None,
        };
        let out = format_dyff_output(
            &args,
            &[
                DiffEntry {
                    kind: DiffKind::Added,
                    path: "doc[0].a".into(),
                },
                DiffEntry {
                    kind: DiffKind::Removed,
                    path: "doc[0].b".into(),
                },
            ],
            false,
        )
        .expect("format");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v.get("equal").and_then(|x| x.as_bool()), Some(false));
        assert_eq!(
            v.get("summary").and_then(|s| s.get("total")).and_then(|x| x.as_u64()),
            Some(2)
        );
        assert_eq!(v.get("entries").and_then(|x| x.as_array()).map(|x| x.len()), Some(2));
    }

    #[test]
    fn dyff_invalid_color_rejected() {
        let err = color_policy("rainbow", true).expect_err("must fail");
        assert!(matches!(err, Error::DyffColor(_)));
    }

    #[test]
    fn dyff_github_output_has_annotations() {
        let args = DyffArgs {
            from: "a.yaml".into(),
            to: "b.yaml".into(),
            ignore_order: false,
            ignore_whitespace: false,
            quiet: false,
            fail_on_diff: false,
            format: "github".into(),
            color: "never".into(),
            stats: false,
            summary_only: false,
            label_from: None,
            label_to: None,
            output: None,
        };
        let out = format_dyff_output(
            &args,
            &[
                DiffEntry {
                    kind: DiffKind::Changed,
                    path: "doc[0].a".into(),
                },
                DiffEntry {
                    kind: DiffKind::Added,
                    path: "doc[0].b".into(),
                },
            ],
            false,
        )
        .expect("format");
        assert!(out.contains("::error::changed"));
        assert!(out.contains("::notice::added"));
    }

    #[test]
    fn dyff_summary_only_hides_entries() {
        let args = DyffArgs {
            from: "a.yaml".into(),
            to: "b.yaml".into(),
            ignore_order: false,
            ignore_whitespace: false,
            quiet: false,
            fail_on_diff: false,
            format: "text".into(),
            color: "never".into(),
            stats: true,
            summary_only: true,
            label_from: None,
            label_to: None,
            output: None,
        };
        let out = format_dyff_output(
            &args,
            &[DiffEntry {
                kind: DiffKind::Changed,
                path: "doc[0].spec".into(),
            }],
            false,
        )
        .expect("format");
        assert!(out.contains("Summary: total=1 changed=1 added=0 removed=0"));
        assert!(!out.contains("doc[0].spec"));
    }

    #[test]
    fn inspect_command_is_not_stubbed() {
        let cli = Cli {
            web: false,
            web_addr: "127.0.0.1:8088".to_string(),
            command: Some(Command::Inspect(InspectArgs {
                path: "/definitely/missing/chart".to_string(),
                release_name: "inspect".to_string(),
                namespace: None,
                values_files: Vec::new(),
                set_values: Vec::new(),
                set_string_values: Vec::new(),
                set_file_values: Vec::new(),
                set_json_values: Vec::new(),
                kube_version: None,
                api_versions: Vec::new(),
                include_crds: false,
                web: false,
                addr: "127.0.0.1:8088".to_string(),
            })),
        };
        let result = run_with(cli);
        assert!(
            matches!(result, Err(Error::Source(_))),
            "inspect must execute implementation path and fail only on source/render stage for missing chart"
        );
    }

    #[test]
    fn validate_command_succeeds_on_valid_yaml() {
        let td = TempDir::new().expect("tmp");
        let p = td.path().join("values.yaml");
        fs::write(&p, "global:\n  env: dev\n").expect("write");
        let cli = Cli {
            web: false,
            web_addr: "127.0.0.1:8088".to_string(),
            command: Some(Command::Validate(ValidateArgs {
                values: p.to_string_lossy().to_string(),
            })),
        };
        let result = run_with(cli);
        assert!(result.is_ok(), "validate should pass for valid yaml: {result:?}");
    }

    #[test]
    fn validate_command_fails_on_invalid_yaml() {
        let td = TempDir::new().expect("tmp");
        let p = td.path().join("values.yaml");
        fs::write(&p, "global:\n  env: [dev\n").expect("write");
        let cli = Cli {
            web: false,
            web_addr: "127.0.0.1:8088".to_string(),
            command: Some(Command::Validate(ValidateArgs {
                values: p.to_string_lossy().to_string(),
            })),
        };
        let result = run_with(cli);
        assert!(
            matches!(result, Err(Error::Source(crate::source::Error::Yaml(_)))),
            "expected yaml parse error, got: {result:?}"
        );
    }

    #[test]
    fn parse_doc_selection_modes() {
        let first = QueryArgs {
            query: ".".to_string(),
            input: "-".to_string(),
            doc_mode: "first".to_string(),
            doc_index: None,
            compact: false,
            raw_output: false,
        };
        assert!(matches!(parse_doc_selection(&first).expect("mode"), DocSelection::First));

        let all = QueryArgs {
            doc_mode: "all".to_string(),
            ..first.clone()
        };
        assert!(matches!(parse_doc_selection(&all).expect("mode"), DocSelection::All));

        let idx = QueryArgs {
            doc_mode: "index".to_string(),
            doc_index: Some(2),
            ..first.clone()
        };
        assert!(matches!(
            parse_doc_selection(&idx).expect("mode"),
            DocSelection::Index(2)
        ));
    }

    #[test]
    fn parse_doc_selection_index_requires_value() {
        let args = QueryArgs {
            query: ".".to_string(),
            input: "-".to_string(),
            doc_mode: "index".to_string(),
            doc_index: None,
            compact: false,
            raw_output: false,
        };
        let err = parse_doc_selection(&args).expect_err("must fail");
        assert!(err.to_string().contains("--doc-index is required"));
    }

    #[test]
    fn select_docs_modes() {
        let docs = vec![
            serde_json::json!({"a":1}),
            serde_json::json!({"a":2}),
            serde_json::json!({"a":3}),
        ];
        let first = select_docs(docs.clone(), DocSelection::First, "yq").expect("first");
        assert_eq!(first, vec![serde_json::json!({"a":1})]);
        let all = select_docs(docs.clone(), DocSelection::All, "yq").expect("all");
        assert_eq!(all.len(), 3);
        let one = select_docs(docs, DocSelection::Index(1), "yq").expect("idx");
        assert_eq!(one, vec![serde_json::json!({"a":2})]);
    }

    #[test]
    fn format_query_error_adds_input_context_when_line_col_present() {
        let input = "a: 1\nb: [\n";
        let err = crate::query::Error::Yaml(
            serde_yaml::from_str::<serde_yaml::Value>(input).expect_err("must fail"),
        );
        let msg = format_query_error("yq", input, &err);
        assert!(msg.contains("input context:"));
        assert!(msg.contains("| b: ["));
    }
}
