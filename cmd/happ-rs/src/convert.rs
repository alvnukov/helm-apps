use serde_yaml::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

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
    if is_helpers_strategy(&args.import_strategy) {
        return build_values_helpers_experimental(args, docs);
    }
    build_values_raw(args, docs)
}

fn is_helpers_strategy(value: &str) -> bool {
    value.trim() == "helpers"
}

fn build_values_helpers_experimental(args: &ImportArgs, docs: &[Value]) -> Result<Value, String> {
    let mut root = Mapping::new();
    let mut global = Mapping::new();
    global.insert(k("env"), Value::String(args.env.clone()));
    root.insert(k("global"), Value::Mapping(global));

    let mut apps_stateless = Mapping::new();
    let mut apps_configmaps = Mapping::new();
    let mut apps_secrets = Mapping::new();
    let mut apps_services = Mapping::new();
    let mut apps_ingresses = Mapping::new();
    let mut apps_network_policies = Mapping::new();
    let mut apps_service_accounts = Mapping::new();
    let mut apps_jobs = Mapping::new();
    let mut apps_cronjobs = Mapping::new();

    let mut stateless_by_ns_name: HashMap<String, String> = HashMap::new();
    let mut service_accounts_by_ns_name: HashMap<String, String> = HashMap::new();

    let mut pending_pdb: Vec<Value> = Vec::new();
    let mut pending_service_accounts: Vec<Value> = Vec::new();
    let mut pending_roles: Vec<Value> = Vec::new();
    let mut pending_role_bindings: Vec<Value> = Vec::new();
    let mut pending_cluster_roles: Vec<Value> = Vec::new();
    let mut pending_cluster_role_bindings: Vec<Value> = Vec::new();

    let mut raw_fallback: Vec<Value> = Vec::new();
    let mut raw_fallback_seen: HashSet<String> = HashSet::new();

    // Pass 1: workloads first.
    for doc in docs {
        let Some(m) = doc.as_mapping() else {
            continue;
        };
        if kind_of(m) != "Deployment" {
            continue;
        }
        if let Some((app_key, app)) = map_deployment_to_apps_stateless(m) {
            let final_key = insert_group_with_dedupe(&mut apps_stateless, &app_key, app);
            let name = metadata_name(m);
            if !name.is_empty() {
                stateless_by_ns_name.insert(format!("{}/{}", metadata_namespace(m), name), final_key);
            }
        } else {
            push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
        }
    }

    // Pass 2: independent resources and deferred attachments.
    for doc in docs {
        let Some(m) = doc.as_mapping() else {
            continue;
        };
        let kind = kind_of(m);
        match kind.as_str() {
            "Deployment" => {}
            "ConfigMap" => {
                if let Some((app_key, app)) = map_configmap_to_apps_configmaps(m) {
                    insert_group_with_dedupe(&mut apps_configmaps, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "Secret" => {
                if let Some((app_key, app)) = map_secret_to_apps_secrets(m) {
                    insert_group_with_dedupe(&mut apps_secrets, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "Service" => {
                if let Some((app_key, app)) = map_service_to_apps_services(m) {
                    insert_group_with_dedupe(&mut apps_services, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "Ingress" => {
                if let Some((app_key, app)) = map_ingress_to_apps_ingresses(m) {
                    insert_group_with_dedupe(&mut apps_ingresses, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "NetworkPolicy" => {
                if let Some((app_key, app)) = map_network_policy_to_apps_network_policies(m) {
                    insert_group_with_dedupe(&mut apps_network_policies, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "Job" => {
                if let Some((app_key, app)) = map_job_to_apps_jobs(m) {
                    insert_group_with_dedupe(&mut apps_jobs, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "CronJob" => {
                if let Some((app_key, app)) = map_cronjob_to_apps_cronjobs(m) {
                    insert_group_with_dedupe(&mut apps_cronjobs, &app_key, app);
                } else {
                    push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc);
                }
            }
            "PodDisruptionBudget" => pending_pdb.push(doc.clone()),
            "ServiceAccount" => pending_service_accounts.push(doc.clone()),
            "Role" => pending_roles.push(doc.clone()),
            "RoleBinding" => pending_role_bindings.push(doc.clone()),
            "ClusterRole" => pending_cluster_roles.push(doc.clone()),
            "ClusterRoleBinding" => pending_cluster_role_bindings.push(doc.clone()),
            _ => push_raw_fallback(&mut raw_fallback, &mut raw_fallback_seen, doc),
        }
    }

    attach_pdbs_to_stateless_apps(
        &mut apps_stateless,
        &stateless_by_ns_name,
        &pending_pdb,
        &mut raw_fallback,
        &mut raw_fallback_seen,
    );
    map_service_accounts_to_group(
        &pending_service_accounts,
        &mut apps_service_accounts,
        &mut service_accounts_by_ns_name,
        &mut raw_fallback,
        &mut raw_fallback_seen,
    );
    attach_rbac_to_service_accounts(
        &mut apps_service_accounts,
        &service_accounts_by_ns_name,
        &pending_roles,
        &pending_role_bindings,
        &pending_cluster_roles,
        &pending_cluster_role_bindings,
        &mut raw_fallback,
        &mut raw_fallback_seen,
    );
    attach_service_accounts_to_stateless_apps(
        &mut apps_stateless,
        &stateless_by_ns_name,
        &service_accounts_by_ns_name,
        &pending_service_accounts,
        &mut raw_fallback,
        &mut raw_fallback_seen,
    );

    if !apps_stateless.is_empty() {
        root.insert(k("apps-stateless"), Value::Mapping(apps_stateless));
    }
    if !apps_configmaps.is_empty() {
        root.insert(k("apps-configmaps"), Value::Mapping(apps_configmaps));
    }
    if !apps_secrets.is_empty() {
        root.insert(k("apps-secrets"), Value::Mapping(apps_secrets));
    }
    if !apps_services.is_empty() {
        root.insert(k("apps-services"), Value::Mapping(apps_services));
    }
    if !apps_ingresses.is_empty() {
        root.insert(k("apps-ingresses"), Value::Mapping(apps_ingresses));
    }
    if !apps_network_policies.is_empty() {
        root.insert(k("apps-network-policies"), Value::Mapping(apps_network_policies));
    }
    if !apps_service_accounts.is_empty() {
        root.insert(k("apps-service-accounts"), Value::Mapping(apps_service_accounts));
    }
    if !apps_jobs.is_empty() {
        root.insert(k("apps-jobs"), Value::Mapping(apps_jobs));
    }
    if !apps_cronjobs.is_empty() {
        root.insert(k("apps-cronjobs"), Value::Mapping(apps_cronjobs));
    }

    if !raw_fallback.is_empty() {
        let raw_values = build_values_raw(args, &raw_fallback)?;
        if let Some(raw_map) = raw_values.as_mapping() {
            merge_top_level_values(&mut root, raw_map);
        }
    }

    if root.len() == 1 {
        return Err("no supported Kubernetes resources found in input".to_string());
    }

    Ok(Value::Mapping(root))
}

fn build_values_raw(args: &ImportArgs, docs: &[Value]) -> Result<Value, String> {
    let mut group = Mapping::new();
    let mut index = 0usize;
    for doc in docs {
        if let Some((key, app)) = convert_document_raw(args, doc, index) {
            let mut final_key = key.clone();
            let mut dup = 2usize;
            while group.contains_key(&k(&final_key)) {
                final_key = format!("{}-{}", key, dup);
                dup += 1;
            }
            group.insert(k(&final_key), Value::Mapping(app));
            index += 1;
        }
    }
    if group.is_empty() {
        return Err("no supported Kubernetes resources found in input".to_string());
    }

    let mut global = Mapping::new();
    global.insert(k("env"), Value::String(args.env.clone()));

    let mut root = Mapping::new();
    root.insert(k("global"), Value::Mapping(global));
    root.insert(k(&args.group_name), Value::Mapping(group));
    Ok(Value::Mapping(root))
}

fn convert_document_raw(args: &ImportArgs, doc: &Value, index: usize) -> Option<(String, Mapping)> {
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
    top.remove(&k("apiVersion"));
    top.remove(&k("kind"));
    if !args.include_status {
        top.remove(&k("status"));
    }

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("apiVersion"), Value::String(api_version));
    app.insert(k("kind"), Value::String(kind.clone()));
    app.insert(k("name"), Value::String(name.clone()));

    let mut meta_residual = metadata.clone();
    meta_residual.remove(&k("name"));
    if let Some(labels) = get_map_mut(&mut meta_residual, "labels") {
        let mut filtered = labels.clone();
        for key in IGNORED_IMPORTED_METADATA_LABEL_KEYS {
            filtered.remove(&k(key));
        }
        if filtered.is_empty() {
            meta_residual.remove(&k("labels"));
        } else {
            *labels = filtered;
        }
    }
    if let Some(s) = yaml_body_sorted(&Value::Mapping(meta_residual)) {
        app.insert(k("metadata"), Value::String(s));
    }

    for field in ["spec", "data", "stringData", "binaryData"] {
        if let Some(v) = top.get(&k(field)).cloned() {
            if let Some(s) = yaml_body_sorted(&v) {
                app.insert(k(field), Value::String(s));
            }
            top.remove(&k(field));
        }
    }

    for field in ["type", "immutable"] {
        if let Some(v) = top.get(&k(field)).cloned() {
            if !v.is_null() {
                app.insert(k(field), v);
            }
            top.remove(&k(field));
        }
    }

    top.remove(&k("metadata"));
    if let Some(s) = yaml_body_sorted(&Value::Mapping(top)) {
        app.insert(k("extraFields"), Value::String(s));
    }

    Some((generic_app_key(&kind, &ns, &name), app))
}

fn map_configmap_to_apps_configmaps(doc: &Mapping) -> Option<(String, Mapping)> {
    if !builtin_namespace_allowed(&metadata_namespace(doc)) {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }
    if let Some(v) = doc.get(&k("data")) {
        let escaped = escape_tpl_delimiters_value(v);
        if let Some(s) = yaml_body(&escaped) {
            app.insert(k("data"), Value::String(s));
        }
    }
    if let Some(v) = doc.get(&k("binaryData")) {
        let escaped = escape_tpl_delimiters_value(v);
        if let Some(s) = yaml_body(&escaped) {
            app.insert(k("binaryData"), Value::String(s));
        }
    }

    if let Some(v) = doc.get(&k("immutable")) {
        if !v.is_null() {
            let mut extra = Mapping::new();
            extra.insert(k("immutable"), v.clone());
            if let Some(s) = yaml_body_clean(&Value::Mapping(extra)) {
                append_extra_field(&mut app, &s);
            }
        }
    }

    if let Some(extra) = extract_by_allowed(doc, &["apiVersion", "kind", "metadata", "data", "binaryData", "immutable"]) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            append_extra_field(&mut app, &s);
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_secret_to_apps_secrets(doc: &Mapping) -> Option<(String, Mapping)> {
    if !builtin_namespace_allowed(&metadata_namespace(doc)) {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }
    if let Some(v) = doc.get(&k("type")) {
        if !v.is_null() {
            app.insert(k("type"), v.clone());
        }
    }
    if let Some(v) = doc.get(&k("data")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("data"), Value::String(s));
        }
    }
    if let Some(v) = doc.get(&k("stringData")) {
        if let Some(s) = yaml_body(v) {
            let block = format!("stringData:\n{}", indent_yaml_block(&s, 2));
            append_extra_field(&mut app, &block);
        }
    }
    if let Some(v) = doc.get(&k("immutable")) {
        if !v.is_null() {
            let mut extra = Mapping::new();
            extra.insert(k("immutable"), v.clone());
            if let Some(s) = yaml_body_clean(&Value::Mapping(extra)) {
                append_extra_field(&mut app, &s);
            }
        }
    }

    if let Some(extra) = extract_by_allowed(doc, &["apiVersion", "kind", "metadata", "type", "data", "stringData", "immutable"]) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            append_extra_field(&mut app, &s);
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_deployment_to_apps_stateless(doc: &Mapping) -> Option<(String, Mapping)> {
    if kind_of(doc) != "Deployment" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let spec = get_map(doc, "spec").cloned().unwrap_or_default();
    let template = get_map(&spec, "template").cloned().unwrap_or_default();
    let pod_spec = get_map(&template, "spec").cloned().unwrap_or_default();
    let containers = get_seq(&pod_spec, "containers").cloned().unwrap_or_default();
    if containers.is_empty() {
        return None;
    }

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }

    copy_scalar_if_present(&mut app, &spec, "replicas");
    copy_scalar_if_present(&mut app, &spec, "revisionHistoryLimit");

    if let Some(v) = spec.get(&k("strategy")) {
        if let Some(s) = yaml_body_clean(v) {
            app.insert(k("strategy"), Value::String(s));
        }
    }

    if let Some(sel) = get_map(&spec, "selector") {
        if let Some(match_labels) = get_map(sel, "matchLabels") {
            if let Some(s) = yaml_body_clean(&Value::Mapping(match_labels.clone())) {
                app.insert(k("selector"), Value::String(s));
            }
        } else if let Some(s) = yaml_body_clean(&Value::Mapping(sel.clone())) {
            app.insert(k("selector"), Value::String(s));
        }
    } else if let Some(v) = spec.get(&k("selector")) {
        if let Some(s) = yaml_body_clean(v) {
            app.insert(k("selector"), Value::String(s));
        }
    }

    let container_map = map_containers_for_stateless(&containers)?;
    app.insert(k("containers"), Value::Mapping(container_map));

    let init_containers = get_seq(&pod_spec, "initContainers").cloned().unwrap_or_default();
    if !init_containers.is_empty() {
        if let Some(mapped) = map_containers_for_stateless(&init_containers) {
            app.insert(k("initContainers"), Value::Mapping(mapped));
        }
    }

    for key in [
        "automountServiceAccountToken",
        "hostIPC",
        "hostNetwork",
        "shareProcessNamespace",
        "dnsPolicy",
        "priorityClassName",
        "serviceAccountName",
        "terminationGracePeriodSeconds",
    ] {
        copy_scalar_if_present(&mut app, &pod_spec, key);
    }

    for key in [
        "affinity",
        "tolerations",
        "volumes",
        "securityContext",
        "imagePullSecrets",
        "nodeSelector",
        "topologySpreadConstraints",
        "hostAliases",
        "dnsConfig",
        "readinessGates",
        "overhead",
    ] {
        if let Some(v) = pod_spec.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                app.insert(k(key), Value::String(s));
            }
        }
    }

    if let Some(extra) = extract_deployment_extra_spec(&spec) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("extraSpec"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_service_to_apps_services(doc: &Mapping) -> Option<(String, Mapping)> {
    if kind_of(doc) != "Service" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }
    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let spec = get_map(doc, "spec").cloned().unwrap_or_default();

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }

    for key in ["ports", "selector", "sessionAffinityConfig"] {
        if let Some(v) = spec.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                app.insert(k(key), Value::String(s));
            }
        }
    }

    for key in [
        "type",
        "clusterIP",
        "externalName",
        "externalTrafficPolicy",
        "internalTrafficPolicy",
        "ipFamilyPolicy",
        "loadBalancerClass",
        "loadBalancerIP",
        "sessionAffinity",
        "publishNotReadyAddresses",
        "allocateLoadBalancerNodePorts",
        "healthCheckNodePort",
    ] {
        copy_scalar_if_present(&mut app, &spec, key);
    }

    for key in ["clusterIPs", "externalIPs", "ipFamilies", "loadBalancerSourceRanges"] {
        if let Some(v) = spec.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                app.insert(k(key), Value::String(s));
            }
        }
    }

    if let Some(extra) = extract_service_extra_spec(&spec) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("extraSpec"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_ingress_to_apps_ingresses(doc: &Mapping) -> Option<(String, Mapping)> {
    if kind_of(doc) != "Ingress" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }
    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let spec = get_map(doc, "spec").cloned().unwrap_or_default();

    let rules = get_seq(&spec, "rules").cloned().unwrap_or_default();
    if rules.len() != 1 {
        return None;
    }
    let rule = rules[0].as_mapping()?.clone();
    let host = get_str(&rule, "host").unwrap_or_default();

    let http_block = get_map(&rule, "http")?.clone();
    let paths = http_block.get(&k("paths"))?.clone();

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));
    app.insert(k("host"), Value::String(host));

    let mut ann = get_map(&metadata, "annotations").cloned().unwrap_or_default();
    if let Some(class) = get_str(&ann, "kubernetes.io/ingress.class") {
        let class = class.trim().to_string();
        if !class.is_empty() {
            app.insert(k("class"), Value::String(class));
        }
        ann.remove(&k("kubernetes.io/ingress.class"));
    }
    if let Some(s) = yaml_body(&Value::Mapping(ann)) {
        app.insert(k("annotations"), Value::String(s));
    }

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }

    if let Some(s) = yaml_body_clean(&paths) {
        app.insert(k("paths"), Value::String(s));
    } else {
        return None;
    }

    copy_scalar_if_present(&mut app, &spec, "ingressClassName");

    let tls_items = get_seq(&spec, "tls").cloned().unwrap_or_default();
    if !tls_items.is_empty() {
        if let Some(tls0) = tls_items[0].as_mapping() {
            let mut tls = Mapping::new();
            tls.insert(k("enabled"), Value::Bool(true));
            if let Some(secret_name) = get_str(tls0, "secretName") {
                if !secret_name.trim().is_empty() {
                    tls.insert(k("secret_name"), Value::String(secret_name.trim().to_string()));
                }
            }
            app.insert(k("tls"), Value::Mapping(tls));
        }
    }

    if let Some(extra) = extract_ingress_extra_spec(&spec) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("extraSpec"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_network_policy_to_apps_network_policies(doc: &Mapping) -> Option<(String, Mapping)> {
    if kind_of(doc) != "NetworkPolicy" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let spec = get_map(doc, "spec").cloned().unwrap_or_default();

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));
    app.insert(k("type"), Value::String("kubernetes".to_string()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }

    for key in ["podSelector", "policyTypes", "ingress", "egress"] {
        if let Some(v) = spec.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                app.insert(k(key), Value::String(s));
            }
        }
    }

    if let Some(extra_spec) = extract_by_allowed(&spec, &["podSelector", "policyTypes", "ingress", "egress"]) {
        if let Some(s) = yaml_body(&Value::Mapping(extra_spec)) {
            app.insert(k("spec"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_job_to_apps_jobs(doc: &Mapping) -> Option<(String, Mapping)> {
    if kind_of(doc) != "Job" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let spec = get_map(doc, "spec").cloned().unwrap_or_default();
    let template = get_map(&spec, "template").cloned().unwrap_or_default();
    let pod_spec = get_map(&template, "spec").cloned().unwrap_or_default();
    let containers = get_seq(&pod_spec, "containers").cloned().unwrap_or_default();
    if containers.is_empty() {
        return None;
    }

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }

    let mapped_containers = map_containers_for_stateless(&containers)?;
    app.insert(k("containers"), Value::Mapping(mapped_containers));

    let init_containers = get_seq(&pod_spec, "initContainers").cloned().unwrap_or_default();
    if !init_containers.is_empty() {
        if let Some(mapped) = map_containers_for_stateless(&init_containers) {
            app.insert(k("initContainers"), Value::Mapping(mapped));
        }
    }

    merge_maps(&mut app, &map_pod_spec_fields_for_library(&pod_spec));

    for key in [
        "parallelism",
        "completions",
        "backoffLimit",
        "activeDeadlineSeconds",
        "ttlSecondsAfterFinished",
        "manualSelector",
        "suspend",
        "completionMode",
    ] {
        copy_scalar_if_present(&mut app, &spec, key);
    }
    if let Some(v) = spec.get(&k("selector")) {
        if let Some(s) = yaml_body_clean(v) {
            app.insert(k("selector"), Value::String(s));
        }
    }

    if let Some(extra) = extract_by_allowed(
        &spec,
        &[
            "parallelism",
            "completions",
            "backoffLimit",
            "activeDeadlineSeconds",
            "ttlSecondsAfterFinished",
            "manualSelector",
            "suspend",
            "selector",
            "completionMode",
            "template",
        ],
    ) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("jobTemplateExtraSpec"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_cronjob_to_apps_cronjobs(doc: &Mapping) -> Option<(String, Mapping)> {
    if kind_of(doc) != "CronJob" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let spec = get_map(doc, "spec").cloned().unwrap_or_default();
    let job_template = get_map(&spec, "jobTemplate").cloned().unwrap_or_default();
    let job_spec = get_map(&job_template, "spec").cloned().unwrap_or_default();
    let template = get_map(&job_spec, "template").cloned().unwrap_or_default();
    let pod_spec = get_map(&template, "spec").cloned().unwrap_or_default();
    let containers = get_seq(&pod_spec, "containers").cloned().unwrap_or_default();
    if containers.is_empty() {
        return None;
    }

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }

    let mapped_containers = map_containers_for_stateless(&containers)?;
    app.insert(k("containers"), Value::Mapping(mapped_containers));

    let init_containers = get_seq(&pod_spec, "initContainers").cloned().unwrap_or_default();
    if !init_containers.is_empty() {
        if let Some(mapped) = map_containers_for_stateless(&init_containers) {
            app.insert(k("initContainers"), Value::Mapping(mapped));
        }
    }

    merge_maps(&mut app, &map_pod_spec_fields_for_library(&pod_spec));

    for key in ["schedule", "concurrencyPolicy", "failedJobsHistoryLimit", "startingDeadlineSeconds", "successfulJobsHistoryLimit", "suspend"] {
        copy_scalar_if_present(&mut app, &spec, key);
    }

    for key in [
        "parallelism",
        "completions",
        "backoffLimit",
        "activeDeadlineSeconds",
        "ttlSecondsAfterFinished",
        "manualSelector",
        "completionMode",
    ] {
        copy_scalar_if_present(&mut app, &job_spec, key);
    }
    if let Some(v) = job_spec.get(&k("selector")) {
        if let Some(s) = yaml_body_clean(v) {
            app.insert(k("selector"), Value::String(s));
        }
    }

    if let Some(extra) = extract_by_allowed(
        &job_spec,
        &[
            "parallelism",
            "completions",
            "backoffLimit",
            "activeDeadlineSeconds",
            "ttlSecondsAfterFinished",
            "manualSelector",
            "selector",
            "completionMode",
            "template",
        ],
    ) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("jobTemplateExtraSpec"), Value::String(s));
        }
    }

    if let Some(extra) = extract_by_allowed(
        &spec,
        &[
            "schedule",
            "concurrencyPolicy",
            "failedJobsHistoryLimit",
            "startingDeadlineSeconds",
            "successfulJobsHistoryLimit",
            "suspend",
            "jobTemplate",
        ],
    ) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("extraSpec"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app))
}

fn map_pod_spec_fields_for_library(pod_spec: &Mapping) -> Mapping {
    let mut out = Mapping::new();

    for key in [
        "automountServiceAccountToken",
        "hostIPC",
        "hostNetwork",
        "shareProcessNamespace",
        "dnsPolicy",
        "priorityClassName",
        "serviceAccountName",
        "serviceAccount",
        "terminationGracePeriodSeconds",
        "restartPolicy",
    ] {
        copy_scalar_if_present(&mut out, pod_spec, key);
    }

    for key in [
        "affinity",
        "tolerations",
        "volumes",
        "securityContext",
        "imagePullSecrets",
        "nodeSelector",
        "topologySpreadConstraints",
        "hostAliases",
        "dnsConfig",
        "readinessGates",
        "overhead",
    ] {
        if let Some(v) = pod_spec.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                out.insert(k(key), Value::String(s));
            }
        }
    }

    if let Some(extra) = extract_by_allowed(
        pod_spec,
        &[
            "containers",
            "initContainers",
            "automountServiceAccountToken",
            "hostIPC",
            "hostNetwork",
            "shareProcessNamespace",
            "dnsPolicy",
            "priorityClassName",
            "serviceAccountName",
            "serviceAccount",
            "terminationGracePeriodSeconds",
            "restartPolicy",
            "affinity",
            "tolerations",
            "volumes",
            "securityContext",
            "imagePullSecrets",
            "nodeSelector",
            "topologySpreadConstraints",
            "hostAliases",
            "dnsConfig",
            "readinessGates",
            "overhead",
        ],
    ) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            out.insert(k("podSpecExtra"), Value::String(s));
        }
    }

    out
}

fn attach_pdbs_to_stateless_apps(
    apps_stateless: &mut Mapping,
    stateless_by_ns_name: &HashMap<String, String>,
    docs: &[Value],
    raw_fallback: &mut Vec<Value>,
    raw_fallback_seen: &mut HashSet<String>,
) {
    for doc in docs {
        let Some(m) = doc.as_mapping() else {
            continue;
        };
        let name = metadata_name(m);
        let ns = metadata_namespace(m);
        let Some(app_key) = stateless_by_ns_name.get(&format!("{}/{}", ns, name)) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, doc);
            continue;
        };
        let Some(app) = apps_stateless.get_mut(&k(app_key)).and_then(Value::as_mapping_mut) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, doc);
            continue;
        };
        let spec = get_map(m, "spec").cloned().unwrap_or_default();
        let mut pdb = Mapping::new();
        pdb.insert(k("enabled"), Value::Bool(true));
        copy_scalar_if_present(&mut pdb, &spec, "maxUnavailable");
        copy_scalar_if_present(&mut pdb, &spec, "minAvailable");
        if let Some(extra) = extract_by_allowed(&spec, &["selector", "maxUnavailable", "minAvailable"]) {
            if let Some(s) = yaml_body(&Value::Mapping(extra)) {
                pdb.insert(k("extraSpec"), Value::String(s));
            }
        }
        app.insert(k("podDisruptionBudget"), Value::Mapping(pdb));
    }
}

