use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("no services found in compose file")]
    NoServices,
    #[error("compose file not found in directory {0}")]
    NotFound(String),
    #[error("unsupported format '{0}' (expected yaml or json)")]
    Format(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub source_path: String,
    pub project: Option<String>,
    pub services: Vec<ServiceNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub command: Vec<String>,
    pub command_shell: Option<String>,
    pub entrypoint: Vec<String>,
    pub entrypoint_shell: Option<String>,
    pub working_dir: Option<String>,
    pub env: BTreeMap<String, String>,
    pub expose: Vec<String>,
    pub healthcheck: Option<Healthcheck>,
    pub labels: BTreeMap<String, String>,
    pub profiles: Vec<String>,
    pub depends_on: Vec<String>,
    pub ports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Healthcheck {
    pub test: Vec<String>,
    pub test_shell: Option<String>,
    pub interval_seconds: u64,
    pub timeout_seconds: u64,
    pub retries: u64,
    pub start_period_seconds: u64,
}

pub fn load(path: &str) -> Result<Report, Error> {
    let p = resolve_compose_path(path)?;
    let body = fs::read_to_string(&p)?;
    let doc: Value = serde_yaml::from_str(&body)?;
    let map = doc.as_mapping().cloned().unwrap_or_default();
    let project = map
        .get(Value::String("name".into()))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let services = map
        .get(Value::String("services".into()))
        .and_then(Value::as_mapping)
        .cloned()
        .unwrap_or_default();
    if services.is_empty() {
        return Err(Error::NoServices);
    }

    let mut out = Vec::new();
    for (k, v) in services {
        let Some(name) = k.as_str().map(ToString::to_string) else { continue; };
        let vm = v.as_mapping().cloned().unwrap_or_default();
        let image = vm
            .get(Value::String("image".into()))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let depends_on = parse_depends_on(vm.get(Value::String("depends_on".into())));
        let ports = parse_string_list(vm.get(Value::String("ports".into())));
        out.push(ServiceNode {
            id: format!("service:{name}"),
            name,
            image,
            command: parse_string_vec(vm.get(Value::String("command".into()))),
            command_shell: parse_shell_string(vm.get(Value::String("command".into()))),
            entrypoint: parse_string_vec(vm.get(Value::String("entrypoint".into()))),
            entrypoint_shell: parse_shell_string(vm.get(Value::String("entrypoint".into()))),
            working_dir: vm
                .get(Value::String("working_dir".into()))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            env: parse_environment(vm.get(Value::String("environment".into()))),
            expose: parse_string_list(vm.get(Value::String("expose".into()))),
            healthcheck: parse_healthcheck(vm.get(Value::String("healthcheck".into()))),
            labels: parse_string_map(vm.get(Value::String("labels".into()))),
            profiles: parse_string_list(vm.get(Value::String("profiles".into()))),
            depends_on,
            ports,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Report {
        source_path: p.to_string_lossy().to_string(),
        project,
        services: out,
    })
}

fn parse_string_vec(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(|x| x.as_str().map(ToString::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn parse_shell_string(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

pub fn resolve_and_write(path: &str, format: &str, out: Option<&str>) -> Result<(), Error> {
    let report = load(path)?;
    let body = match format.trim().to_ascii_lowercase().as_str() {
        "" | "yaml" | "yml" => serde_yaml::to_string(&report)?,
        "json" => serde_json::to_string_pretty(&report).map_err(|e| Error::Io(io::Error::new(io::ErrorKind::Other, e)))?,
        other => return Err(Error::Format(other.to_string())),
    };
    if let Some(p) = out {
        fs::write(p, body.as_bytes())?;
    } else {
        let mut stdout = io::stdout();
        stdout.write_all(body.as_bytes())?;
        if !body.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
    }
    Ok(())
}

fn resolve_compose_path(path: &str) -> Result<PathBuf, Error> {
    let p = PathBuf::from(path);
    if p.is_file() {
        return Ok(p);
    }
    let candidates = [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ];
    for c in candidates {
        let x = Path::new(path).join(c);
        if x.is_file() {
            return Ok(x);
        }
    }
    Err(Error::NotFound(path.to_string()))
}

fn parse_depends_on(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(|x| x.as_str().map(ToString::to_string)).collect(),
        Some(Value::Mapping(map)) => {
            let mut keys: Vec<String> = map.keys().filter_map(|k| k.as_str().map(ToString::to_string)).collect();
            keys.sort();
            keys
        }
        _ => Vec::new(),
    }
}

fn parse_string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Sequence(seq)) => seq
            .iter()
            .map(|x| x.as_str().map(ToString::to_string).unwrap_or_else(|| serde_yaml::to_string(x).unwrap_or_default().trim().to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_string_map(v: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    match v {
        Some(Value::Mapping(m)) => {
            for (k, v) in m {
                let Some(key) = k.as_str() else { continue };
                let val = match v {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Null => String::new(),
                    _ => continue,
                };
                out.insert(key.to_string(), val);
            }
        }
        Some(Value::Sequence(seq)) => {
            for item in seq {
                let Some(s) = item.as_str() else { continue };
                if let Some((k, v)) = s.split_once('=') {
                    out.insert(k.to_string(), v.to_string());
                }
            }
        }
        _ => {}
    }
    out
}

fn parse_environment(v: Option<&Value>) -> BTreeMap<String, String> {
    parse_string_map(v)
}

fn parse_healthcheck(v: Option<&Value>) -> Option<Healthcheck> {
    let Value::Mapping(m) = v? else { return None };
    if m
        .get(Value::String("disable".into()))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    let test_raw = m.get(Value::String("test".into()));
    let test = parse_string_vec(test_raw);
    let test_shell = parse_shell_string(test_raw);
    if test.is_empty() && test_shell.is_none() {
        return None;
    }

    Some(Healthcheck {
        test,
        test_shell,
        interval_seconds: parse_duration_seconds(m.get(Value::String("interval".into()))),
        timeout_seconds: parse_duration_seconds(m.get(Value::String("timeout".into()))),
        retries: m
            .get(Value::String("retries".into()))
            .and_then(Value::as_i64)
            .map(|x| x.max(0) as u64)
            .unwrap_or(0),
        start_period_seconds: parse_duration_seconds(m.get(Value::String("start_period".into()))),
    })
}

fn parse_duration_seconds(v: Option<&Value>) -> u64 {
    let Some(v) = v else { return 0 };
    match v {
        Value::Number(n) => n.as_u64().unwrap_or(0),
        Value::String(s) => parse_duration_literal(s),
        _ => 0,
    }
}

fn parse_duration_literal(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let Ok(v) = num.parse::<u64>() else { return 0 };
    match unit {
        "" | "s" => v,
        "m" => v.saturating_mul(60),
        "h" => v.saturating_mul(3600),
        _ => 0,
    }
}

#[allow(dead_code)]
pub fn as_service_map(report: &Report) -> BTreeMap<String, ServiceNode> {
    report.services.iter().cloned().map(|s| (s.name.clone(), s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn loads_compose_services() {
        let td = TempDir::new().expect("tmp");
        let file = td.path().join("compose.yaml");
        fs::write(
            &file,
            r#"
name: demo
services:
  web:
    image: nginx
    depends_on: [db]
  db:
    image: postgres
"#,
        )
        .expect("write");
        let report = load(td.path().to_str().expect("path")).expect("load");
        assert_eq!(report.services.len(), 2);
        assert_eq!(report.project.as_deref(), Some("demo"));
    }

    #[test]
    fn loads_compose_service_runtime_fields() {
        let td = TempDir::new().expect("tmp");
        let file = td.path().join("compose.yaml");
        fs::write(
            &file,
            r#"
services:
  app:
    image: nginx:1.27
    command: ["nginx","-g","daemon off;"]
    entrypoint: "/docker-entrypoint.sh"
    working_dir: /work
    environment:
      LOG_LEVEL: debug
      EMPTY: ""
    expose:
      - "8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://127.0.0.1:8080/healthz"]
      interval: 15s
      timeout: 3s
      retries: 4
      start_period: 20s
"#,
        )
        .expect("write");

        let report = load(td.path().to_str().expect("path")).expect("load");
        let app = report
            .services
            .iter()
            .find(|s| s.name == "app")
            .expect("service");
        assert_eq!(app.command, vec!["nginx", "-g", "daemon off;"]);
        assert_eq!(app.entrypoint_shell.as_deref(), Some("/docker-entrypoint.sh"));
        assert_eq!(app.working_dir.as_deref(), Some("/work"));
        assert_eq!(app.env.get("LOG_LEVEL").map(String::as_str), Some("debug"));
        assert_eq!(app.expose, vec!["8080"]);
        assert!(app.healthcheck.is_some());
    }
}
