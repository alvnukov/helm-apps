use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivalenceResult {
    pub equal: bool,
    pub summary: String,
}

pub fn equivalent(source_docs: &[YamlValue], generated_docs: &[YamlValue]) -> EquivalenceResult {
    let src_idx = index_docs(normalize_docs(source_docs));
    let gen_idx = index_docs(normalize_docs(generated_docs));

    let src_keys: Vec<String> = src_idx.keys().cloned().collect();
    let gen_keys: Vec<String> = gen_idx.keys().cloned().collect();

    if src_keys.len() != gen_keys.len() {
        return EquivalenceResult {
            equal: false,
            summary: format!(
                "resource count differs: source={} generated={}",
                src_keys.len(),
                gen_keys.len()
            ),
        };
    }
    for key in &src_keys {
        if !gen_idx.contains_key(key) {
            return EquivalenceResult {
                equal: false,
                summary: format!("missing resource in generated chart: {key}"),
            };
        }
    }
    for key in &gen_keys {
        if !src_idx.contains_key(key) {
            return EquivalenceResult {
                equal: false,
                summary: format!("extra resource in generated chart: {key}"),
            };
        }
    }
    for key in &src_keys {
        let src = src_idx.get(key).expect("src key exists");
        let generated = gen_idx.get(key).expect("generated key exists");
        if let Some((path, av, bv)) = first_diff_path(
            &JsonValue::Object(src.clone()),
            &JsonValue::Object(generated.clone()),
            "",
        ) {
            return EquivalenceResult {
                equal: false,
                summary: format!(
                    "resource content mismatch: {key} at {path} (source={} generated={})",
                    short_json(&av),
                    short_json(&bv)
                ),
            };
        }
    }
    EquivalenceResult {
        equal: true,
        summary: format!("equivalent resources: {}", src_keys.len()),
    }
}

fn normalize_docs(in_docs: &[YamlValue]) -> Vec<JsonMap<String, JsonValue>> {
    let mut out = Vec::new();
    for doc in in_docs {
        let Ok(json) = serde_json::to_value(doc) else {
            continue;
        };
        let JsonValue::Object(m) = json else {
            continue;
        };
        let Some(JsonValue::Object(mut norm)) = normalize_any(&JsonValue::Object(m)) else {
            continue;
        };
        drop_known_library_compare_noise(&mut norm);
        out.push(norm);
    }
    out
}

fn drop_known_library_compare_noise(doc: &mut JsonMap<String, JsonValue>) {
    delete_nested_key_from_map(doc, &["spec", "template", "metadata", "name"]);
    delete_nested_key_from_map(
        doc,
        &[
            "spec",
            "jobTemplate",
            "spec",
            "template",
            "metadata",
            "name",
        ],
    );
}

fn delete_nested_key_from_map(root: &mut JsonMap<String, JsonValue>, path: &[&str]) {
    if path.is_empty() {
        return;
    }
    delete_nested_key_from_value_in_place(root, path);
}

fn delete_nested_key_from_value_in_place(root: &mut JsonMap<String, JsonValue>, path: &[&str]) {
    if path.is_empty() {
        return;
    }
    if path.len() == 1 {
        root.remove(path[0]);
        return;
    }
    let Some(next) = root.get_mut(path[0]) else {
        return;
    };
    if let JsonValue::Object(next_map) = next {
        delete_nested_key_from_value_in_place(next_map, &path[1..]);
    }
}

