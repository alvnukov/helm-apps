use serde_yaml::{Mapping, Value};

use crate::cli::ImportArgs;

const IGNORED_IMPORTED_METADATA_LABEL_KEYS: &[&str] = &[
    "helm.sh/chart",
    "app.kubernetes.io/managed-by",
    "app.kubernetes.io/instance",
    "app.kubernetes.io/name",
    "app.kubernetes.io/version",
    "app.kubernetes.io/part-of",
    "app.kubernetes.io/component",
];

pub fn build_values(args: &ImportArgs, docs: &[Value]) -> Result<Value, String> {
    let mut group = Mapping::new();
    let mut index = 0usize;
    for doc in docs {
        if let Some((key, app)) = convert_document(args, doc, index) {
            let mut final_key = key.clone();
            let mut dup = 2usize;
            while group.contains_key(Value::String(final_key.clone())) {
                final_key = format!("{}-{}", key, dup);
                dup += 1;
            }
            group.insert(Value::String(final_key), Value::Mapping(app));
            index += 1;
        }
    }
    if group.is_empty() {
        return Err("no supported Kubernetes resources found in input".to_string());
    }

    let mut global = Mapping::new();
    global.insert(Value::String("env".into()), Value::String(args.env.clone()));

    let mut root = Mapping::new();
    root.insert(Value::String("global".into()), Value::Mapping(global));
    root.insert(Value::String(args.group_name.clone()), Value::Mapping(group));
    Ok(Value::Mapping(root))
}

fn convert_document(args: &ImportArgs, doc: &Value, index: usize) -> Option<(String, Mapping)> {
    let m = doc.as_mapping()?;
    let kind = get_str(m, "kind")?;
    let api_version = get_str(m, "apiVersion")?;

    let metadata = get_map(m, "metadata").cloned().unwrap_or_default();
    let mut name = get_str(&metadata, "name").unwrap_or_default();
    if name.trim().is_empty() {
        name = format!("{}-{}", kind.to_lowercase(), index + 1);
    }
    let ns = get_str(&metadata, "namespace").unwrap_or_default();

    let mut top = m.clone();
    top.remove(Value::String("apiVersion".into()));
    top.remove(Value::String("kind".into()));
    if !args.include_status {
        top.remove(Value::String("status".into()));
    }

    let mut app = Mapping::new();
    app.insert(Value::String("enabled".into()), Value::Bool(true));
    app.insert(Value::String("apiVersion".into()), Value::String(api_version));
    app.insert(Value::String("kind".into()), Value::String(kind.clone()));
    app.insert(Value::String("name".into()), Value::String(name.clone()));

    let mut meta_residual = metadata.clone();
    meta_residual.remove(Value::String("name".into()));
    if let Some(labels) = get_map_mut(&mut meta_residual, "labels") {
        let mut filtered = labels.clone();
        for k in IGNORED_IMPORTED_METADATA_LABEL_KEYS {
            filtered.remove(Value::String((*k).to_string()));
        }
        if filtered.is_empty() {
            meta_residual.remove(Value::String("labels".into()));
        } else {
            *labels = filtered;
        }
    }
    if let Some(s) = yaml_body_sorted(&Value::Mapping(meta_residual)) {
        app.insert(Value::String("metadata".into()), Value::String(s));
    }

    for k in ["spec", "data", "stringData", "binaryData"] {
        let key = Value::String(k.to_string());
        if let Some(v) = top.get(&key).cloned() {
            if let Some(s) = yaml_body_sorted(&v) {
                app.insert(key.clone(), Value::String(s));
            }
            top.remove(&key);
        }
    }

    for k in ["type", "immutable"] {
        let key = Value::String(k.to_string());
        if let Some(v) = top.get(&key).cloned() {
            if !v.is_null() {
                app.insert(key.clone(), v);
            }
            top.remove(&key);
        }
    }

    top.remove(Value::String("metadata".into()));
    if let Some(s) = yaml_body_sorted(&Value::Mapping(top)) {
        app.insert(Value::String("extraFields".into()), Value::String(s));
    }

    Some((generic_app_key(&kind, &ns, &name), app))
}