fn map_service_accounts_to_group(
    docs: &[Value],
    group: &mut Mapping,
    by_ns_name: &mut HashMap<String, String>,
    raw_fallback: &mut Vec<Value>,
    raw_fallback_seen: &mut HashSet<String>,
) {
    for doc in docs {
        let Some(m) = doc.as_mapping() else {
            continue;
        };
        let Some((app_key, app, ns, name)) = map_service_account_to_apps_service_accounts(m) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, doc);
            continue;
        };
        let final_key = insert_group_with_dedupe(group, &app_key, app);
        by_ns_name.insert(format!("{}/{}", normalized_ns(&ns), name), final_key);
    }
}

fn map_service_account_to_apps_service_accounts(doc: &Mapping) -> Option<(String, Mapping, String, String)> {
    if kind_of(doc) != "ServiceAccount" {
        return None;
    }
    let name = metadata_name(doc);
    if name.is_empty() {
        return None;
    }
    let ns = metadata_namespace(doc);
    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();

    let mut app = Mapping::new();
    app.insert(k("enabled"), Value::Bool(true));
    app.insert(k("name"), Value::String(name.clone()));
    if !ns.trim().is_empty() && ns.trim() != "default" {
        app.insert(k("namespace"), Value::String(ns.clone()));
    }

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            app.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            app.insert(k("annotations"), Value::String(s));
        }
    }

    copy_scalar_if_present(&mut app, doc, "automountServiceAccountToken");

    if let Some(v) = doc.get(&k("imagePullSecrets")) {
        if let Some(s) = yaml_body_clean(v) {
            app.insert(k("imagePullSecrets"), Value::String(s));
        }
    }

    if let Some(extra) = extract_by_allowed(
        doc,
        &["apiVersion", "kind", "metadata", "automountServiceAccountToken", "imagePullSecrets"],
    ) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            app.insert(k("extraFields"), Value::String(s));
        }
    }

    Some((sanitize_key(&name), app, ns, name))
}

