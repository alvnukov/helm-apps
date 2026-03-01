use serde_yaml::{Mapping, Value};

pub fn normalize_value(v: Value) -> Value {
    match v {
        Value::Mapping(map) => normalize_mapping_merge(map),
        Value::Sequence(seq) => Value::Sequence(seq.into_iter().map(normalize_value).collect()),
        other => other,
    }
}

pub fn normalize_documents(input: &str) -> Result<Vec<Value>, serde_yaml::Error> {
    let docs: Vec<Value> = serde_yaml::Deserializer::from_str(input)
        .map(Value::deserialize)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(docs
        .into_iter()
        .map(normalize_value)
        .filter(|v| !v.is_null())
        .collect())
}

fn normalize_mapping_merge(map: Mapping) -> Value {
    let mut out = Mapping::new();
    if let Some(merge_source) = map.get(Value::String("<<".to_string())).cloned() {
        apply_merge_source(&mut out, merge_source);
    }
    for (k, v) in map {
        if matches!(&k, Value::String(s) if s == "<<") {
            continue;
        }
        out.insert(k, normalize_value(v));
    }
    Value::Mapping(out)
}

fn apply_merge_source(target: &mut Mapping, source: Value) {
    match normalize_value(source) {
        Value::Mapping(m) => merge_mapping_into(target, m),
        Value::Sequence(seq) => {
            for item in seq {
                if let Value::Mapping(m) = normalize_value(item) {
                    merge_mapping_into(target, m);
                }
            }
        }
        _ => {}
    }
}

fn merge_mapping_into(target: &mut Mapping, source: Mapping) {
    for (k, v) in source {
        target.entry(k).or_insert(v);
    }
}

use serde::Deserialize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_inline_merge_map() {
        let v: Value = serde_yaml::from_str(
            r#"
obj:
  <<: { foo: 123, bar: 456 }
  baz: 999
"#,
        )
        .expect("parse");
        let n = normalize_value(v);
        let j = serde_json::to_value(n).expect("json");
        assert_eq!(j["obj"]["foo"], 123);
        assert_eq!(j["obj"]["bar"], 456);
        assert_eq!(j["obj"]["baz"], 999);
        assert!(j["obj"].get("<<").is_none());
        let line = serde_json::to_string(&j["obj"]).expect("json");
        assert_eq!(line, r#"{"foo":123,"bar":456,"baz":999}"#);
    }

    #[test]
    fn merge_sequence_earlier_source_overrides_later_source() {
        let v: Value = serde_yaml::from_str(
            r#"
base1: &base1
  x: first
base2: &base2
  x: second
obj:
  <<: [*base1, *base2]
"#,
        )
        .expect("parse");
        let n = normalize_value(v);
        let j = serde_json::to_value(n).expect("json");
        assert_eq!(j["obj"]["x"], "first");
    }

    #[test]
    fn explicit_key_overrides_merged_value() {
        let v: Value = serde_yaml::from_str(
            r#"
base: &base
  image: nginx
  replicas: 2
obj:
  <<: *base
  replicas: 3
"#,
        )
        .expect("parse");
        let n = normalize_value(v);
        let j = serde_json::to_value(n).expect("json");
        assert_eq!(j["obj"]["image"], "nginx");
        assert_eq!(j["obj"]["replicas"], 3);
    }

}