fn normalize_any(v: &JsonValue) -> Option<JsonValue> {
    match v {
        JsonValue::Object(map) => {
            let mut out = JsonMap::new();
            for (k, vv) in map {
                if k == "status" || k.starts_with("__") {
                    continue;
                }
                if k == "metadata" {
                    if let JsonValue::Object(meta) = vv {
                        let normalized_meta = normalize_metadata(meta);
                        out.insert(k.clone(), JsonValue::Object(normalized_meta));
                        continue;
                    }
                }
                if let Some(nv) = normalize_any(vv) {
                    out.insert(k.clone(), nv);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(JsonValue::Object(out))
            }
        }
        JsonValue::Array(items) => {
            let mut arr = Vec::with_capacity(items.len());
            for item in items {
                arr.push(normalize_any(item).unwrap_or(JsonValue::Null));
            }
            if arr.is_empty() {
                return None;
            }
            if let Some(sorted) = sort_semantic_list(&arr) {
                return Some(JsonValue::Array(sorted));
            }
            Some(JsonValue::Array(arr))
        }
        JsonValue::Null => None,
        _ => Some(v.clone()),
    }
}

fn normalize_metadata(meta: &JsonMap<String, JsonValue>) -> JsonMap<String, JsonValue> {
    let mut out = JsonMap::new();
    for (k, v) in meta {
        if k == "labels" || k == "annotations" {
            continue;
        }
        if k == "namespace" {
            if let JsonValue::String(ns) = v {
                if ns.is_empty() || ns == "default" {
                    continue;
                }
            }
        }
        if let Some(nv) = normalize_any(v) {
            out.insert(k.clone(), nv);
        }
    }
    out
}

fn sort_semantic_list(input: &[JsonValue]) -> Option<Vec<JsonValue>> {
    if input.len() < 2 {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut items = Vec::with_capacity(input.len());
    for item in input {
        let key = semantic_list_item_key(item)?;
        if !seen.insert(key.clone()) {
            return None;
        }
        items.push((key, item.clone()));
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    Some(items.into_iter().map(|(_, v)| v).collect())
}

fn semantic_list_item_key(v: &JsonValue) -> Option<String> {
    let JsonValue::Object(map) = v else {
        return None;
    };
    if map.is_empty() {
        return None;
    }
    if let Some(name) = str_field(map, "name") {
        if let Some(mp) = str_field(map, "mountPath") {
            return Some(format!("name+mountPath:{name}|{mp}"));
        }
        if let Some(port) = int_like_field(map, "containerPort") {
            return Some(format!("name+containerPort:{name}|{port}"));
        }
        if let Some(port) = int_like_field(map, "port") {
            return Some(format!("name+port:{name}|{port}"));
        }
        return Some(format!("name:{name}"));
    }
    if let Some(name) = nested_str_field(map, "configMapRef", "name") {
        return Some(format!("configMapRef:{name}"));
    }
    if let Some(name) = nested_str_field(map, "secretRef", "name") {
        return Some(format!("secretRef:{name}"));
    }
    if let Some(mount_path) = str_field(map, "mountPath") {
        let name = str_field(map, "name").unwrap_or_default();
        return Some(format!("mountPath:{mount_path}|name:{name}"));
    }
    if let Some(port) = int_like_field(map, "containerPort") {
        let name = str_field(map, "name").unwrap_or_default();
        let proto = str_field(map, "protocol").unwrap_or_default();
        return Some(format!("containerPort:{port}|name:{name}|proto:{proto}"));
    }
    if let Some(port) = int_like_field(map, "port") {
        let name = str_field(map, "name").unwrap_or_default();
        let target = str_field(map, "targetPort").unwrap_or_default();
        return Some(format!("port:{port}|name:{name}|targetPort:{target}"));
    }
    if let Some(host) = str_field(map, "host") {
        return Some(format!("host:{host}"));
    }
    if let Some(path) = str_field(map, "path") {
        let path_type = str_field(map, "pathType").unwrap_or_default();
        return Some(format!("path:{path}|pathType:{path_type}"));
    }
    if let Some(JsonValue::Object(meta)) = map.get("metadata") {
        if let Some(name) = str_field(meta, "name") {
            return Some(format!("metadata.name:{name}"));
        }
    }
    None
}

fn str_field(map: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    let JsonValue::String(value) = map.get(key)? else {
        return None;
    };
    if value.trim().is_empty() {
        None
    } else {
        Some(value.clone())
    }
}

fn nested_str_field(map: &JsonMap<String, JsonValue>, key1: &str, key2: &str) -> Option<String> {
    let JsonValue::Object(nested) = map.get(key1)? else {
        return None;
    };
    str_field(nested, key2)
}

fn int_like_field(map: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    let value = map.get(key)?;
    match value {
        JsonValue::Number(n) => {
            if let Some(v) = n.as_i64() {
                Some(v.to_string())
            } else if let Some(v) = n.as_u64() {
                Some(v.to_string())
            } else if let Some(v) = n.as_f64() {
                if v.fract() == 0.0 {
                    Some((v as i64).to_string())
                } else {
                    Some(v.to_string())
                }
            } else {
                Some(n.to_string())
            }
        }
        JsonValue::String(s) if !s.trim().is_empty() => Some(s.clone()),
        JsonValue::String(_) => None,
        other => Some(other.to_string()),
    }
}

fn index_docs(
    docs: Vec<JsonMap<String, JsonValue>>,
) -> BTreeMap<String, JsonMap<String, JsonValue>> {
    let mut out = BTreeMap::new();
    for doc in docs {
        out.insert(doc_key(&doc), doc);
    }
    out
}

fn doc_key(doc: &JsonMap<String, JsonValue>) -> String {
    let api_version = doc
        .get("apiVersion")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let kind = doc
        .get("kind")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let (name, mut namespace) = if let Some(JsonValue::Object(meta)) = doc.get("metadata") {
        (
            meta.get("name")
                .and_then(JsonValue::as_str)
                .unwrap_or_default(),
            meta.get("namespace")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    } else {
        ("", String::new())
    };
    if namespace.is_empty() {
        namespace = "default".to_string();
    }
    format!("{api_version}/{kind}/{namespace}/{name}")
}

fn first_diff_path(
    a: &JsonValue,
    b: &JsonValue,
    path: &str,
) -> Option<(String, JsonValue, JsonValue)> {
    if (is_empty_object(a) && b.is_null()) || (a.is_null() && is_empty_object(b)) {
        return None;
    }
    if (is_empty_array(a) && b.is_null()) || (a.is_null() && is_empty_array(b)) {
        return None;
    }
    match (a, b) {
        (JsonValue::Object(am), JsonValue::Object(bm)) => {
            let keys: BTreeSet<String> = am.keys().cloned().chain(bm.keys().cloned()).collect();
            for key in keys {
                let av = am.get(&key);
                let bv = bm.get(&key);
                if av.is_none() || bv.is_none() {
                    return Some((
                        join_path(path, &key),
                        av.cloned().unwrap_or(JsonValue::Null),
                        bv.cloned().unwrap_or(JsonValue::Null),
                    ));
                }
                if let Some(diff) = first_diff_path(
                    av.expect("key exists"),
                    bv.expect("key exists"),
                    &join_path(path, &key),
                ) {
                    return Some(diff);
                }
            }
            None
        }
        (JsonValue::Array(aa), JsonValue::Array(ba)) => {
            if aa.len() != ba.len() {
                return Some((
                    format!("{}.length", path_or_root(path)),
                    JsonValue::from(aa.len()),
                    JsonValue::from(ba.len()),
                ));
            }
            for (idx, (av, bv)) in aa.iter().zip(ba.iter()).enumerate() {
                let next_path = format!("{}[{idx}]", path_or_root(path));
                if let Some(diff) = first_diff_path(av, bv, &next_path) {
                    return Some(diff);
                }
            }
            None
        }
        _ => {
            if values_equal(a, b) {
                None
            } else {
                Some((path_or_root(path), a.clone(), b.clone()))
            }
        }
    }
}

fn is_empty_object(v: &JsonValue) -> bool {
    matches!(v, JsonValue::Object(m) if m.is_empty())
}

fn is_empty_array(v: &JsonValue) -> bool {
    matches!(v, JsonValue::Array(a) if a.is_empty())
}

fn values_equal(a: &JsonValue, b: &JsonValue) -> bool {
    match (canonical_json(a), canonical_json(b)) {
        (Ok(aj), Ok(bj)) => aj == bj,
        _ => a == b,
    }
}

fn canonical_json(v: &JsonValue) -> Result<String, serde_json::Error> {
    serde_json::to_string(&sort_rec(v))
}

fn sort_rec(v: &JsonValue) -> JsonValue {
    match v {
        JsonValue::Object(map) => {
            let mut out = JsonMap::new();
            let keys: BTreeSet<String> = map.keys().cloned().collect();
            for key in keys {
                let value = map.get(&key).expect("key exists");
                out.insert(key, sort_rec(value));
            }
            JsonValue::Object(out)
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(sort_rec).collect()),
        other => other.clone(),
    }
}

fn join_path(base: &str, key: &str) -> String {
    if base.is_empty() {
        key.to_string()
    } else {
        format!("{base}.{key}")
    }
}

fn path_or_root(path: &str) -> String {
    if path.is_empty() {
        "$".to_string()
    } else {
        path.to_string()
    }
}

fn short_json(v: &JsonValue) -> String {
    let text = serde_json::to_string(v).unwrap_or_else(|_| v.to_string());
    const LIMIT: usize = 120;
    if text.len() > LIMIT {
        format!("{}...", &text[..LIMIT])
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn y(v: JsonValue) -> YamlValue {
        serde_yaml::to_value(v).expect("json->yaml")
    }

    #[test]
    fn equivalent_ignores_metadata_labels_annotations_and_status() {
        let src = vec![y(json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {
                "name": "demo",
                "namespace": "default",
                "labels": {"a":"1"},
                "annotations": {"x":"y"}
            },
            "data": {"k":"v"},
            "status": {"ignored": true}
        }))];
        let generated = vec![y(json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {
                "name": "demo",
                "namespace": "default",
                "labels": {"a":"2"},
                "annotations": {"another":"value"}
            },
            "data": {"k":"v"}
        }))];
        let res = equivalent(&src, &generated);
        assert!(res.equal, "{res:?}");
    }

    #[test]
    fn equivalent_detects_mismatch_with_path() {
        let src = vec![y(json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {"name":"demo","namespace":"default"},
            "data": {"k":"v1"}
        }))];
        let generated = vec![y(json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": {"name":"demo"},
            "data": {"k":"v2"}
        }))];
        let res = equivalent(&src, &generated);
        assert!(!res.equal, "{res:?}");
        assert!(res.summary.contains("data.k"), "{}", res.summary);
    }

    #[test]
    fn equivalent_treats_empty_namespace_as_default() {
        let src = vec![y(json!({
            "apiVersion":"v1",
            "kind":"Service",
            "metadata":{"name":"demo","namespace":"default"},
            "spec":{"type":"ClusterIP"}
        }))];
        let generated = vec![y(json!({
            "apiVersion":"v1",
            "kind":"Service",
            "metadata":{"name":"demo"},
            "spec":{"type":"ClusterIP"}
        }))];
        let res = equivalent(&src, &generated);
        assert!(res.equal, "{res:?}");
    }

    #[test]
    fn equivalent_treats_empty_map_as_null_and_ignores_template_metadata_name() {
        let src = vec![y(json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {"name":"demo","namespace":"default"},
            "spec": {
                "selector": {"matchLabels":{"app":"demo"}},
                "strategy": {"rollingUpdate": {}},
                "template": {
                    "metadata": {},
                    "spec": {"containers":[{"name":"app","image":"nginx"}]}
                }
            }
        }))];
        let generated = vec![y(json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {"name":"demo"},
            "spec": {
                "selector": {"matchLabels":{"app":"demo"}},
                "strategy": {"rollingUpdate": null},
                "template": {
                    "metadata": {"name":"demo"},
                    "spec": {"containers":[{"name":"app","image":"nginx"}]}
                }
            }
        }))];
        let res = equivalent(&src, &generated);
        assert!(res.equal, "{res:?}");
    }

    #[test]
    fn equivalent_ignores_env_order_by_semantic_sort() {
        let src = vec![y(json!({
            "apiVersion":"apps/v1",
            "kind":"Deployment",
            "metadata":{"name":"demo","namespace":"default"},
            "spec":{
                "selector":{"matchLabels":{"app":"demo"}},
                "template":{
                    "metadata":{},
                    "spec":{
                        "containers":[{
                            "name":"app",
                            "image":"nginx",
                            "env":[
                                {"name":"POD_NAME","valueFrom":{"fieldRef":{"fieldPath":"metadata.name"}}},
                                {"name":"LD_PRELOAD","value":"x"}
                            ]
                        }]
                    }
                }
            }
        }))];
        let generated = vec![y(json!({
            "apiVersion":"apps/v1",
            "kind":"Deployment",
            "metadata":{"name":"demo"},
            "spec":{
                "selector":{"matchLabels":{"app":"demo"}},
                "template":{
                    "metadata":{"name":"demo"},
                    "spec":{
                        "containers":[{
                            "name":"app",
                            "image":"nginx",
                            "env":[
                                {"name":"LD_PRELOAD","value":"x"},
                                {"name":"POD_NAME","valueFrom":{"fieldRef":{"fieldPath":"metadata.name"}}}
                            ]
                        }]
                    }
                }
            }
        }))];
        let res = equivalent(&src, &generated);
        assert!(res.equal, "{res:?}");
    }
}
