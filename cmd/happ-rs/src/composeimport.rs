use serde_yaml::{Mapping, Value};

use crate::cli::ImportArgs;
use crate::composeinspect::Report;

pub fn build_values(args: &ImportArgs, report: &Report) -> Value {
    let mut root = Mapping::new();
    let mut global = Mapping::new();
    global.insert(Value::String("env".into()), Value::String(args.env.clone()));
    root.insert(Value::String("global".into()), Value::Mapping(global));

    let mut apps_stateless = Mapping::new();
    let mut apps_services = Mapping::new();

    for svc in &report.services {
        let mut app = Mapping::new();
        app.insert(Value::String("enabled".into()), Value::Bool(true));
        app.insert(Value::String("name".into()), Value::String(svc.name.clone()));

        let mut containers = Mapping::new();
        let mut c = Mapping::new();
        let image_name = svc.image.clone().unwrap_or_else(|| "busybox".to_string());
        let mut image = Mapping::new();
        image.insert(Value::String("name".into()), Value::String(image_name));
        c.insert(Value::String("image".into()), Value::Mapping(image));
        if !svc.entrypoint.is_empty() {
            c.insert(Value::String("command".into()), yaml_block_string(Value::Sequence(string_seq(&svc.entrypoint))));
        } else if let Some(entrypoint_shell) = &svc.entrypoint_shell {
            c.insert(
                Value::String("command".into()),
                yaml_block_string(Value::Sequence(string_seq(&[
                    "/bin/sh".to_string(),
                    "-lc".to_string(),
                    entrypoint_shell.to_string(),
                ]))),
            );
        }
        if !svc.command.is_empty() {
            c.insert(Value::String("args".into()), yaml_block_string(Value::Sequence(string_seq(&svc.command))));
        } else if let Some(command_shell) = &svc.command_shell {
            c.insert(
                Value::String("args".into()),
                yaml_block_string(Value::Sequence(string_seq(&[
                    "/bin/sh".to_string(),
                    "-lc".to_string(),
                    command_shell.to_string(),
                ]))),
            );
        }
        if let Some(working_dir) = &svc.working_dir {
            c.insert(Value::String("workingDir".into()), Value::String(working_dir.clone()));
        }
        if !svc.env.is_empty() {
            let mut env_vars = Mapping::new();
            for (k, v) in &svc.env {
                env_vars.insert(Value::String(k.clone()), Value::String(v.clone()));
            }
            c.insert(Value::String("envVars".into()), Value::Mapping(env_vars));
        }
        if let Some(probe) = readiness_probe(svc) {
            c.insert(Value::String("readinessProbe".into()), Value::Mapping(probe));
        }
        if !svc.ports.is_empty() {
            let ports_block = svc
                .ports
                .iter()
                .filter_map(|p| parse_target_port(p))
                .map(|p| format!("- name: p{}\n  containerPort: {}", p, p))
                .collect::<Vec<_>>()
                .join("\n");
            if !ports_block.is_empty() {
                c.insert(Value::String("ports".into()), Value::String(ports_block));
            }
        }
        if !svc.expose.is_empty() && !c.contains_key(Value::String("ports".into())) {
            let ports_block = svc
                .expose
                .iter()
                .filter_map(|p| parse_target_port(p))
                .map(|p| format!("- name: p{}\n  containerPort: {}", p, p))
                .collect::<Vec<_>>()
                .join("\n");
            if !ports_block.is_empty() {
                c.insert(Value::String("ports".into()), Value::String(ports_block));
            }
        }
        containers.insert(Value::String(svc.name.clone()), Value::Mapping(c));
        app.insert(Value::String("containers".into()), Value::Mapping(containers));
        apps_stateless.insert(Value::String(svc.name.clone()), Value::Mapping(app));

        if !svc.ports.is_empty() {
            let mut service = Mapping::new();
            service.insert(Value::String("enabled".into()), Value::Bool(true));
            let ports_block = svc
                .ports
                .iter()
                .filter_map(|p| parse_target_port(p))
                .map(|p| format!("- name: p{}\n  port: {}", p, p))
                .collect::<Vec<_>>()
                .join("\n");
            service.insert(Value::String("ports".into()), Value::String(ports_block));
            apps_services.insert(Value::String(svc.name.clone()), Value::Mapping(service));
        } else if !svc.expose.is_empty() {
            let mut service = Mapping::new();
            service.insert(Value::String("enabled".into()), Value::Bool(true));
            let ports_block = svc
                .expose
                .iter()
                .filter_map(|p| parse_target_port(p))
                .map(|p| format!("- name: p{}\n  port: {}\n  targetPort: {}", p, p, p))
                .collect::<Vec<_>>()
                .join("\n");
            if !ports_block.is_empty() {
                service.insert(Value::String("ports".into()), Value::String(ports_block));
            }
            apps_services.insert(Value::String(svc.name.clone()), Value::Mapping(service));
        }
    }

    root.insert(Value::String("apps-stateless".into()), Value::Mapping(apps_stateless));
    if !apps_services.is_empty() {
        root.insert(Value::String("apps-services".into()), Value::Mapping(apps_services));
    }
    Value::Mapping(root)
}