#[allow(clippy::too_many_arguments)]
fn attach_rbac_to_service_accounts(
    service_accounts_group: &mut Mapping,
    service_accounts_by_ns_name: &HashMap<String, String>,
    roles: &[Value],
    role_bindings: &[Value],
    cluster_roles: &[Value],
    cluster_role_bindings: &[Value],
    raw_fallback: &mut Vec<Value>,
    raw_fallback_seen: &mut HashSet<String>,
) {
    let mut role_by_ns_name: HashMap<String, Value> = HashMap::new();
    let mut cluster_role_by_name: HashMap<String, Value> = HashMap::new();
    for role in roles {
        if let Some(m) = role.as_mapping() {
            role_by_ns_name.insert(format!("{}/{}", metadata_namespace(m), metadata_name(m)), role.clone());
        }
    }
    for role in cluster_roles {
        if let Some(m) = role.as_mapping() {
            cluster_role_by_name.insert(metadata_name(m), role.clone());
        }
    }

    let mut attached_roles: HashSet<String> = HashSet::new();
    let mut attached_role_bindings: HashSet<String> = HashSet::new();
    let mut attached_cluster_roles: HashSet<String> = HashSet::new();
    let mut attached_cluster_role_bindings: HashSet<String> = HashSet::new();

    for rb in role_bindings {
        let Some(rb_map) = rb.as_mapping() else {
            continue;
        };
        let Some((sa_ns, sa_name)) = role_binding_target_service_account(rb_map) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
            continue;
        };
        let Some(sa_group_key) = service_accounts_by_ns_name.get(&format!("{}/{}", sa_ns, sa_name)) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
            continue;
        };

        let role_ref = get_map(rb_map, "roleRef").cloned().unwrap_or_default();
        if get_str(&role_ref, "apiGroup").unwrap_or_default().trim() != "rbac.authorization.k8s.io"
            || get_str(&role_ref, "kind").unwrap_or_default().trim() != "Role"
        {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
            continue;
        }
        let role_name = get_str(&role_ref, "name").unwrap_or_default();
        let role_lookup = format!("{}/{}", metadata_namespace(rb_map), role_name.trim());
        let Some(role_doc) = role_by_ns_name.get(&role_lookup) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
            continue;
        };
        let Some(role_map) = role_doc.as_mapping() else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
            continue;
        };

        if !attach_role_doc_to_service_account(
            service_accounts_group,
            sa_group_key,
            role_map,
            rb_map,
            &metadata_name(role_map),
            "Role",
            false,
        ) {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
            push_raw_fallback(raw_fallback, raw_fallback_seen, role_doc);
            continue;
        }

        attached_roles.insert(resource_uid(role_doc));
        attached_role_bindings.insert(resource_uid(rb));
    }

    for crb in cluster_role_bindings {
        let Some(crb_map) = crb.as_mapping() else {
            continue;
        };
        let Some((sa_ns, sa_name)) = role_binding_target_service_account(crb_map) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, crb);
            continue;
        };
        let Some(sa_group_key) = service_accounts_by_ns_name.get(&format!("{}/{}", sa_ns, sa_name)) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, crb);
            continue;
        };

        let role_ref = get_map(crb_map, "roleRef").cloned().unwrap_or_default();
        if get_str(&role_ref, "apiGroup").unwrap_or_default().trim() != "rbac.authorization.k8s.io"
            || get_str(&role_ref, "kind").unwrap_or_default().trim() != "ClusterRole"
        {
            push_raw_fallback(raw_fallback, raw_fallback_seen, crb);
            continue;
        }
        let role_name = get_str(&role_ref, "name").unwrap_or_default();
        let Some(role_doc) = cluster_role_by_name.get(role_name.trim()) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, crb);
            continue;
        };
        let Some(role_map) = role_doc.as_mapping() else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, crb);
            continue;
        };

        if !attach_role_doc_to_service_account(
            service_accounts_group,
            sa_group_key,
            role_map,
            crb_map,
            &metadata_name(role_map),
            "ClusterRole",
            true,
        ) {
            push_raw_fallback(raw_fallback, raw_fallback_seen, crb);
            push_raw_fallback(raw_fallback, raw_fallback_seen, role_doc);
            continue;
        }

        attached_cluster_roles.insert(resource_uid(role_doc));
        attached_cluster_role_bindings.insert(resource_uid(crb));
    }

    for role in roles {
        if !attached_roles.contains(&resource_uid(role)) {
            push_raw_fallback(raw_fallback, raw_fallback_seen, role);
        }
    }
    for rb in role_bindings {
        if !attached_role_bindings.contains(&resource_uid(rb)) {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
        }
    }
    for role in cluster_roles {
        if !attached_cluster_roles.contains(&resource_uid(role)) {
            push_raw_fallback(raw_fallback, raw_fallback_seen, role);
        }
    }
    for rb in cluster_role_bindings {
        if !attached_cluster_role_bindings.contains(&resource_uid(rb)) {
            push_raw_fallback(raw_fallback, raw_fallback_seen, rb);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn attach_role_doc_to_service_account(
    service_accounts_group: &mut Mapping,
    sa_group_key: &str,
    role_doc: &Mapping,
    binding_doc: &Mapping,
    role_key: &str,
    role_kind: &str,
    cluster_role: bool,
) -> bool {
    let Some(mut role_app) = map_role_doc_to_service_account_role(role_doc) else {
        return false;
    };
    if let Some(binding_override) = map_role_binding_override(binding_doc, role_doc, role_kind) {
        role_app.insert(k("binding"), Value::Mapping(binding_override));
    }

    let Some(sa_app) = service_accounts_group
        .get_mut(&k(sa_group_key))
        .and_then(Value::as_mapping_mut)
    else {
        return false;
    };

    let roles_field = if cluster_role { "clusterRoles" } else { "roles" };
    let roles_map = get_or_create_child_mapping(sa_app, roles_field);
    let role_entry_key = dedupe_group_key(roles_map, &sanitize_key(role_key));
    roles_map.insert(k(&role_entry_key), Value::Mapping(role_app));

    true
}

fn map_role_doc_to_service_account_role(doc: &Mapping) -> Option<Mapping> {
    let rules = get_seq(doc, "rules").cloned().unwrap_or_default();
    if rules.is_empty() {
        return None;
    }

    let metadata = get_map(doc, "metadata").cloned().unwrap_or_default();
    let mut role = Mapping::new();
    let role_name = metadata_name(doc);
    if !role_name.is_empty() {
        role.insert(k("name"), Value::String(role_name));
    }

    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            role.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            role.insert(k("annotations"), Value::String(s));
        }
    }

    let width = rules.len().to_string().len().max(1);
    let mut rules_map = Mapping::new();

    for (idx, rule) in rules.iter().enumerate() {
        let Some(rule_map) = rule.as_mapping() else {
            continue;
        };
        if rule_map.is_empty() {
            continue;
        }
        let mut item = Mapping::new();
        for field in ["apiGroups", "resources", "verbs", "resourceNames", "nonResourceURLs"] {
            if let Some(v) = rule_map.get(&k(field)) {
                if let Some(clean) = clean_value(v.clone()) {
                    item.insert(k(field), clean);
                }
            }
        }
        if let Some(extra) = extract_by_allowed(rule_map, &["apiGroups", "resources", "verbs", "resourceNames", "nonResourceURLs"]) {
            if let Some(s) = yaml_body(&Value::Mapping(extra)) {
                item.insert(k("extraFields"), Value::String(s));
            }
        }
        if item.is_empty() {
            continue;
        }
        let key_name = if width > 1 {
            format!("rule-{num:0width$}", num = idx + 1, width = width)
        } else {
            format!("rule-{}", idx + 1)
        };
        rules_map.insert(k(&key_name), Value::Mapping(item));
    }

    if rules_map.is_empty() {
        return None;
    }

    role.insert(k("rules"), Value::Mapping(rules_map));

    if let Some(extra) = extract_by_allowed(doc, &["apiVersion", "kind", "metadata", "rules"]) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            role.insert(k("extraFields"), Value::String(s));
        }
    }

    Some(role)
}

