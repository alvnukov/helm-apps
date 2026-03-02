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
    #[error("library chart: {0}")]
    Library(String),
}

pub fn values_yaml(values: &Value) -> Result<String, Error> {
    let mut root = values.as_mapping().cloned().unwrap_or_default();
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
    fs::write(
        Path::new(out_dir).join("templates/init-helm-apps-library.yaml"),
        b"{{- include \"apps-utils.init-library\" $ }}\n",
    )?;
    write_values(
        Some(&Path::new(out_dir).join("values.yaml").to_string_lossy()),
        values,
    )?;

    let dst = Path::new(out_dir).join("charts/helm-apps");
    let src = resolve_library_path(library_chart_path)?;
    if let Some(src) = src {
        copy_dir(&src, &dst)?;
    } else if crate::assets::has_helm_apps_chart() {
        crate::assets::extract_helm_apps_chart(&dst)?;
    } else {
        return Err(Error::Library(
            "embedded helm-apps chart is unavailable and no local library chart path was resolved"
                .to_string(),
        ));
    }
    Ok(())
}

pub fn copy_chart_crds_if_any(source_chart_path: &str, out_dir: &str) -> Result<bool, Error> {
    let src_crds = Path::new(source_chart_path).join("crds");
    if !src_crds.exists() || !src_crds.is_dir() {
        return Ok(false);
    }
    let dst_crds = Path::new(out_dir).join("crds");
    copy_dir(&src_crds, &dst_crds)?;
    Ok(true)
}

fn resolve_library_path(explicit: Option<&str>) -> Result<Option<PathBuf>, Error> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.join("Chart.yaml").exists() {
            return Ok(Some(pb));
        }
        return Err(Error::Library(format!(
            "explicit path '{}' does not contain Chart.yaml",
            p
        )));
    }
    let candidate = PathBuf::from("charts/helm-apps");
    if candidate.join("Chart.yaml").exists() {
        return Ok(Some(candidate));
    }
    Ok(None)
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
        root.insert(
            Value::String("apps-k8s-manifests".into()),
            Value::Mapping(Mapping::new()),
        );
        root.insert(
            Value::String("global".into()),
            Value::Mapping(Mapping::new()),
        );
        let txt = values_yaml(&Value::Mapping(root)).expect("yaml");
        assert!(txt.starts_with("global:"));
    }

    #[test]
    fn creates_consumer_chart_files() {
        let td = TempDir::new().expect("tmp");
        let out = td.path().join("chart");
        let mut root = Mapping::new();
        root.insert(
            Value::String("global".into()),
            Value::Mapping(Mapping::new()),
        );
        root.insert(
            Value::String("apps-k8s-manifests".into()),
            Value::Mapping(Mapping::new()),
        );
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
        assert!(out.join("charts/helm-apps/Chart.yaml").exists());
    }

    #[test]
    fn rejects_invalid_explicit_library_path() {
        let td = TempDir::new().expect("tmp");
        let out = td.path().join("chart");
        let mut root = Mapping::new();
        root.insert(
            Value::String("global".into()),
            Value::Mapping(Mapping::new()),
        );
        let err = generate_consumer_chart(
            out.to_str().expect("path"),
            Some("demo"),
            &Value::Mapping(root),
            Some("/definitely/not/exist"),
        )
        .expect_err("must fail");
        assert!(matches!(err, Error::Library(_)), "{err:?}");
    }

    #[test]
    fn copies_crds_from_source_chart_when_present() {
        let td = TempDir::new().expect("tmp");
        let src = td.path().join("src-chart");
        let out = td.path().join("out-chart");
        fs::create_dir_all(src.join("crds")).expect("mkdir");
        fs::write(
            src.join("crds/demo.example.com.yaml"),
            "kind: CustomResourceDefinition\n",
        )
        .expect("write");

        let copied = copy_chart_crds_if_any(src.to_str().expect("src"), out.to_str().expect("out"))
            .expect("copy");
        assert!(copied);
        assert!(out.join("crds/demo.example.com.yaml").exists());
    }
}