fn yaml_body_sorted(v: &Value) -> Option<String> {
    if is_blank_container(v) {
        return None;
    }
    let s = serde_yaml::to_string(&sort_rec(v.clone())).ok()?;
    let trimmed = s.trim().trim_start_matches("---").trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn sort_rec(v: Value) -> Value {
    match v {
        Value::Mapping(m) => {
            let mut pairs: Vec<(String, Value)> = m
                .into_iter()
                .map(|(k, v)| (k.as_str().map(ToString::to_string).unwrap_or_default(), sort_rec(v)))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = Mapping::new();
            for (k, v) in pairs {
                out.insert(Value::String(k), v);
            }
            Value::Mapping(out)
        }
        Value::Sequence(seq) => Value::Sequence(seq.into_iter().map(sort_rec).collect()),
        x => x,
    }
}

fn is_blank_container(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Sequence(s) => s.is_empty(),
        Value::Mapping(m) => m.is_empty(),
        _ => false,
    }
}

fn get_str(m: &Mapping, key: &str) -> Option<String> {
    m.get(Value::String(key.to_string())).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn get_map<'a>(m: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    m.get(Value::String(key.to_string())).and_then(|v| v.as_mapping())
}

fn get_map_mut<'a>(m: &'a mut Mapping, key: &str) -> Option<&'a mut Mapping> {
    m.get_mut(Value::String(key.to_string())).and_then(|v| v.as_mapping_mut())
}

fn generic_app_key(kind: &str, ns: &str, name: &str) -> String {
    let base = if name.trim().is_empty() {
        "resource".to_string()
    } else {
        name.trim().to_string()
    };
    let prefix = {
        let s = camel_to_kebab(kind);
        if s.is_empty() { "resource".to_string() } else { s }
    };
    if ns.trim().is_empty() {
        sanitize_key(&format!("{prefix}-{base}"))
    } else {
        sanitize_key(&format!("{prefix}-{}-{base}", ns.trim()))
    }
}

fn camel_to_kebab(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() {
            out.push('-');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn sanitize_key(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let mut out = out.trim_matches('-').to_string();
    if out.is_empty() {
        out = "item".to_string();
    }
    if out.len() > 63 {
        out.truncate(63);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use crate::cli::ImportArgs;

    fn import_args() -> ImportArgs {
        ImportArgs {
            path: "x".into(),
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
            values_files: vec![],
            set_values: vec![],
            set_string_values: vec![],
            set_file_values: vec![],
            set_json_values: vec![],
            kube_version: None,
            api_versions: vec![],
            include_crds: false,
            write_rendered_output: None,
        }
    }

    #[test]
    fn converts_manifest_to_apps_k8s_manifests() {
        let docs: Vec<Value> = serde_yaml::Deserializer::from_str(
            r#"
apiVersion: v1
kind: ConfigMap
metadata:
  name: demo
  namespace: default
data:
  a: b
"#,
        )
        .map(Value::deserialize)
        .collect::<Result<Vec<_>, _>>()
        .expect("docs");
        let values = build_values(&import_args(), &docs).expect("values");
        let txt = serde_yaml::to_string(&values).expect("yaml");
        assert!(txt.contains("apps-k8s-manifests"));
        assert!(txt.contains("kind: ConfigMap"));
        assert!(txt.contains("name: demo"));
    }

    #[test]
    fn strips_helm_labels_from_metadata() {
        let docs: Vec<Value> = serde_yaml::Deserializer::from_str(
            r#"
apiVersion: v1
kind: Service
metadata:
  name: s1
  labels:
    app.kubernetes.io/name: x
    custom: y
spec:
  type: ClusterIP
"#,
        )
        .map(Value::deserialize)
        .collect::<Result<Vec<_>, _>>()
        .expect("docs");
        let values = build_values(&import_args(), &docs).expect("values");
        let txt = serde_yaml::to_string(&values).expect("yaml");
        assert!(txt.contains("custom: y"));
        assert!(!txt.contains("app.kubernetes.io/name"));
    }
}
