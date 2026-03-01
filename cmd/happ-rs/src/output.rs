use serde_yaml::{Mapping, Value};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub fn values_yaml(values: &Value) -> Result<String, Error> {
    let mut root = values
        .as_mapping()
        .cloned()
        .unwrap_or_default();
    let mut ordered = Mapping::new();
    if let Some(g) = root.remove(Value::String("global".into())) {
        ordered.insert(Value::String("global".into()), g);
    }
    let mut keys: Vec<String> = root
        .keys()
        .filter_map(|k| k.as_str().map(ToString::to_string))
        .collect();
    keys.sort();
    for k in keys {
        if let Some(v) = root.remove(Value::String(k.clone())) {
            ordered.insert(Value::String(k), v);
        }
    }
    let text = serde_yaml::to_string(&Value::Mapping(ordered))?;
    Ok(text.trim_start_matches("---\n").to_string())
}

pub fn write_values(path: Option<&str>, values: &Value) -> Result<(), Error> {
    let body = values_yaml(values)?;
    if let Some(p) = path {
        fs::write(p, body.as_bytes())?;
    } else {
        let mut out = io::stdout();
        out.write_all(body.as_bytes())?;
    }
    Ok(())
}

pub fn generate_consumer_chart(
    out_dir: &str,
    chart_name: Option<&str>,
    values: &Value,
    library_chart_path: Option<&str>,
) -> Result<(), Error> {
    let chart_name = chart_name
        .filter(|s| !s.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            Path::new(out_dir)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("happ-imported")
                .to_string()
        });

    fs::create_dir_all(Path::new(out_dir).join("templates"))?;
    fs::create_dir_all(Path::new(out_dir).join("charts"))?;

    let chart_yaml = format!(
        "apiVersion: v2\nname: {}\nversion: 0.1.0\ntype: application\n",
        chart_name
    );
    fs::write(Path::new(out_dir).join("Chart.yaml"), chart_yaml.as_bytes())?;
    fs::write(Path::new(out_dir).join("templates/init-helm-apps-library.yaml"), b"{{- include \"apps-utils.init-library\" $ }}\n")?;
    write_values(Some(&Path::new(out_dir).join("values.yaml").to_string_lossy()), values)?;

    let dst = Path::new(out_dir).join("charts/helm-apps");
    let src = resolve_library_path(library_chart_path);
    if let Some(src) = src {
        copy_dir(&src, &dst)?;
    }
    Ok(())
}

fn resolve_library_path(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.join("Chart.yaml").exists() {
            return Some(pb);
        }
    }
    let candidate = PathBuf::from("charts/helm-apps");
    if candidate.join("Chart.yaml").exists() {
        return Some(candidate);
    }
    None
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), Error> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir_inner(src, dst)
}

fn copy_dir_inner(src: &Path, dst: &Path) -> Result<(), Error> {
    for e in fs::read_dir(src)? {
        let e = e?;
        let p = e.path();
        let target = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_inner(&p, &target)?;
        } else {
            fs::copy(&p, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn puts_global_first_in_values_yaml() {
        let mut root = Mapping::new();
        root.insert(Value::String("apps-k8s-manifests".into()), Value::Mapping(Mapping::new()));
        root.insert(Value::String("global".into()), Value::Mapping(Mapping::new()));
        let txt = values_yaml(&Value::Mapping(root)).expect("yaml");
        assert!(txt.starts_with("global:"));
    }

    #[test]
    fn creates_consumer_chart_files() {
        let td = TempDir::new().expect("tmp");
        let out = td.path().join("chart");
        let mut root = Mapping::new();
        root.insert(Value::String("global".into()), Value::Mapping(Mapping::new()));
        root.insert(Value::String("apps-k8s-manifests".into()), Value::Mapping(Mapping::new()));
        generate_consumer_chart(
            out.to_str().expect("path"),
            Some("demo"),
            &Value::Mapping(root),
            None,
        )
        .expect("generate");
        assert!(out.join("Chart.yaml").exists());
        assert!(out.join("values.yaml").exists());
        assert!(out.join("templates/init-helm-apps-library.yaml").exists());
    }
}