fn map_role_binding_override(binding_doc: &Mapping, role_doc: &Mapping, role_kind: &str) -> Option<Mapping> {
    let role_ref = get_map(binding_doc, "roleRef").cloned().unwrap_or_default();
    if get_str(&role_ref, "apiGroup").unwrap_or_default().trim() != "rbac.authorization.k8s.io" {
        return None;
    }
    if get_str(&role_ref, "kind").unwrap_or_default().trim() != role_kind.trim() {
        return None;
    }
    if get_str(&role_ref, "name").unwrap_or_default().trim() != metadata_name(role_doc).trim() {
        return None;
    }

    let mut override_map = Mapping::new();

    let binding_name = metadata_name(binding_doc);
    let role_name = metadata_name(role_doc);
    if !binding_name.is_empty() && binding_name != role_name {
        override_map.insert(k("name"), Value::String(binding_name));
    }

    let metadata = get_map(binding_doc, "metadata").cloned().unwrap_or_default();
    if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
        if let Some(s) = yaml_body(&labels) {
            override_map.insert(k("labels"), Value::String(s));
        }
    }
    if let Some(v) = metadata.get(&k("annotations")) {
        if let Some(s) = yaml_body(v) {
            override_map.insert(k("annotations"), Value::String(s));
        }
    }

    let binding_ns = metadata_namespace(binding_doc);
    if !binding_ns.is_empty() && binding_ns != "default" {
        override_map.insert(k("namespace"), Value::String(binding_ns));
    }

    if !is_default_single_service_account_binding_subjects(binding_doc) {
        if let Some(v) = binding_doc.get(&k("subjects")) {
            if let Some(s) = yaml_body_clean(v) {
                override_map.insert(k("subjects"), Value::String(s));
            }
        }
    }

    if let Some(extra) = extract_by_allowed(binding_doc, &["apiVersion", "kind", "metadata", "subjects", "roleRef"]) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            override_map.insert(k("extraFields"), Value::String(s));
        }
    }

    if override_map.is_empty() {
        None
    } else {
        Some(override_map)
    }
}

