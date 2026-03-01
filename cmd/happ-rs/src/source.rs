use serde::Deserialize;
use serde_yaml::Value;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::ImportArgs;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("yaml parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("no YAML files found at {0}")]
    NoYamlFiles(String),
    #[error("helm template failed: {0}")]
    Helm(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderInvocation {
    program: String,
    args: Vec<String>,
}

pub fn load_documents_for_chart(args: &ImportArgs) -> Result<Vec<Value>, Error> {
    let rendered = render_chart(args, &args.path)?;
    parse_documents(&rendered)
}

pub fn load_documents_for_manifests(path: &str) -> Result<Vec<Value>, Error> {
    let files = collect_manifest_files(path)?;
    if files.is_empty() {
        return Err(Error::NoYamlFiles(path.to_string()));
    }
    let mut out = Vec::new();
    for file in files {
        let data = fs::read_to_string(&file)?;
        out.extend(parse_documents(&data)?);
    }
    Ok(flatten_k8s_lists(out))
}

pub fn parse_documents(stream: &str) -> Result<Vec<Value>, Error> {
    let mut docs = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(stream) {
        let v: Value = Value::deserialize(doc)?;
        if !v.is_null() {
            docs.push(v);
        }
    }
    Ok(flatten_k8s_lists(docs))
}

pub fn render_chart(args: &ImportArgs, chart_path: &str) -> Result<String, Error> {
    let mut last_error = String::new();
    for inv in render_invocations(args, chart_path) {
        let output = match Command::new(&inv.program).args(&inv.args).output() {
            Ok(o) => o,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                last_error = format!("{} not found", inv.program);
                continue;
            }
            Err(e) => return Err(Error::Io(e)),
        };
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            last_error = if err.is_empty() {
                format!("{} exited with status {}", inv.program, output.status)
            } else {
                format!("{}: {err}", inv.program)
            };
            continue;
        }
        let rendered = String::from_utf8_lossy(&output.stdout).to_string();
        if let Some(path) = &args.write_rendered_output {
            fs::write(path, rendered.as_bytes())?;
        }
        return Ok(rendered);
    }
    Err(Error::Helm(if last_error.is_empty() {
        "no renderer available".to_string()
    } else {
        last_error
    }))
}

fn render_invocations(args: &ImportArgs, chart_path: &str) -> Vec<RenderInvocation> {
    let mut out = Vec::with_capacity(2);

    let mut werf = RenderInvocation {
        program: "werf".to_string(),
        args: vec!["render".to_string(), "--release".to_string(), args.release_name.clone(), chart_path.to_string()],
    };
    apply_render_flags(&mut werf.args, args);
    out.push(werf);

    let mut helm = RenderInvocation {
        program: "helm".to_string(),
        args: vec!["template".to_string(), args.release_name.clone(), chart_path.to_string()],
    };
    apply_render_flags(&mut helm.args, args);
    out.push(helm);

    out
}

fn apply_render_flags(cmd_args: &mut Vec<String>, args: &ImportArgs) {
    if let Some(ns) = &args.namespace {
        if !ns.trim().is_empty() {
            cmd_args.push("--namespace".to_string());
            cmd_args.push(ns.clone());
        }
    }
    for v in &args.values_files {
        cmd_args.push("--values".to_string());
        cmd_args.push(v.clone());
    }
    for v in &args.set_values {
        cmd_args.push("--set".to_string());
        cmd_args.push(v.clone());
    }
    for v in &args.set_string_values {
        cmd_args.push("--set-string".to_string());
        cmd_args.push(v.clone());
    }
    for v in &args.set_file_values {
        cmd_args.push("--set-file".to_string());
        cmd_args.push(v.clone());
    }
    for v in &args.set_json_values {
        cmd_args.push("--set-json".to_string());
        cmd_args.push(v.clone());
    }
    if let Some(kv) = &args.kube_version {
        if !kv.trim().is_empty() {
            cmd_args.push("--kube-version".to_string());
            cmd_args.push(kv.clone());
        }
    }
    for v in &args.api_versions {
        cmd_args.push("--api-versions".to_string());
        cmd_args.push(v.clone());
    }
    if args.include_crds {
        cmd_args.push("--include-crds".to_string());
    }
}

