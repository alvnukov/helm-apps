use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use std::ffi::CStr;
use std::slice;
use unsafe_libyaml::{
    yaml_event_delete, yaml_event_t, yaml_parser_delete, yaml_parser_initialize, yaml_parser_parse,
    yaml_parser_set_input_string, yaml_parser_t, yaml_scalar_style_t, YAML_ALIAS_EVENT,
    YAML_DOCUMENT_END_EVENT, YAML_DOCUMENT_START_EVENT, YAML_MAPPING_END_EVENT,
    YAML_MAPPING_START_EVENT, YAML_PLAIN_SCALAR_STYLE, YAML_SCALAR_EVENT, YAML_SEQUENCE_END_EVENT,
    YAML_SEQUENCE_START_EVENT, YAML_STREAM_END_EVENT,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PathSegment {
    Key(String),
    Index(usize),
}

type Path = Vec<PathSegment>;

#[derive(Debug, Default, Clone)]
struct MergeStyleHints {
    plain_merge_paths: HashSet<Path>,
    nonplain_merge_paths: HashSet<Path>,
}

enum Frame {
    Mapping {
        path: Path,
        expecting_key: bool,
        current_key: Option<String>,
    },
    Sequence {
        path: Path,
        next_index: usize,
    },
}

pub fn normalize_value(v: Value) -> Value {
    normalize_value_with_hints(v, None, &mut Vec::new())
}

#[allow(dead_code)]
pub fn normalize_value_from_source(input: &str, v: Value) -> Value {
    let hints = collect_merge_style_hints(input).ok();
    normalize_value_with_hints(v, hints.as_ref().and_then(|v| v.first()), &mut Vec::new())
}

pub fn normalize_documents(input: &str) -> Result<Vec<Value>, serde_yaml::Error> {
    let docs: Vec<Value> = serde_yaml::Deserializer::from_str(input)
        .map(Value::deserialize)
        .collect::<Result<Vec<_>, _>>()?;
    let hints = collect_merge_style_hints(input).ok();
    Ok(docs
        .into_iter()
        .enumerate()
        .map(|(i, doc)| {
            normalize_value_with_hints(doc, hints.as_ref().and_then(|v| v.get(i)), &mut Vec::new())
        })
        .filter(|v| !v.is_null())
        .collect())
}

fn normalize_value_with_hints(v: Value, hints: Option<&MergeStyleHints>, path: &mut Path) -> Value {
    match v {
        Value::Mapping(map) => normalize_mapping_merge(map, hints, path),
        Value::Sequence(seq) => {
            let mut out = Vec::with_capacity(seq.len());
            for (i, item) in seq.into_iter().enumerate() {
                path.push(PathSegment::Index(i));
                out.push(normalize_value_with_hints(item, hints, path));
                path.pop();
            }
            Value::Sequence(out)
        }
        other => other,
    }
}

fn normalize_mapping_merge(
    map: Mapping,
    hints: Option<&MergeStyleHints>,
    path: &mut Path,
) -> Value {
    let merge_key = Value::String("<<".to_string());
    let has_merge_key = map.contains_key(&merge_key);
    let should_merge = has_merge_key && should_apply_merge(hints, path);

    let mut out = Mapping::new();
    if should_merge {
        if let Some(merge_source) = map.get(&merge_key).cloned() {
            apply_merge_source(&mut out, merge_source);
        }
    }

    for (k, v) in map {
        if matches!(&k, Value::String(s) if s == "<<") && should_merge {
            continue;
        }
        let key_seg = mapping_key_segment(&k);
        path.push(PathSegment::Key(key_seg));
        let nv = normalize_value_with_hints(v, hints, path);
        path.pop();
        out.insert(k, nv);
    }
    Value::Mapping(out)
}

fn should_apply_merge(hints: Option<&MergeStyleHints>, path: &Path) -> bool {
    let Some(hints) = hints else {
        return true;
    };
    if hints.plain_merge_paths.contains(path) {
        return true;
    }
    if hints.nonplain_merge_paths.contains(path) {
        return false;
    }
    true
}

fn mapping_key_segment(k: &Value) -> String {
    if let Some(s) = k.as_str() {
        return s.to_string();
    }
    serde_yaml::to_string(k)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "<non-string-key>".to_string())
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

fn collect_merge_style_hints(input: &str) -> Result<Vec<MergeStyleHints>, String> {
    unsafe {
        let mut parser = std::mem::MaybeUninit::<yaml_parser_t>::zeroed().assume_init();
        if !yaml_parser_initialize(&mut parser).ok {
            return Err("yaml parser init failed".to_string());
        }
        yaml_parser_set_input_string(&mut parser, input.as_ptr(), input.len() as u64);

        let mut all_hints: Vec<MergeStyleHints> = Vec::new();
        let mut current_doc: Option<usize> = None;
        let mut stack: Vec<Frame> = Vec::new();

        loop {
            let mut event = std::mem::MaybeUninit::<yaml_event_t>::zeroed().assume_init();
            if !yaml_parser_parse(&mut parser, &mut event).ok {
                let err = parser_error(&parser);
                yaml_parser_delete(&mut parser);
                return Err(err);
            }
            let t = event.type_;

            match t {
                YAML_DOCUMENT_START_EVENT => {
                    all_hints.push(MergeStyleHints::default());
                    current_doc = Some(all_hints.len() - 1);
                    stack.clear();
                }
                YAML_DOCUMENT_END_EVENT => {
                    stack.clear();
                    current_doc = None;
                }
                YAML_MAPPING_START_EVENT => {
                    let child_path = begin_container(&mut stack)?;
                    stack.push(Frame::Mapping {
                        path: child_path,
                        expecting_key: true,
                        current_key: None,
                    });
                }
                YAML_MAPPING_END_EVENT => {
                    stack.pop();
                }
                YAML_SEQUENCE_START_EVENT => {
                    let child_path = begin_container(&mut stack)?;
                    stack.push(Frame::Sequence {
                        path: child_path,
                        next_index: 0,
                    });
                }
                YAML_SEQUENCE_END_EVENT => {
                    stack.pop();
                }
                YAML_SCALAR_EVENT => {
                    if let Some(Frame::Mapping {
                        path,
                        expecting_key,
                        current_key,
                    }) = stack.last_mut()
                    {
                        if *expecting_key {
                            let k = scalar_string(&event);
                            if k == "<<" {
                                let style: yaml_scalar_style_t = event.data.scalar.style;
                                if let Some(doc_idx) = current_doc {
                                    if style == YAML_PLAIN_SCALAR_STYLE {
                                        all_hints[doc_idx].plain_merge_paths.insert(path.clone());
                                    } else {
                                        all_hints[doc_idx]
                                            .nonplain_merge_paths
                                            .insert(path.clone());
                                    }
                                }
                            }
                            *current_key = Some(k);
                            *expecting_key = false;
                            yaml_event_delete(&mut event);
                            if t == YAML_STREAM_END_EVENT {
                                break;
                            }
                            continue;
                        }
                    }
                    consume_scalar_or_alias_value(&mut stack);
                }
                YAML_ALIAS_EVENT => {
                    if let Some(Frame::Mapping {
                        expecting_key,
                        current_key,
                        ..
                    }) = stack.last_mut()
                    {
                        if *expecting_key {
                            *expecting_key = false;
                            *current_key = None;
                            yaml_event_delete(&mut event);
                            if t == YAML_STREAM_END_EVENT {
                                break;
                            }
                            continue;
                        }
                    }
                    consume_scalar_or_alias_value(&mut stack);
                }
                _ => {}
            }

            yaml_event_delete(&mut event);
            if t == YAML_STREAM_END_EVENT {
                break;
            }
        }

        yaml_parser_delete(&mut parser);
        Ok(all_hints)
    }
}

fn begin_container(stack: &mut [Frame]) -> Result<Path, String> {
    let Some(parent) = stack.last_mut() else {
        return Ok(Vec::new());
    };
    match parent {
        Frame::Sequence { path, next_index } => {
            let idx = *next_index;
            *next_index += 1;
            let mut out = path.clone();
            out.push(PathSegment::Index(idx));
            Ok(out)
        }
        Frame::Mapping {
            path,
            expecting_key,
            current_key,
        } => {
            if *expecting_key {
                *expecting_key = false;
                *current_key = None;
                let mut out = path.clone();
                out.push(PathSegment::Key("<complex-key>".to_string()));
                return Ok(out);
            }
            let key = current_key
                .take()
                .unwrap_or_else(|| "<complex-key>".to_string());
            *expecting_key = true;
            let mut out = path.clone();
            out.push(PathSegment::Key(key));
            Ok(out)
        }
    }
}

fn consume_scalar_or_alias_value(stack: &mut [Frame]) {
    let Some(parent) = stack.last_mut() else {
        return;
    };
    match parent {
        Frame::Sequence { next_index, .. } => {
            *next_index += 1;
        }
        Frame::Mapping {
            expecting_key,
            current_key,
            ..
        } => {
            if !*expecting_key {
                *expecting_key = true;
                *current_key = None;
            }
        }
    }
}

unsafe fn scalar_string(event: &yaml_event_t) -> String {
    let ptr = event.data.scalar.value.cast::<u8>();
    let len = event.data.scalar.length as usize;
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let bytes = slice::from_raw_parts(ptr, len);
    String::from_utf8_lossy(bytes).into_owned()
}

unsafe fn parser_error(parser: &yaml_parser_t) -> String {
    let problem = if parser.problem.is_null() {
        "yaml parse error".to_string()
    } else {
        CStr::from_ptr(parser.problem.cast::<std::ffi::c_char>())
            .to_string_lossy()
            .into_owned()
    };
    format!(
        "{} at line {} column {}",
        problem,
        parser.problem_mark.line + 1,
        parser.problem_mark.column + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_inline_merge_map() {
        let src = r#"
obj:
  <<: { foo: 123, bar: 456 }
  baz: 999
"#;
        let v: Value = serde_yaml::from_str(src).expect("parse");
        let n = normalize_value_from_source(src, v);
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
        let src = r#"
base1: &base1
  x: first
base2: &base2
  x: second
obj:
  <<: [*base1, *base2]
"#;
        let v: Value = serde_yaml::from_str(src).expect("parse");
        let n = normalize_value_from_source(src, v);
        let j = serde_json::to_value(n).expect("json");
        assert_eq!(j["obj"]["x"], "first");
    }

    #[test]
    fn explicit_key_overrides_merged_value() {
        let src = r#"
base: &base
  image: nginx
  replicas: 2
obj:
  <<: *base
  replicas: 3
"#;
        let v: Value = serde_yaml::from_str(src).expect("parse");
        let n = normalize_value_from_source(src, v);
        let j = serde_json::to_value(n).expect("json");
        assert_eq!(j["obj"]["image"], "nginx");
        assert_eq!(j["obj"]["replicas"], 3);
    }

    #[test]
    fn quoted_merge_key_is_treated_as_regular_key() {
        let src = r#"
obj:
  "<<": { foo: 1 }
  baz: 2
"#;
        let v: Value = serde_yaml::from_str(src).expect("parse");
        let n = normalize_value_from_source(src, v);
        let j = serde_json::to_value(n).expect("json");
        assert_eq!(j["obj"]["<<"]["foo"], 1);
        assert_eq!(j["obj"]["baz"], 2);
    }

    #[test]
    fn merge_precedence_property_like_regression() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..200 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let a = (seed % 1000) as i64;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let b = (seed % 1000) as i64;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let c = (seed % 1000) as i64;

            let src = format!(
                r#"
base1: &base1
  x: {a}
  y: {b}
base2: &base2
  y: {c}
  z: {a}
obj:
  <<: [*base1, *base2]
  z: {b}
"#
            );
            let v: Value = serde_yaml::from_str(&src).expect("parse");
            let n = normalize_value_from_source(&src, v);
            let j = serde_json::to_value(n).expect("json");
            assert_eq!(j["obj"]["x"], a, "x mismatch for source:\n{src}");
            assert_eq!(j["obj"]["y"], b, "y precedence mismatch for source:\n{src}");
            assert_eq!(
                j["obj"]["z"], b,
                "local override mismatch for source:\n{src}"
            );
        }
    }
}