fn is_default_single_service_account_binding_subjects(binding_doc: &Mapping) -> bool {
    let subjects = get_seq(binding_doc, "subjects").cloned().unwrap_or_default();
    if subjects.len() != 1 {
        return false;
    }
    let Some(subject) = subjects[0].as_mapping() else {
        return false;
    };
    if get_str(subject, "kind").unwrap_or_default().trim() != "ServiceAccount" {
        return false;
    }
    if get_str(subject, "name").unwrap_or_default().trim().is_empty() {
        return false;
    }
    let subject_ns = normalized_ns(&get_str(subject, "namespace").unwrap_or_default());
    let binding_ns = normalized_ns(&metadata_namespace(binding_doc));
    subject_ns == binding_ns
}

fn role_binding_target_service_account(doc: &Mapping) -> Option<(String, String)> {
    let subjects = get_seq(doc, "subjects").cloned().unwrap_or_default();
    if subjects.is_empty() {
        return None;
    }

    let mut seen: HashSet<String> = HashSet::new();
    for subject in subjects {
        let Some(s) = subject.as_mapping() else {
            continue;
        };
        let kind = get_str(s, "kind").unwrap_or_default();
        if kind.trim().is_empty() {
            return None;
        }
        if kind.trim() != "ServiceAccount" {
            continue;
        }

        let sa_name = get_str(s, "name").unwrap_or_default();
        if sa_name.trim().is_empty() {
            return None;
        }

        let subject_ns = get_str(s, "namespace").unwrap_or_else(|| metadata_namespace(doc));
        let sa_ns = normalized_ns(&subject_ns);
        seen.insert(format!("{}/{}", sa_ns, sa_name.trim()));
    }

    if seen.len() != 1 {
        return None;
    }

    let key = seen.into_iter().next()?;
    let (ns, name) = key.split_once('/')?;
    Some((ns.to_string(), name.to_string()))
}