pub fn collect_manifest_files(path: &str) -> Result<Vec<PathBuf>, Error> {
    let p = Path::new(path);
    if p.is_file() {
        return Ok(vec![p.to_path_buf()]);
    }
    let mut out = Vec::new();
    walk_yaml_files(p, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_yaml_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            walk_yaml_files(&p, out)?;
            continue;
        }
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            let low = name.to_ascii_lowercase();
            if low.ends_with(".yaml") || low.ends_with(".yml") {
                out.push(p);
            }
        }
    }
    Ok(())
}

fn flatten_k8s_lists(docs: Vec<Value>) -> Vec<Value> {
    let mut out = Vec::new();
    for doc in docs {
        if doc.get("kind").and_then(|k| k.as_str()) == Some("List") {
            if let Some(items) = doc.get("items").and_then(|v| v.as_sequence()) {
                for item in items {
                    if item.is_mapping() {
                        out.push(item.clone());
                    }
                }
            }
        } else if doc.is_mapping() {
            out.push(doc);
        }
    }
    out
}

pub fn read_input(path: &str) -> Result<String, Error> {
    if path == "-" {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        return Ok(s);
    }
    Ok(fs::read_to_string(path)?)
}

pub fn validate_values_file(path: &str) -> Result<(), Error> {
    let src = fs::read_to_string(path)?;
    for doc in serde_yaml::Deserializer::from_str(&src) {
        let _: Value = Value::deserialize(doc)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ImportArgs;
    use tempfile::TempDir;

    #[test]
    fn parse_documents_flattens_k8s_list() {
        let src = r#"
apiVersion: v1
kind: List
items:
  - apiVersion: v1
    kind: ConfigMap
    metadata:
      name: a
"#;
        let docs = parse_documents(src).expect("parse");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].get("kind").and_then(|v| v.as_str()), Some("ConfigMap"));
    }

    #[test]
    fn collect_manifest_files_walks_yaml_only() {
        let td = TempDir::new().expect("tmp");
        fs::write(td.path().join("a.yaml"), "a: 1").expect("w");
        fs::write(td.path().join("b.txt"), "x").expect("w");
        fs::create_dir_all(td.path().join("sub")).expect("mk");
        fs::write(td.path().join("sub/c.yml"), "c: 1").expect("w");

        let files = collect_manifest_files(td.path().to_str().expect("path")).expect("collect");
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn render_invocations_prefers_werf_then_helm() {
        let args = minimal_import_args();
        let inv = render_invocations(&args, "./chart");
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].program, "werf");
        assert_eq!(inv[0].args[0], "render");
        assert_eq!(inv[1].program, "helm");
        assert_eq!(inv[1].args[0], "template");
    }

    #[test]
    fn render_invocations_apply_render_flags() {
        let mut args = minimal_import_args();
        args.namespace = Some("default".into());
        args.values_files = vec!["values.yaml".into()];
        args.set_values = vec!["a=b".into()];
        args.set_string_values = vec!["x=1".into()];
        args.set_file_values = vec!["k=path.txt".into()];
        args.set_json_values = vec!["obj={}".into()];
        args.kube_version = Some("1.29.0".into());
        args.api_versions = vec!["batch/v1".into()];
        args.include_crds = true;

        let inv = render_invocations(&args, "./chart");
        let helm = &inv[1].args;
        assert!(helm.windows(2).any(|w| w == ["--namespace", "default"]));
        assert!(helm.windows(2).any(|w| w == ["--values", "values.yaml"]));
        assert!(helm.windows(2).any(|w| w == ["--set", "a=b"]));
        assert!(helm.windows(2).any(|w| w == ["--set-string", "x=1"]));
        assert!(helm.windows(2).any(|w| w == ["--set-file", "k=path.txt"]));
        assert!(helm.windows(2).any(|w| w == ["--set-json", "obj={}"]));
        assert!(helm.windows(2).any(|w| w == ["--kube-version", "1.29.0"]));
        assert!(helm.windows(2).any(|w| w == ["--api-versions", "batch/v1"]));
        assert!(helm.contains(&"--include-crds".to_string()));
    }

    #[test]
    fn validate_values_file_detects_invalid_yaml() {
        let td = TempDir::new().expect("tmp");
        let p = td.path().join("values.yaml");
        fs::write(&p, "global:\n  env: [dev\n").expect("write");
        let err = validate_values_file(p.to_str().expect("path")).expect_err("must fail");
        assert!(matches!(err, Error::Yaml(_)));
    }

    fn minimal_import_args() -> ImportArgs {
        ImportArgs {
            path: "./chart".into(),
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
            release_name: "inspect".into(),
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
        }
    }
}
