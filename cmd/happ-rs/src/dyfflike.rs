use serde_yaml::Value;

#[derive(Clone, Copy, Debug, Default)]
pub struct DiffOptions {
    pub ignore_order_changes: bool,
    pub ignore_whitespace_change: bool,
}

pub fn between_yaml(from: &str, to: &str, opts: DiffOptions) -> Result<String, serde_yaml::Error> {
    let a = crate::yamlmerge::normalize_documents(from)?;
    let b = crate::yamlmerge::normalize_documents(to)?;
    Ok(between_docs(&a, &b, opts))
}

pub fn between_docs(from: &[Value], to: &[Value], opts: DiffOptions) -> String {
    let mut out = Vec::new();
    let max = from.len().max(to.len());
    for i in 0..max {
        match (from.get(i), to.get(i)) {
            (Some(a), Some(b)) => diff_value(a, b, format!("doc[{i}]"), opts, &mut out),
            (Some(_), None) => out.push(format!("removed: doc[{i}]")),
            (None, Some(_)) => out.push(format!("added: doc[{i}]")),
            (None, None) => {}
        }
    }
    out.join("\n")
}

fn diff_value(a: &Value, b: &Value, path: String, opts: DiffOptions, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Mapping(ma), Value::Mapping(mb)) => {
            let mut keys = std::collections::BTreeSet::new();
            for k in ma.keys() {
                keys.insert(key_string(k));
            }
            for k in mb.keys() {
                keys.insert(key_string(k));
            }
            for k in keys {
                let kv = Value::String(k.clone());
                let kp = format!("{}.{}", path, k);
                match (ma.get(&kv), mb.get(&kv)) {
                    (Some(va), Some(vb)) => diff_value(va, vb, kp, opts, out),
                    (Some(_), None) => out.push(format!("removed: {kp}")),
                    (None, Some(_)) => out.push(format!("added: {kp}")),
                    _ => {}
                }
            }
        }
        (Value::Sequence(sa), Value::Sequence(sb)) => {
            if opts.ignore_order_changes {
                let mut ca: Vec<String> = sa.iter().map(canonical).collect();
                let mut cb: Vec<String> = sb.iter().map(canonical).collect();
                ca.sort();
                cb.sort();
                if ca != cb {
                    out.push(format!("changed: {path}"));
                }
                return;
            }
            let max = sa.len().max(sb.len());
            for i in 0..max {
                let kp = format!("{}[{i}]", path);
                match (sa.get(i), sb.get(i)) {
                    (Some(va), Some(vb)) => diff_value(va, vb, kp, opts, out),
                    (Some(_), None) => out.push(format!("removed: {kp}")),
                    (None, Some(_)) => out.push(format!("added: {kp}")),
                    _ => {}
                }
            }
        }
        _ => {
            if scalar_eq(a, b, opts.ignore_whitespace_change) {
                return;
            }
            out.push(format!("changed: {path}"));
        }
    }
}

fn scalar_eq(a: &Value, b: &Value, trim_ws: bool) -> bool {
    match (a, b) {
        (Value::String(sa), Value::String(sb)) if trim_ws => sa.trim() == sb.trim(),
        _ => a == b,
    }
}

fn canonical(v: &Value) -> String {
    let json = serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    let sorted = canonicalize_json(json);
    serde_json::to_string(&sorted).unwrap_or_default()
}

fn key_string(v: &Value) -> String {
    v.as_str().map(ToString::to_string).unwrap_or_else(|| canonical(v))
}

fn canonicalize_json(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::with_capacity(keys.len());
            for k in keys {
                if let Some(val) = map.get(&k).cloned() {
                    out.insert(k, canonicalize_json(val));
                }
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(canonicalize_json).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_changed_field() {
        let a = "a: 1\nb: 2\n";
        let b = "a: 1\nb: 3\n";
        let diff = between_yaml(a, b, DiffOptions::default()).expect("diff");
        assert!(diff.contains("changed: doc[0].b"));
    }

    #[test]
    fn supports_ignore_order_for_lists() {
        let a = "a:\n  - 1\n  - 2\n";
        let b = "a:\n  - 2\n  - 1\n";
        let d1 = between_yaml(a, b, DiffOptions::default()).expect("diff");
        assert!(d1.contains("changed: doc[0].a"));

        let d2 = between_yaml(
            a,
            b,
            DiffOptions {
                ignore_order_changes: true,
                ..DiffOptions::default()
            },
        )
        .expect("diff");
        assert!(d2.is_empty());
    }

    #[test]
    fn supports_ignore_whitespace_for_strings() {
        let a = "a: \" hello \"\n";
        let b = "a: \"hello\"\n";
        let d1 = between_yaml(a, b, DiffOptions::default()).expect("diff");
        assert!(!d1.is_empty());
        let d2 = between_yaml(
            a,
            b,
            DiffOptions {
                ignore_whitespace_change: true,
                ..DiffOptions::default()
            },
        )
        .expect("diff");
        assert!(d2.is_empty());
    }

    #[test]
    fn merge_key_and_expanded_object_are_equal() {
        let a = r#"
base: &base
  dummy: 42
obj:
  <<: { foo: 123, bar: 456 }
  baz: 999
"#;
        let b = r#"
base:
  dummy: 42
obj:
  foo: 123
  bar: 456
  baz: 999
"#;
        let diff = between_yaml(a, b, DiffOptions::default()).expect("diff");
        assert!(diff.is_empty(), "unexpected diff: {diff}");
    }

    #[test]
    fn ignore_order_mode_is_stable_for_maps_with_different_key_order() {
        let a = "list:\n  - {x: 1, y: 2}\n";
        let b = "list:\n  - {y: 2, x: 1}\n";
        let diff = between_yaml(
            a,
            b,
            DiffOptions {
                ignore_order_changes: true,
                ..DiffOptions::default()
            },
        )
        .expect("diff");
        assert!(diff.is_empty(), "unexpected diff: {diff}");
    }
}