fn attach_service_accounts_to_stateless_apps(
    apps_stateless: &mut Mapping,
    stateless_by_ns_name: &HashMap<String, String>,
    service_accounts_by_ns_name: &HashMap<String, String>,
    docs: &[Value],
    raw_fallback: &mut Vec<Value>,
    raw_fallback_seen: &mut HashSet<String>,
) {
    for doc in docs {
        let Some(m) = doc.as_mapping() else {
            continue;
        };
        let name = metadata_name(m);
        let ns = metadata_namespace(m);

        if service_accounts_by_ns_name.contains_key(&format!("{}/{}", normalized_ns(&ns), name)) {
            continue;
        }

        let Some(app_key) = stateless_by_ns_name.get(&format!("{}/{}", ns, name)) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, doc);
            continue;
        };
        let Some(app) = apps_stateless.get_mut(&k(app_key)).and_then(Value::as_mapping_mut) else {
            push_raw_fallback(raw_fallback, raw_fallback_seen, doc);
            continue;
        };

        let metadata = get_map(m, "metadata").cloned().unwrap_or_default();
        let mut sa = Mapping::new();
        sa.insert(k("enabled"), Value::Bool(true));
        sa.insert(k("name"), Value::String(name));

        if let Some(labels) = filter_imported_metadata_labels(metadata.get(&k("labels"))) {
            if let Some(s) = yaml_body(&labels) {
                sa.insert(k("labels"), Value::String(s));
            }
        }
        if let Some(v) = metadata.get(&k("annotations")) {
            if let Some(s) = yaml_body(v) {
                sa.insert(k("annotations"), Value::String(s));
            }
        }

        app.insert(k("serviceAccount"), Value::Mapping(sa));
    }
}

fn map_containers_for_stateless(in_seq: &[Value]) -> Option<Mapping> {
    let mut out = Mapping::new();
    for (idx, raw) in in_seq.iter().enumerate() {
        let c = raw.as_mapping()?;
        let mut name = get_str(c, "name").unwrap_or_default();
        if name.trim().is_empty() {
            name = format!("container-{}", idx + 1);
        }
        let mapped = map_container_for_stateless(c)?;
        out.insert(k(&sanitize_key(&name)), Value::Mapping(mapped));
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn map_container_for_stateless(container: &Mapping) -> Option<Mapping> {
    let image = parse_container_image(container.get(&k("image"))?)?;

    let mut out = Mapping::new();
    out.insert(k("image"), Value::Mapping(image));

    copy_scalar_if_present(&mut out, container, "imagePullPolicy");

    for key in ["ports", "command", "args", "volumeMounts"] {
        if let Some(v) = container.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                out.insert(k(key), Value::String(s));
            }
        }
    }

    let (env_vars, env_residual) = split_container_env(container.get(&k("env")));
    if !env_vars.is_empty() {
        out.insert(k("envVars"), Value::Mapping(env_vars));
    }
    if !env_residual.is_empty() {
        if let Some(s) = yaml_body_clean(&Value::Sequence(env_residual)) {
            out.insert(k("env"), Value::String(s));
        }
    }

    let (shared_cms, shared_secrets, env_from_residual) = split_container_env_from(container.get(&k("envFrom")));
    if !shared_cms.is_empty() {
        out.insert(
            k("sharedEnvConfigMaps"),
            Value::Sequence(shared_cms.into_iter().map(Value::String).collect()),
        );
    }
    if !shared_secrets.is_empty() {
        out.insert(
            k("sharedEnvSecrets"),
            Value::Sequence(shared_secrets.into_iter().map(Value::String).collect()),
        );
    }
    if !env_from_residual.is_empty() {
        if let Some(s) = yaml_body_clean(&Value::Sequence(env_from_residual)) {
            out.insert(k("envFrom"), Value::String(s));
        }
    }

    for key in ["livenessProbe", "readinessProbe", "startupProbe", "securityContext", "lifecycle"] {
        if let Some(v) = container.get(&k(key)) {
            if let Some(s) = yaml_body_clean(v) {
                out.insert(k(key), Value::String(s));
            }
        }
    }

    if let Some(extra) = extract_container_extra_fields(container) {
        if let Some(s) = yaml_body(&Value::Mapping(extra)) {
            out.insert(k("extraFields"), Value::String(s));
        }
    }

    Some(out)
}

fn parse_container_image(v: &Value) -> Option<Mapping> {
    let image = v.as_str()?.trim().to_string();
    if image.is_empty() {
        return None;
    }

    let last_slash = image.rfind('/').unwrap_or(0);
    let last_colon = image.rfind(':');

    let (name, tag) = match last_colon {
        Some(colon_idx) if colon_idx > last_slash => (
            image[..colon_idx].to_string(),
            image[colon_idx + 1..].to_string(),
        ),
        _ => (image, "latest".to_string()),
    };

    let mut out = Mapping::new();
    out.insert(k("name"), Value::String(name));
    out.insert(k("staticTag"), Value::String(tag));
    Some(out)
}

fn split_container_env(v: Option<&Value>) -> (Mapping, Vec<Value>) {
    let mut env_vars = Mapping::new();
    let mut residual = Vec::new();

    let Some(Value::Sequence(items)) = v else {
        return (env_vars, residual);
    };

    for item in items {
        let Some(m) = item.as_mapping() else {
            if let Some(clean) = clean_value(item.clone()) {
                residual.push(clean);
            }
            continue;
        };

        let name = get_str(m, "name").unwrap_or_default();
        if name.trim().is_empty() {
            if let Some(clean) = clean_value(item.clone()) {
                residual.push(clean);
            }
            continue;
        }

        if !m.contains_key(&k("valueFrom")) {
            if let Some(value) = m.get(&k("value")) {
                if !value.is_null() {
                    env_vars.insert(k(name.trim()), value.clone());
                    continue;
                }
            }
        }

        if let Some(clean) = clean_value(item.clone()) {
            residual.push(clean);
        }
    }

    (env_vars, residual)
}

fn split_container_env_from(v: Option<&Value>) -> (Vec<String>, Vec<String>, Vec<Value>) {
    let mut shared_cms: Vec<String> = Vec::new();
    let mut shared_secrets: Vec<String> = Vec::new();
    let mut residual: Vec<Value> = Vec::new();

    let Some(Value::Sequence(items)) = v else {
        return (shared_cms, shared_secrets, residual);
    };

    for item in items {
        let Some(m) = item.as_mapping() else {
            if let Some(clean) = clean_value(item.clone()) {
                residual.push(clean);
            }
            continue;
        };

        if let Some(cm_ref) = get_map(m, "configMapRef") {
            let name = get_str(cm_ref, "name").unwrap_or_default();
            let has_optional = cm_ref.contains_key(&k("optional"));
            let optional_null = cm_ref.get(&k("optional")).map(Value::is_null).unwrap_or(true);
            if !name.trim().is_empty() && (!has_optional || optional_null) && m.len() == 1 {
                shared_cms.push(name.trim().to_string());
                continue;
            }
        }

        if let Some(sec_ref) = get_map(m, "secretRef") {
            let name = get_str(sec_ref, "name").unwrap_or_default();
            let has_optional = sec_ref.contains_key(&k("optional"));
            let optional_null = sec_ref.get(&k("optional")).map(Value::is_null).unwrap_or(true);
            if !name.trim().is_empty() && (!has_optional || optional_null) && m.len() == 1 {
                shared_secrets.push(name.trim().to_string());
                continue;
            }
        }

        if let Some(clean) = clean_value(item.clone()) {
            residual.push(clean);
        }
    }

    (uniq_strings(&shared_cms), uniq_strings(&shared_secrets), residual)
}