fn parse_target_port(s: &str) -> Option<u16> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(v) = s.parse::<u16>() {
        return Some(v);
    }
    let last = s.rsplit(':').next()?;
    let last = last.split('/').next().unwrap_or(last);
    last.parse::<u16>().ok()
}

fn string_seq(items: &[String]) -> Vec<Value> {
    items.iter().map(|x| Value::String(x.clone())).collect()
}

fn yaml_block_string(v: Value) -> Value {
    let txt = serde_yaml::to_string(&v).unwrap_or_default();
    Value::String(txt.trim().to_string())
}

fn readiness_probe(svc: &crate::composeinspect::ServiceNode) -> Option<Mapping> {
    let h = svc.healthcheck.as_ref()?;
    let mut probe = Mapping::new();
    let cmd = if !h.test.is_empty() {
        Value::Sequence(string_seq(&h.test))
    } else if let Some(sh) = &h.test_shell {
        Value::Sequence(string_seq(&[
            "/bin/sh".to_string(),
            "-lc".to_string(),
            sh.to_string(),
        ]))
    } else {
        return None;
    };
    let mut exec = Mapping::new();
    exec.insert(Value::String("command".into()), cmd);
    probe.insert(Value::String("exec".into()), Value::Mapping(exec));
    if h.timeout_seconds > 0 {
        probe.insert(Value::String("timeoutSeconds".into()), Value::Number(serde_yaml::Number::from(h.timeout_seconds)));
    }
    if h.interval_seconds > 0 {
        probe.insert(Value::String("periodSeconds".into()), Value::Number(serde_yaml::Number::from(h.interval_seconds)));
    }
    if h.start_period_seconds > 0 {
        probe.insert(
            Value::String("initialDelaySeconds".into()),
            Value::Number(serde_yaml::Number::from(h.start_period_seconds)),
        );
    }
    if h.retries > 0 {
        probe.insert(Value::String("failureThreshold".into()), Value::Number(serde_yaml::Number::from(h.retries)));
    }
    Some(probe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composeinspect::{Report, ServiceNode};
    use std::collections::BTreeMap;

    #[test]
    fn builds_stateless_and_services_from_compose_report() {
        let args = ImportArgs {
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
        };
        let rep = Report {
            source_path: "compose.yaml".into(),
            project: None,
            services: vec![ServiceNode {
                id: "service:web".into(),
                name: "web".into(),
                image: Some("nginx".into()),
                command: Vec::new(),
                command_shell: None,
                entrypoint: Vec::new(),
                entrypoint_shell: None,
                working_dir: None,
                env: BTreeMap::new(),
                expose: Vec::new(),
                healthcheck: None,
                labels: BTreeMap::new(),
                profiles: Vec::new(),
                depends_on: vec![],
                ports: vec!["8080:80".into()],
            }],
        };
        let values = build_values(&args, &rep);
        let txt = serde_yaml::to_string(&values).expect("yaml");
        assert!(txt.contains("apps-stateless"));
        assert!(txt.contains("apps-services"));
        assert!(txt.contains("containerPort: 80"));
    }

    #[test]
    fn maps_runtime_fields_from_compose() {
        let args = ImportArgs {
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
        };
        let mut env = BTreeMap::new();
        env.insert("LOG_LEVEL".to_string(), "debug".to_string());
        let rep = Report {
            source_path: "compose.yaml".into(),
            project: None,
            services: vec![ServiceNode {
                id: "service:web".into(),
                name: "web".into(),
                image: Some("nginx".into()),
                command: vec!["nginx".into(), "-g".into(), "daemon off;".into()],
                command_shell: None,
                entrypoint: vec!["/docker-entrypoint.sh".into()],
                entrypoint_shell: None,
                working_dir: Some("/work".into()),
                env,
                expose: vec!["8080".into()],
                healthcheck: Some(crate::composeinspect::Healthcheck {
                    test: vec!["CMD".into(), "curl".into(), "-f".into(), "http://127.0.0.1:8080/healthz".into()],
                    test_shell: None,
                    interval_seconds: 15,
                    timeout_seconds: 3,
                    retries: 4,
                    start_period_seconds: 20,
                }),
                labels: BTreeMap::new(),
                profiles: Vec::new(),
                depends_on: vec![],
                ports: vec![],
            }],
        };
        let values = build_values(&args, &rep);
        let txt = serde_yaml::to_string(&values).expect("yaml");
        assert!(txt.contains("workingDir: /work"));
        assert!(txt.contains("command:"));
        assert!(txt.contains("/docker-entrypoint.sh"));
        assert!(txt.contains("args:"));
        assert!(txt.contains("daemon off;"));
        assert!(txt.contains("envVars:"));
        assert!(txt.contains("LOG_LEVEL: debug"));
        assert!(txt.contains("readinessProbe:"));
        assert!(txt.contains("failureThreshold: 4"));
        assert!(txt.contains("initialDelaySeconds: 20"));
    }
}