fn extract_deployment_extra_spec(spec: &Mapping) -> Option<Mapping> {
    extract_by_allowed(spec, &["replicas", "revisionHistoryLimit", "strategy", "selector", "template"])
}

fn extract_service_extra_spec(spec: &Mapping) -> Option<Mapping> {
    extract_by_allowed(
        spec,
        &[
            "ports",
            "selector",
            "sessionAffinityConfig",
            "type",
            "clusterIP",
            "externalName",
            "externalTrafficPolicy",
            "internalTrafficPolicy",
            "ipFamilyPolicy",
            "loadBalancerClass",
            "loadBalancerIP",
            "sessionAffinity",
            "publishNotReadyAddresses",
            "allocateLoadBalancerNodePorts",
            "healthCheckNodePort",
            "clusterIPs",
            "externalIPs",
            "ipFamilies",
            "loadBalancerSourceRanges",
        ],
    )
}

fn extract_ingress_extra_spec(spec: &Mapping) -> Option<Mapping> {
    extract_by_allowed(spec, &["ingressClassName", "rules", "tls"])
}

fn extract_container_extra_fields(container: &Mapping) -> Option<Mapping> {
    extract_by_allowed(
        container,
        &[
            "name",
            "image",
            "imagePullPolicy",
            "ports",
            "command",
            "args",
            "env",
            "envFrom",
            "livenessProbe",
            "readinessProbe",
            "startupProbe",
            "securityContext",
            "lifecycle",
            "volumeMounts",
        ],
    )
}

fn extract_by_allowed(src: &Mapping, handled: &[&str]) -> Option<Mapping> {
    if src.is_empty() {
        return None;
    }

    let handled_set: HashSet<&str> = handled.iter().copied().collect();
    let mut out = Mapping::new();
    for key in sorted_string_keys(src) {
        if handled_set.contains(key.as_str()) {
            continue;
        }
        if let Some(v) = src.get(&k(&key)).cloned() {
            if let Some(clean) = clean_value(v) {
                if !is_blank_container(&clean) {
                    out.insert(k(&key), clean);
                }
            }
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn clean_value(v: Value) -> Option<Value> {
    match v {
        Value::Null => None,
        Value::Mapping(m) => {
            let mut out = Mapping::new();
            for key in sorted_string_keys(&m) {
                if let Some(raw) = m.get(&k(&key)).cloned() {
                    if let Some(clean) = clean_value(raw) {
                        out.insert(k(&key), clean);
                    }
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(Value::Mapping(out))
            }
        }
        Value::Sequence(seq) => {
            let mut out = Vec::new();
            for item in seq {
                if let Some(clean) = clean_value(item) {
                    out.push(clean);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(Value::Sequence(out))
            }
        }
        other => Some(other),
    }
}

fn merge_maps(dst: &mut Mapping, src: &Mapping) {
    for (k0, v0) in src {
        dst.insert(k0.clone(), v0.clone());
    }
}

fn merge_top_level_values(dst: &mut Mapping, src: &Mapping) {
    for (k0, v0) in src {
        let Some(key_str) = k0.as_str() else {
            dst.insert(k0.clone(), v0.clone());
            continue;
        };

        if key_str != "global" {
            dst.insert(k0.clone(), v0.clone());
            continue;
        }

        let dst_global = get_or_create_child_mapping(dst, "global");
        let Some(src_global) = v0.as_mapping() else {
            continue;
        };

        for (gk, gv) in src_global {
            let Some(gkey) = gk.as_str() else {
                dst_global.insert(gk.clone(), gv.clone());
                continue;
            };
            if gkey != "_includes" {
                dst_global.insert(gk.clone(), gv.clone());
                continue;
            }

            let dst_includes = get_or_create_child_mapping(dst_global, "_includes");
            if let Some(src_includes) = gv.as_mapping() {
                for (ik, iv) in src_includes {
                    dst_includes.insert(ik.clone(), iv.clone());
                }
            }
        }
    }
}

fn push_raw_fallback(raw: &mut Vec<Value>, seen: &mut HashSet<String>, doc: &Value) {
    let id = resource_uid(doc);
    if seen.insert(id) {
        raw.push(doc.clone());
    }
}

fn resource_uid(doc: &Value) -> String {
    let base = if let Some(m) = doc.as_mapping() {
        let api = get_str(m, "apiVersion").unwrap_or_default();
        let kind = get_str(m, "kind").unwrap_or_default();
        let ns = metadata_namespace(m);
        let name = metadata_name(m);
        format!("{}|{}|{}|{}", api, kind, ns, name)
    } else {
        "unknown|unknown|||".to_string()
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_yaml::to_string(doc).unwrap_or_default().hash(&mut hasher);
    format!("{}|{:x}", base, hasher.finish())
}

fn get_or_create_child_mapping<'a>(parent: &'a mut Mapping, key: &str) -> &'a mut Mapping {
    let key_value = k(key);
    let has_mapping = matches!(parent.get(&key_value), Some(Value::Mapping(_)));
    if !has_mapping {
        parent.insert(key_value.clone(), Value::Mapping(Mapping::new()));
    }
    parent
        .get_mut(&key_value)
        .and_then(Value::as_mapping_mut)
        .expect("mapping must exist")
}

fn dedupe_group_key(group: &Mapping, base: &str) -> String {
    if !group.contains_key(&k(base)) {
        return base.to_string();
    }
    let mut idx = 2usize;
    loop {
        let candidate = format!("{}-{}", base, idx);
        if !group.contains_key(&k(&candidate)) {
            return candidate;
        }
        idx += 1;
    }
}

fn insert_group_with_dedupe(group: &mut Mapping, base: &str, app: Mapping) -> String {
    let key = dedupe_group_key(group, base);
    group.insert(k(&key), Value::Mapping(app));
    key
}

fn kind_of(doc: &Mapping) -> String {
    get_str(doc, "kind").unwrap_or_default()
}

fn metadata_name(doc: &Mapping) -> String {
    get_map(doc, "metadata")
        .and_then(|m| get_str(m, "name"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn metadata_namespace(doc: &Mapping) -> String {
    get_map(doc, "metadata")
        .and_then(|m| get_str(m, "namespace"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn normalized_ns(ns: &str) -> String {
    let ns = ns.trim();
    if ns.is_empty() {
        "default".to_string()
    } else {
        ns.to_string()
    }
}

fn builtin_namespace_allowed(ns: &str) -> bool {
    let ns = ns.trim();
    ns.is_empty() || ns == "default"
}

fn filter_imported_metadata_labels(v: Option<&Value>) -> Option<Value> {
    let v = v?;
    let labels = v.as_mapping()?;

    let mut out = Mapping::new();
    for key in sorted_string_keys(labels) {
        if IGNORED_IMPORTED_METADATA_LABEL_KEYS.contains(&key.as_str()) {
            continue;
        }
        if let Some(value) = labels.get(&k(&key)).cloned() {
            out.insert(k(&key), value);
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(Value::Mapping(out))
    }
}

fn escape_tpl_delimiters_value(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(escape_tpl_delimiters_string(s)),
        Value::Mapping(m) => {
            let mut out = Mapping::new();
            for key in sorted_string_keys(m) {
                if let Some(raw) = m.get(&k(&key)) {
                    out.insert(k(&key), escape_tpl_delimiters_value(raw));
                }
            }
            Value::Mapping(out)
        }
        Value::Sequence(seq) => Value::Sequence(seq.iter().map(escape_tpl_delimiters_value).collect()),
        other => other.clone(),
    }
}

fn escape_tpl_delimiters_string(s: &str) -> String {
    if !s.contains("{{") {
        return s.to_string();
    }

    let mut out = String::new();
    let mut i = 0usize;
    while i < s.len() {
        let Some(open_rel) = s[i..].find("{{") else {
            out.push_str(&s[i..]);
            break;
        };
        let open = i + open_rel;
        out.push_str(&s[i..open]);

        let Some(close_rel) = s[open + 2..].find("}}") else {
            out.push_str("{{ \"{{\" }}");
            out.push_str(&s[open + 2..]);
            break;
        };
        let close = open + 2 + close_rel;
        let inner = &s[open + 2..close];
        out.push_str("{{ \"{{\" }}");
        out.push_str(inner);
        out.push_str("{{ \"}}\" }}");
        i = close + 2;
    }
    out
}

fn append_extra_field(app: &mut Mapping, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    let merged = match app.get(&k("extraFields")).and_then(Value::as_str) {
        Some(prev) if !prev.trim().is_empty() => format!("{}\n{}", prev.trim_end(), body),
        _ => body.to_string(),
    };
    app.insert(k("extraFields"), Value::String(merged));
}

fn uniq_strings(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for v in values {
        if seen.insert(v.as_str()) {
            out.push(v.clone());
        }
    }
    out
}

fn copy_scalar_if_present(dst: &mut Mapping, src: &Mapping, key: &str) {
    if let Some(v) = src.get(&k(key)) {
        if !v.is_null() {
            dst.insert(k(key), v.clone());
        }
    }
}

fn indent_yaml_block(s: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    s.trim_end_matches('\n')
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                pad.clone()
            } else {
                format!("{}{}", pad, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn yaml_body(v: &Value) -> Option<String> {
    if is_blank_container(v) {
        return None;
    }
    let s = serde_yaml::to_string(v).ok()?;
    let trimmed = s.trim().trim_start_matches("---").trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn yaml_body_clean(v: &Value) -> Option<String> {
    clean_value(v.clone()).and_then(|x| yaml_body(&x))
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
            let mut out = Mapping::new();
            for key in sorted_string_keys(&m) {
                if let Some(value) = m.get(&k(&key)).cloned() {
                    out.insert(k(&key), sort_rec(value));
                }
            }
            Value::Mapping(out)
        }
        Value::Sequence(seq) => Value::Sequence(seq.into_iter().map(sort_rec).collect()),
        other => other,
    }
}

fn is_blank_container(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Sequence(seq) => seq.is_empty(),
        Value::Mapping(m) => m.is_empty(),
        _ => false,
    }
}

fn sorted_string_keys(m: &Mapping) -> Vec<String> {
    let mut keys: Vec<String> = m
        .keys()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect();
    keys.sort();
    keys
}

fn get_str(m: &Mapping, key: &str) -> Option<String> {
    m.get(&k(key)).and_then(Value::as_str).map(ToString::to_string)
}

fn get_map<'a>(m: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    m.get(&k(key)).and_then(Value::as_mapping)
}

fn get_seq<'a>(m: &'a Mapping, key: &str) -> Option<&'a Vec<Value>> {
    m.get(&k(key)).and_then(Value::as_sequence)
}

fn get_map_mut<'a>(m: &'a mut Mapping, key: &str) -> Option<&'a mut Mapping> {
    m.get_mut(&k(key)).and_then(Value::as_mapping_mut)
}

fn k(s: &str) -> Value {
    Value::String(s.to_string())
}

fn generic_app_key(kind: &str, ns: &str, name: &str) -> String {
    let base = if name.trim().is_empty() {
        "resource".to_string()
    } else {
        name.trim().to_string()
    };
    let prefix = {
        let s = camel_to_kebab(kind);
        if s.is_empty() {
            "resource".to_string()
        } else {
            s
        }
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
    use crate::cli::ImportArgs;
    use serde::Deserialize;

    fn import_args(strategy: &str) -> ImportArgs {
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
            import_strategy: strategy.into(),
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

    fn parse_docs(src: &str) -> Vec<Value> {
        serde_yaml::Deserializer::from_str(src)
            .map(Value::deserialize)
            .collect::<Result<Vec<_>, _>>()
            .expect("docs")
    }

    #[test]
    fn converts_manifest_to_apps_k8s_manifests() {
        let docs = parse_docs(
            r#"
apiVersion: v1
kind: ConfigMap
metadata:
  name: demo
  namespace: default
data:
  a: b
"#,
        );
        let values = build_values(&import_args("raw"), &docs).expect("values");
        let txt = serde_yaml::to_string(&values).expect("yaml");
        assert!(txt.contains("apps-k8s-manifests"));
        assert!(txt.contains("kind: ConfigMap"));
        assert!(txt.contains("name: demo"));
    }

    #[test]
    fn strips_helm_labels_from_metadata() {
        let docs = parse_docs(
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
        );
        let values = build_values(&import_args("raw"), &docs).expect("values");
        let txt = serde_yaml::to_string(&values).expect("yaml");
        assert!(txt.contains("custom: y"));
        assert!(!txt.contains("app.kubernetes.io/name"));
    }

    #[test]
    fn helpers_experimental_maps_known_kinds_into_helper_groups() {
        let docs = parse_docs(
            r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: demo
spec:
  selector:
    matchLabels:
      app: demo
  template:
    metadata:
      labels:
        app: demo
    spec:
      containers:
      - name: app
        image: nginx:1.27
        ports:
        - name: http
          containerPort: 80
---
apiVersion: v1
kind: Service
metadata:
  name: demo
spec:
  selector:
    app: demo
  ports:
  - name: http
    port: 80
    targetPort: 80
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: demo
  namespace: default
data:
  config: "ok"
"#,
        );
        let values = build_values(&import_args("helpers"), &docs).expect("values");
        let root = values.as_mapping().expect("mapping");
        assert!(root.contains_key(&k("apps-stateless")));
        assert!(root.contains_key(&k("apps-services")));
        assert!(root.contains_key(&k("apps-configmaps")));
        assert!(!root.contains_key(&k("apps-k8s-manifests")));
    }

    #[test]
    fn helpers_experimental_attaches_rbac_to_service_account_group() {
        let docs = parse_docs(
            r#"
apiVersion: v1
kind: ServiceAccount
metadata:
  name: demo
  namespace: default
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: demo
  namespace: default
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["get", "list"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: demo
  namespace: default
subjects:
  - kind: ServiceAccount
    name: demo
    namespace: default
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: demo
"#,
        );
        let values = build_values(&import_args("helpers"), &docs).expect("values");
        let root = values.as_mapping().expect("mapping");
        let sa_group = root
            .get(&k("apps-service-accounts"))
            .and_then(Value::as_mapping)
            .expect("apps-service-accounts");
        assert_eq!(sa_group.len(), 1);
        let sa_app = sa_group.values().next().and_then(Value::as_mapping).expect("sa app");
        assert!(sa_app.contains_key(&k("roles")));
        assert!(!root.contains_key(&k("apps-k8s-manifests")));
    }

    #[test]
    fn helpers_experimental_keeps_unknown_kinds_in_raw_fallback() {
        let docs = parse_docs(
            r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: demo
spec:
  selector:
    matchLabels:
      app: demo
  template:
    metadata:
      labels:
        app: demo
    spec:
      containers:
      - name: app
        image: nginx:1.27
---
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: nginx
spec:
  controller: k8s.io/ingress-nginx
"#,
        );
        let values = build_values(&import_args("helpers"), &docs).expect("values");
        let root = values.as_mapping().expect("mapping");
        assert!(root.contains_key(&k("apps-stateless")));
        assert!(root.contains_key(&k("apps-k8s-manifests")));
    }
}
