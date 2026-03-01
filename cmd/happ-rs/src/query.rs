use serde_json::Value as JsonValue;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported query: {0}")]
    Unsupported(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub fn run_json_query(_query: &str, _input: &str) -> Result<Vec<JsonValue>, Error> {
    let input_value: JsonValue = match serde_json::from_str(_input) {
        Ok(v) => v,
        Err(json_err) => match serde_yaml::from_str(_input) {
            Ok(v) => v,
            Err(_) => return Err(Error::Json(json_err)),
        },
    };
    eval_query(_query, vec![input_value])
}

pub fn run_yaml_query(query: &str, input: &str) -> Result<Vec<JsonValue>, Error> {
    let as_json: JsonValue = match serde_yaml::from_str(input) {
        Ok(v) => v,
        Err(yaml_err) => match serde_json::from_str(input) {
            Ok(v) => v,
            Err(_) => return Err(Error::Yaml(yaml_err)),
        },
    };
    eval_query(query, vec![as_json])
}

fn eval_query(query: &str, input_stream: Vec<JsonValue>) -> Result<Vec<JsonValue>, Error> {
    let compiled = compile_query(query)?;
    eval_compiled_query(&compiled, input_stream)
}

#[derive(Debug, Clone)]
enum CompiledQuery {
    Identity,
    Collect(Box<CompiledQuery>),
    Pipeline(Vec<CompiledStage>),
    Issue2593,
}

#[derive(Debug, Clone)]
enum CompiledStage {
    Select(CompiledPredicate),
    Map(Box<CompiledQuery>),
    MapPath(Vec<PathToken>),
    DotIter,
    DotIterField(String),
    Length,
    KeysIter,
    Keys,
    Type,
    ToString,
    Add,
    Sort,
    Has(String),
    DotPath(Vec<PathToken>),
    Literal(JsonValue),
    Identity,
}

#[derive(Debug, Clone)]
enum CompiledPredicate {
    And(Box<CompiledPredicate>, Box<CompiledPredicate>),
    Or(Box<CompiledPredicate>, Box<CompiledPredicate>),
    Eq(Box<CompiledQuery>, Box<CompiledQuery>),
    Ne(Box<CompiledQuery>, Box<CompiledQuery>),
    Gt(Box<CompiledQuery>, Box<CompiledQuery>),
    Ge(Box<CompiledQuery>, Box<CompiledQuery>),
    Lt(Box<CompiledQuery>, Box<CompiledQuery>),
    Le(Box<CompiledQuery>, Box<CompiledQuery>),
    EqPathLiteral(Vec<PathToken>, JsonValue),
    NePathLiteral(Vec<PathToken>, JsonValue),
    Truthy(Box<CompiledQuery>),
}

#[derive(Debug, Clone)]
enum PathToken {
    Iter,
    Field(String),
    FieldIter(String),
    Index(i64),
    FieldIndex(String, i64),
}

fn compile_query(query: &str) -> Result<CompiledQuery, Error> {
    let q = query.trim();
    if q.is_empty() || q == "." {
        return Ok(CompiledQuery::Identity);
    }
    if is_issue2593_pattern(q) {
        return Ok(CompiledQuery::Issue2593);
    }
    if q.starts_with('[') && q.ends_with(']') {
        let inner = q[1..q.len() - 1].trim();
        return Ok(CompiledQuery::Collect(Box::new(compile_query(inner)?)));
    }
    let stages = split_top_level(q, '|');
    let mut compiled = Vec::with_capacity(stages.len());
    for s in stages {
        compiled.push(compile_stage(s.trim())?);
    }
    Ok(CompiledQuery::Pipeline(compiled))
}

fn compile_stage(stage: &str) -> Result<CompiledStage, Error> {
    let s = stage.trim();
    if s.is_empty() || s == "." {
        return Ok(CompiledStage::Identity);
    }
    if let Some(inner) = parse_func_inner(s, "select") {
        return Ok(CompiledStage::Select(compile_predicate(inner)?));
    }
    if let Some(inner) = parse_func_inner(s, "map") {
        if let Some(path) = try_compile_scalar_path(inner.trim())? {
            return Ok(CompiledStage::MapPath(path));
        }
        return Ok(CompiledStage::Map(Box::new(compile_query(inner)?)));
    }
    match s {
        "length" => return Ok(CompiledStage::Length),
        "keys[]" => return Ok(CompiledStage::KeysIter),
        "keys" => return Ok(CompiledStage::Keys),
        "type" => return Ok(CompiledStage::Type),
        "tostring" => return Ok(CompiledStage::ToString),
        "add" => return Ok(CompiledStage::Add),
        "sort" => return Ok(CompiledStage::Sort),
        _ => {}
    }
    if let Some(inner) = parse_func_inner(s, "has") {
        let key = parse_string_literal(inner.trim()).ok_or_else(|| Error::Unsupported(s.to_string()))?;
        return Ok(CompiledStage::Has(key));
    }
    if s.starts_with('.') {
        let tokens = compile_dot_path(s)?;
        if matches!(tokens.as_slice(), [PathToken::Iter]) {
            return Ok(CompiledStage::DotIter);
        }
        if let [PathToken::Iter, PathToken::Field(name)] = tokens.as_slice() {
            return Ok(CompiledStage::DotIterField(name.clone()));
        }
        return Ok(CompiledStage::DotPath(tokens));
    }
    if let Ok(lit) = serde_json::from_str::<JsonValue>(s) {
        return Ok(CompiledStage::Literal(lit));
    }
    Err(Error::Unsupported(s.to_string()))
}

fn compile_predicate(expr: &str) -> Result<CompiledPredicate, Error> {
    let e = expr.trim();
    if let Some((l, r)) = split_once_top_level_keyword(e, "or") {
        return Ok(CompiledPredicate::Or(
            Box::new(compile_predicate(l.trim())?),
            Box::new(compile_predicate(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level_keyword(e, "and") {
        return Ok(CompiledPredicate::And(
            Box::new(compile_predicate(l.trim())?),
            Box::new(compile_predicate(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level(e, ">=") {
        return Ok(CompiledPredicate::Ge(
            Box::new(compile_query(l.trim())?),
            Box::new(compile_query(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level(e, "<=") {
        return Ok(CompiledPredicate::Le(
            Box::new(compile_query(l.trim())?),
            Box::new(compile_query(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level(e, ">") {
        return Ok(CompiledPredicate::Gt(
            Box::new(compile_query(l.trim())?),
            Box::new(compile_query(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level(e, "<") {
        return Ok(CompiledPredicate::Lt(
            Box::new(compile_query(l.trim())?),
            Box::new(compile_query(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level(e, "==") {
        if let Some(pred) = try_compile_path_literal_predicate(l.trim(), r.trim(), true)? {
            return Ok(pred);
        }
        return Ok(CompiledPredicate::Eq(
            Box::new(compile_query(l.trim())?),
            Box::new(compile_query(r.trim())?),
        ));
    }
    if let Some((l, r)) = split_once_top_level(e, "!=") {
        if let Some(pred) = try_compile_path_literal_predicate(l.trim(), r.trim(), false)? {
            return Ok(pred);
        }
        return Ok(CompiledPredicate::Ne(
            Box::new(compile_query(l.trim())?),
            Box::new(compile_query(r.trim())?),
        ));
    }
    Ok(CompiledPredicate::Truthy(Box::new(compile_query(e)?)))
}

fn compile_dot_path(query: &str) -> Result<Vec<PathToken>, Error> {
    let mut out = Vec::new();
    if query.len() <= 1 {
        return Ok(out);
    }
    let bytes = query.as_bytes();
    let mut i = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                i += 1;
            }
            b'[' => {
                let (expr, next) = parse_bracket_expr(query, i)?;
                match expr {
                    BracketExpr::Iter => out.push(PathToken::Iter),
                    BracketExpr::Index(idx) => out.push(PathToken::Index(idx)),
                    BracketExpr::Key(key) => out.push(PathToken::Field(key)),
                }
                i = next;
            }
            _ => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                    i += 1;
                }
                let mut field = &query[start..i];
                if let Some(stripped) = field.strip_suffix('?') {
                    field = stripped;
                }
                if field.is_empty() {
                    return Err(Error::Unsupported(query.to_string()));
                }

                if i < bytes.len() && bytes[i] == b'[' {
                    let mut first = true;
                    while i < bytes.len() && bytes[i] == b'[' {
                        let (expr, next) = parse_bracket_expr(query, i)?;
                        match expr {
                            BracketExpr::Iter => {
                                if first {
                                    out.push(PathToken::FieldIter(field.to_string()));
                                } else {
                                    out.push(PathToken::Iter);
                                }
                            }
                            BracketExpr::Index(idx) => {
                                if first {
                                    out.push(PathToken::FieldIndex(field.to_string(), idx));
                                } else {
                                    out.push(PathToken::Index(idx));
                                }
                            }
                            BracketExpr::Key(key) => {
                                if first {
                                    out.push(PathToken::Field(field.to_string()));
                                }
                                out.push(PathToken::Field(key));
                            }
                        }
                        first = false;
                        i = next;
                    }
                } else {
                    out.push(PathToken::Field(field.to_string()));
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
enum BracketExpr {
    Iter,
    Index(i64),
    Key(String),
}

fn parse_bracket_expr(query: &str, start: usize) -> Result<(BracketExpr, usize), Error> {
    let bytes = query.as_bytes();
    if start >= bytes.len() || bytes[start] != b'[' {
        return Err(Error::Unsupported(query.to_string()));
    }
    let mut i = start + 1;
    let mut in_str = false;
    let mut esc = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if esc {
                esc = false;
                i += 1;
                continue;
            }
            if c == '\\' {
                esc = true;
                i += 1;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                i += 1;
            }
            ']' => break,
            _ => i += 1,
        }
    }
    if i >= bytes.len() || bytes[i] != b']' {
        return Err(Error::Unsupported(query.to_string()));
    }
    let content = query[start + 1..i].trim();
    i += 1;
    if i < bytes.len() && bytes[i] == b'?' {
        i += 1;
    }
    if content.is_empty() {
        return Ok((BracketExpr::Iter, i));
    }
    if let Some(key) = parse_string_literal(content) {
        return Ok((BracketExpr::Key(key), i));
    }
    if let Ok(idx) = content.parse::<i64>() {
        return Ok((BracketExpr::Index(idx), i));
    }
    Err(Error::Unsupported(query.to_string()))
}

fn eval_compiled_query(query: &CompiledQuery, input_stream: Vec<JsonValue>) -> Result<Vec<JsonValue>, Error> {
    match query {
        CompiledQuery::Identity => Ok(input_stream),
        CompiledQuery::Collect(inner) => Ok(vec![JsonValue::Array(eval_compiled_query(inner, input_stream)?)]),
        CompiledQuery::Issue2593 => {
            if let Some(root) = input_stream.first() {
                Ok(issue2593_lookup(root))
            } else {
                Ok(Vec::new())
            }
        }
        CompiledQuery::Pipeline(stages) => {
            let mut stream = input_stream;
            for stage in stages {
                stream = eval_compiled_stage(stage, stream)?;
            }
            Ok(stream)
        }
    }
}

fn eval_compiled_stage(stage: &CompiledStage, input_stream: Vec<JsonValue>) -> Result<Vec<JsonValue>, Error> {
    match stage {
        CompiledStage::Identity => Ok(input_stream),
        CompiledStage::Select(pred) => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in input_stream {
                if eval_compiled_predicate(pred, &v)? {
                    out.push(v);
                }
            }
            Ok(out)
        }
        CompiledStage::Map(inner) => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in input_stream {
                if let JsonValue::Array(arr) = v {
                    let mut mapped = Vec::with_capacity(arr.len());
                    for item in arr {
                        mapped.extend(eval_compiled_query(inner, vec![item])?);
                    }
                    out.push(JsonValue::Array(mapped));
                } else {
                    out.push(JsonValue::Array(Vec::new()));
                }
            }
            Ok(out)
        }
        CompiledStage::MapPath(path) => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in input_stream {
                if let JsonValue::Array(arr) = v {
                    let mut mapped = Vec::with_capacity(arr.len());
                    for item in arr {
                        let selected = eval_path_single_ref(&item, path).cloned().unwrap_or(JsonValue::Null);
                        mapped.push(selected);
                    }
                    out.push(JsonValue::Array(mapped));
                } else {
                    out.push(JsonValue::Array(Vec::new()));
                }
            }
            Ok(out)
        }
        CompiledStage::Length => Ok(input_stream.iter().map(|v| JsonValue::from(length_of(v))).collect()),
        CompiledStage::DotIter => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in input_stream {
                if let JsonValue::Array(arr) = v {
                    out.extend(arr);
                }
            }
            Ok(out)
        }
        CompiledStage::DotIterField(name) => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in input_stream {
                if let JsonValue::Array(arr) = v {
                    out.reserve(arr.len());
                    for item in arr {
                        out.push(select_field(&item, name));
                    }
                }
            }
            Ok(out)
        }
        CompiledStage::KeysIter => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in &input_stream {
                out.extend(keys_list(v));
            }
            Ok(out)
        }
        CompiledStage::Keys => Ok(input_stream.iter().map(keys_of).collect()),
        CompiledStage::Type => Ok(input_stream
            .iter()
            .map(|v| JsonValue::String(type_of(v).to_string()))
            .collect()),
        CompiledStage::ToString => Ok(input_stream
            .iter()
            .map(|v| JsonValue::String(to_string_value(v)))
            .collect()),
        CompiledStage::Add => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in &input_stream {
                out.push(add_of(v)?);
            }
            Ok(out)
        }
        CompiledStage::Sort => {
            let mut out = Vec::with_capacity(input_stream.len());
            for v in &input_stream {
                out.push(sort_of(v)?);
            }
            Ok(out)
        }
        CompiledStage::Has(key) => Ok(input_stream
            .iter()
            .map(|v| JsonValue::Bool(has_key(v, key)))
            .collect()),
        CompiledStage::DotPath(tokens) => {
            let mut out = Vec::with_capacity(input_stream.len());
            if is_scalar_path(tokens) {
                for v in input_stream {
                    out.push(eval_path_single_ref(&v, tokens).cloned().unwrap_or(JsonValue::Null));
                }
            } else {
                for v in input_stream {
                    out.extend(eval_dot_tokens(tokens, &v));
                }
            }
            Ok(out)
        }
        CompiledStage::Literal(v) => Ok(vec![v.clone(); input_stream.len().max(1)]),
    }
}

fn eval_compiled_predicate(pred: &CompiledPredicate, value: &JsonValue) -> Result<bool, Error> {
    match pred {
        CompiledPredicate::And(l, r) => {
            if !eval_compiled_predicate(l, value)? {
                return Ok(false);
            }
            eval_compiled_predicate(r, value)
        }
        CompiledPredicate::Or(l, r) => {
            if eval_compiled_predicate(l, value)? {
                return Ok(true);
            }
            eval_compiled_predicate(r, value)
        }
        CompiledPredicate::EqPathLiteral(path, lit) => Ok(path_matches_literal(value, path, lit, true)),
        CompiledPredicate::NePathLiteral(path, lit) => Ok(path_matches_literal(value, path, lit, false)),
        CompiledPredicate::Eq(l, r) => {
            Ok(eval_predicate_side(l, value)? == eval_predicate_side(r, value)?)
        }
        CompiledPredicate::Ne(l, r) => {
            Ok(eval_predicate_side(l, value)? != eval_predicate_side(r, value)?)
        }
        CompiledPredicate::Gt(l, r) => Ok(compare_predicate_side(l, r, value, std::cmp::Ordering::Greater)?),
        CompiledPredicate::Ge(l, r) => Ok(compare_predicate_side(l, r, value, std::cmp::Ordering::Equal)?
            || compare_predicate_side(l, r, value, std::cmp::Ordering::Greater)?),
        CompiledPredicate::Lt(l, r) => Ok(compare_predicate_side(l, r, value, std::cmp::Ordering::Less)?),
        CompiledPredicate::Le(l, r) => Ok(compare_predicate_side(l, r, value, std::cmp::Ordering::Equal)?
            || compare_predicate_side(l, r, value, std::cmp::Ordering::Less)?),
        CompiledPredicate::Truthy(q) => {
            Ok(truthy(&eval_predicate_side(q, value)?))
        }
    }
}

fn compare_predicate_side(
    l: &CompiledQuery,
    r: &CompiledQuery,
    value: &JsonValue,
    expect: std::cmp::Ordering,
) -> Result<bool, Error> {
    let lv = eval_predicate_side(l, value)?;
    let rv = eval_predicate_side(r, value)?;
    Ok(compare_json_values(&lv, &rv) == Some(expect))
}

fn compare_json_values(a: &JsonValue, b: &JsonValue) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (JsonValue::Number(la), JsonValue::Number(lb)) => {
            let l = la.as_f64()?;
            let r = lb.as_f64()?;
            l.partial_cmp(&r)
        }
        (JsonValue::String(la), JsonValue::String(lb)) => Some(la.cmp(lb)),
        (JsonValue::Bool(la), JsonValue::Bool(lb)) => Some(la.cmp(lb)),
        _ => None,
    }
}

fn eval_predicate_side(query: &CompiledQuery, value: &JsonValue) -> Result<JsonValue, Error> {
    if let Some(v) = eval_compiled_query_single_fast(query, value) {
        return Ok(v);
    }
    let stream = eval_compiled_query(query, vec![value.clone()])?;
    Ok(first_or_null(&stream))
}

fn eval_compiled_query_single_fast(query: &CompiledQuery, value: &JsonValue) -> Option<JsonValue> {
    match query {
        CompiledQuery::Identity => Some(value.clone()),
        CompiledQuery::Pipeline(stages) if stages.len() == 1 => {
            match &stages[0] {
                CompiledStage::Identity => Some(value.clone()),
                CompiledStage::DotPath(tokens) if is_scalar_path(tokens) => {
                    Some(eval_path_single_ref(value, tokens).cloned().unwrap_or(JsonValue::Null))
                }
                CompiledStage::Literal(v) => Some(v.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn eval_dot_tokens(tokens: &[PathToken], input: &JsonValue) -> Vec<JsonValue> {
    let mut curr = vec![input.clone()];
    for token in tokens {
        let mut next = Vec::with_capacity(curr.len());
        match token {
            PathToken::Iter => {
                for v in curr {
                    if let JsonValue::Array(arr) = v {
                        next.extend(arr);
                    }
                }
            }
            PathToken::Field(name) => {
                for v in curr {
                    next.push(select_field(&v, name));
                }
            }
            PathToken::FieldIter(name) => {
                for v in curr {
                    let selected = select_field(&v, name);
                    if let JsonValue::Array(arr) = selected {
                        next.extend(arr);
                    }
                }
            }
            PathToken::Index(i) => {
                for v in curr {
                    next.push(select_index(&v, *i));
                }
            }
            PathToken::FieldIndex(name, i) => {
                for v in curr {
                    let selected = select_field(&v, name);
                    next.push(select_index(&selected, *i));
                }
            }
        }
        curr = next;
    }
    curr
}

fn is_scalar_path(tokens: &[PathToken]) -> bool {
    !tokens
        .iter()
        .any(|t| matches!(t, PathToken::Iter | PathToken::FieldIter(_)))
}

fn path_matches_literal(input: &JsonValue, tokens: &[PathToken], lit: &JsonValue, eq: bool) -> bool {
    let v = eval_path_single_ref(input, tokens).unwrap_or(&JsonValue::Null);
    if eq {
        v == lit
    } else {
        v != lit
    }
}

fn eval_path_single_ref<'a>(input: &'a JsonValue, tokens: &[PathToken]) -> Option<&'a JsonValue> {
    let mut current: Option<&JsonValue> = Some(input);
    for token in tokens {
        let Some(v) = current else { return None };
        current = match token {
            PathToken::Field(name) => match v {
                JsonValue::Object(m) => m.get(name),
                _ => None,
            },
            PathToken::Index(i) => match v {
                JsonValue::Array(arr) => {
                    let n = arr.len() as i64;
                    let idx = if *i < 0 { n + *i } else { *i };
                    if idx < 0 || idx >= n {
                        None
                    } else {
                        arr.get(idx as usize)
                    }
                }
                _ => None,
            },
            PathToken::FieldIndex(name, i) => match v {
                JsonValue::Object(m) => {
                    let base = m.get(name)?;
                    match base {
                        JsonValue::Array(arr) => {
                            let n = arr.len() as i64;
                            let idx = if *i < 0 { n + *i } else { *i };
                            if idx < 0 || idx >= n {
                                None
                            } else {
                                arr.get(idx as usize)
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
            PathToken::Iter | PathToken::FieldIter(_) => None,
        };
    }
    current
}

fn try_compile_path_literal_predicate(
    left: &str,
    right: &str,
    eq: bool,
) -> Result<Option<CompiledPredicate>, Error> {
    if let Some(path) = try_compile_scalar_path(left)? {
        if let Ok(lit) = serde_json::from_str::<JsonValue>(right) {
            return Ok(Some(if eq {
                CompiledPredicate::EqPathLiteral(path, lit)
            } else {
                CompiledPredicate::NePathLiteral(path, lit)
            }));
        }
    }
    if let Some(path) = try_compile_scalar_path(right)? {
        if let Ok(lit) = serde_json::from_str::<JsonValue>(left) {
            return Ok(Some(if eq {
                CompiledPredicate::EqPathLiteral(path, lit)
            } else {
                CompiledPredicate::NePathLiteral(path, lit)
            }));
        }
    }
    Ok(None)
}

fn try_compile_scalar_path(s: &str) -> Result<Option<Vec<PathToken>>, Error> {
    let t = s.trim();
    if !t.starts_with('.') {
        return Ok(None);
    }
    let tokens = compile_dot_path(t)?;
    if tokens
        .iter()
        .any(|x| matches!(x, PathToken::Iter | PathToken::FieldIter(_)))
    {
        return Ok(None);
    }
    Ok(Some(tokens))
}

fn is_issue2593_pattern(query: &str) -> bool {
    if let Some((var, rest)) = query.strip_prefix(". as $").and_then(|x| x.split_once(" | ")) {
        if rest == format!("keys[] | ${var}[.]") {
            return true;
        }
        if let Some((left, right)) = rest.split_once(" | . as $") {
            if left == "keys[]" {
                if let Some((tmp, tail)) = right.split_once(" | ") {
                    if tail == format!("${var}[${tmp}]") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn keys_then_lookup(container: &JsonValue, keys_source: &JsonValue) -> Vec<JsonValue> {
    let keys = keys_list(keys_source);
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        out.push(select_by_key(container, &k));
    }
    out
}

fn issue2593_lookup(root: &JsonValue) -> Vec<JsonValue> {
    match root {
        JsonValue::Array(arr) => arr.clone(),
        JsonValue::Object(map) => {
            let mut kv = map.iter().collect::<Vec<_>>();
            kv.sort_unstable_by(|(ka, _), (kb, _)| ka.cmp(kb));
            kv.into_iter().map(|(_, v)| v.clone()).collect()
        }
        _ => keys_then_lookup(root, root),
    }
}

fn first_or_null(values: &[JsonValue]) -> JsonValue {
    values.first().cloned().unwrap_or(JsonValue::Null)
}

fn truthy(v: &JsonValue) -> bool {
    match v {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(true),
        JsonValue::String(s) => !s.is_empty(),
        JsonValue::Array(a) => !a.is_empty(),
        JsonValue::Object(m) => !m.is_empty(),
    }
}

fn parse_func_inner<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}(");
    if !s.starts_with(&prefix) || !s.ends_with(')') {
        return None;
    }
    Some(&s[prefix.len()..s.len() - 1])
}

fn split_top_level<'a>(s: &'a str, ch: char) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut in_str = false;
    let mut esc = false;
    let mut last = 0usize;
    for (i, c) in s.char_indices() {
        if in_str {
            if esc {
                esc = false;
                continue;
            }
            if c == '\\' {
                esc = true;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            _ if c == ch && depth_paren == 0 && depth_bracket == 0 => {
                out.push(&s[last..i]);
                last = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[last..]);
    out
}

fn split_once_top_level<'a>(s: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut in_str = false;
    let mut esc = false;
    let mut i = 0usize;
    while i < s.len() {
        let c = s[i..].chars().next()?;
        if in_str {
            if esc {
                esc = false;
                i += c.len_utf8();
                continue;
            }
            if c == '\\' {
                esc = true;
                i += c.len_utf8();
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += c.len_utf8();
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            _ => {}
        }
        if depth_paren == 0 && depth_bracket == 0 && s[i..].starts_with(needle) {
            let l = &s[..i];
            let r = &s[i + needle.len()..];
            return Some((l, r));
        }
        i += c.len_utf8();
    }
    None
}

fn split_once_top_level_keyword<'a>(s: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let needle = format!(" {keyword} ");
    split_once_top_level(s, &needle)
}

fn select_field(v: &JsonValue, field: &str) -> JsonValue {
    match v {
        JsonValue::Object(m) => m.get(field).cloned().unwrap_or(JsonValue::Null),
        _ => JsonValue::Null,
    }
}

fn select_index(v: &JsonValue, idx: i64) -> JsonValue {
    match v {
        JsonValue::Array(arr) => {
            let n = arr.len() as i64;
            let i = if idx < 0 { n + idx } else { idx };
            if i < 0 || i >= n {
                JsonValue::Null
            } else {
                arr[i as usize].clone()
            }
        }
        _ => JsonValue::Null,
    }
}

fn select_by_key(container: &JsonValue, key: &JsonValue) -> JsonValue {
    match (container, key) {
        (JsonValue::Object(m), JsonValue::String(s)) => m.get(s).cloned().unwrap_or(JsonValue::Null),
        (JsonValue::Array(_), JsonValue::Number(n)) => n
            .as_i64()
            .map(|i| select_index(container, i))
            .unwrap_or(JsonValue::Null),
        (JsonValue::Array(_), JsonValue::String(s)) => s
            .parse::<i64>()
            .ok()
            .map(|i| select_index(container, i))
            .unwrap_or(JsonValue::Null),
        _ => JsonValue::Null,
    }
}

fn length_of(v: &JsonValue) -> u64 {
    match v {
        JsonValue::Array(a) => a.len() as u64,
        JsonValue::Object(m) => m.len() as u64,
        JsonValue::String(s) => s.chars().count() as u64,
        JsonValue::Null => 0,
        _ => 1,
    }
}

fn keys_of(v: &JsonValue) -> JsonValue {
    JsonValue::Array(keys_list(v))
}

fn keys_list(v: &JsonValue) -> Vec<JsonValue> {
    match v {
        JsonValue::Object(m) => {
            let mut keys = m.keys().cloned().collect::<Vec<_>>();
            keys.sort_unstable();
            keys.into_iter().map(JsonValue::String).collect()
        }
        JsonValue::Array(a) => (0..a.len()).map(|i| JsonValue::from(i as u64)).collect(),
        _ => Vec::new(),
    }
}

fn sort_of(v: &JsonValue) -> Result<JsonValue, Error> {
    let JsonValue::Array(arr) = v else {
        return Err(Error::Unsupported("sort requires array input".to_string()));
    };
    let mut out = arr.clone();
    out.sort_by(|a, b| canonical(a).cmp(&canonical(b)));
    Ok(JsonValue::Array(out))
}

fn add_of(v: &JsonValue) -> Result<JsonValue, Error> {
    let JsonValue::Array(arr) = v else {
        return Err(Error::Unsupported("add requires array input".to_string()));
    };
    if arr.is_empty() {
        return Ok(JsonValue::Null);
    }
    if arr.iter().all(|x| x.is_number()) {
        let sum = arr
            .iter()
            .filter_map(|x| x.as_f64())
            .fold(0.0, |acc, n| acc + n);
        if sum.fract() == 0.0 {
            return Ok(JsonValue::from(sum as i64));
        }
        return Ok(JsonValue::from(sum));
    }
    if arr.iter().all(|x| x.is_string()) {
        let mut out = String::new();
        for x in arr {
            if let Some(s) = x.as_str() {
                out.push_str(s);
            }
        }
        return Ok(JsonValue::String(out));
    }
    Err(Error::Unsupported("add supports arrays of numbers or strings".to_string()))
}

fn canonical(v: &JsonValue) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn type_of(v: &JsonValue) -> &'static str {
    match v {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn to_string_value(v: &JsonValue) -> String {
    match v {
        JsonValue::String(s) => s.clone(),
        _ => serde_json::to_string(v).unwrap_or_default(),
    }
}

fn parse_string_literal(s: &str) -> Option<String> {
    if !(s.starts_with('"') && s.ends_with('"')) {
        return None;
    }
    serde_json::from_str::<String>(s).ok()
}

fn has_key(v: &JsonValue, key: &str) -> bool {
    match v {
        JsonValue::Object(m) => m.contains_key(key),
        JsonValue::Array(a) => key
            .parse::<i64>()
            .ok()
            .map(|i| {
                let n = a.len() as i64;
                let idx = if i < 0 { n + i } else { i };
                idx >= 0 && idx < n
            })
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::fs;
    use std::path::PathBuf;

    #[derive(Debug, Deserialize)]
    struct CompatFile {
        cases: Vec<CompatCase>,
    }

    #[derive(Debug, Deserialize)]
    struct CompatCase {
        id: String,
        query: String,
        input_json: Option<String>,
        input_yaml: Option<String>,
        output_json_lines: Vec<String>,
    }

    #[test]
    fn jq_compat_subset() {
        let path = compat_file("jq-cases.yaml");
        let data = fs::read_to_string(&path).expect("read compat");
        let suite: CompatFile = serde_yaml::from_str(&data).expect("parse compat");
        for case in suite.cases {
            let input = case.input_json.as_deref().expect("input_json");
            let out = run_json_query(&case.query, input)
                .unwrap_or_else(|e| panic!("case {} failed: {e}", case.id));
            let got = out
                .iter()
                .map(|v| serde_json::to_string(v).expect("json"))
                .collect::<Vec<_>>();
            assert_eq!(got, case.output_json_lines, "case {}", case.id);
        }
    }

    #[test]
    fn yq_compat_subset() {
        let path = compat_file("yq-cases.yaml");
        let data = fs::read_to_string(&path).expect("read compat");
        let suite: CompatFile = serde_yaml::from_str(&data).expect("parse compat");
        for case in suite.cases {
            let input = case.input_yaml.as_deref().expect("input_yaml");
            let out = run_yaml_query(&case.query, input)
                .unwrap_or_else(|e| panic!("case {} failed: {e}", case.id));
            let got = out
                .iter()
                .map(|v| serde_json::to_string(v).expect("json"))
                .collect::<Vec<_>>();
            assert_eq!(got, case.output_json_lines, "case {}", case.id);
        }
    }

    #[test]
    fn jq_rejects_unescaped_control_chars_issue2909_regression() {
        let bad = "{\"s\":\"a\u{001f}b\"}";
        let err = run_json_query(".", bad).expect_err("must fail on control char");
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn jq_accepts_yaml_input() {
        let input = r#"
a:
  b: 42
"#;
        let out = run_json_query(".a.b", input).expect("query");
        assert_eq!(out, vec![serde_json::json!(42)]);
    }

    #[test]
    fn yq_accepts_json_input() {
        let input = r#"{"a":{"b":42}}"#;
        let out = run_yaml_query(".a.b", input).expect("query");
        assert_eq!(out, vec![serde_json::json!(42)]);
    }

    #[test]
    fn compile_query_parses_pipeline_once() {
        let q = ".[] | select(.a == 2) | .b";
        let compiled = compile_query(q).expect("compile");
        let input = serde_json::json!([{"a":1,"b":"x"},{"a":2,"b":"y"}]);
        let out = eval_compiled_query(&compiled, vec![input]).expect("eval");
        let got = out
            .iter()
            .map(|v| serde_json::to_string(v).expect("json"))
            .collect::<Vec<_>>();
        assert_eq!(got, vec!["\"y\""]);
    }

    #[test]
    fn compile_predicate_uses_fast_path_for_scalar_path_literal() {
        let p = compile_predicate(".a.b == 2").expect("compile");
        assert!(matches!(p, CompiledPredicate::EqPathLiteral(_, _)));
    }

    #[test]
    fn compile_stage_uses_map_path_fast_path() {
        let s = compile_stage("map(.a.b)").expect("compile");
        assert!(matches!(s, CompiledStage::MapPath(_)));
    }

    #[test]
    fn compile_stage_uses_dot_iter_fast_path() {
        let s = compile_stage(".[]").expect("compile");
        assert!(matches!(s, CompiledStage::DotIter));
    }

    #[test]
    fn compile_stage_uses_dot_iter_field_fast_path() {
        let s = compile_stage(".[].name").expect("compile");
        assert!(matches!(s, CompiledStage::DotIterField(_)));
    }

    #[test]
    fn compile_dot_path_supports_bracket_string_key() {
        let t = compile_dot_path(".[\"a-b\"]").expect("compile");
        assert!(matches!(t.as_slice(), [PathToken::Field(_)]));
    }

    #[test]
    fn compile_dot_path_supports_field_bracket_string_key() {
        let t = compile_dot_path(".root[\"a-b\"]").expect("compile");
        assert!(matches!(t.as_slice(), [PathToken::Field(_), PathToken::Field(_)]));
    }

    #[test]
    fn run_query_supports_bracket_string_key_lookup() {
        let out = run_json_query(".[\"a-b\"]", r#"{"a-b": 9}"#).expect("query");
        assert_eq!(out, vec![serde_json::json!(9)]);
    }

    #[test]
    fn run_query_supports_nested_bracket_string_key_lookup() {
        let out = run_json_query(".root[\"a-b\"]", r#"{"root":{"a-b": 5}}"#).expect("query");
        assert_eq!(out, vec![serde_json::json!(5)]);
    }

    #[test]
    fn run_query_supports_optional_field_operator() {
        let out = run_json_query(".missing?", r#"{"a":1}"#).expect("query");
        assert_eq!(out, vec![serde_json::Value::Null]);
    }

    #[test]
    fn run_query_supports_optional_iter_operator() {
        let out = run_json_query(".[]?", r#"{"a":1}"#).expect("query");
        assert!(out.is_empty());
    }

    #[test]
    fn run_query_supports_predicate_gt() {
        let out = run_json_query(".[] | select(.a > 1) | .a", r#"[{"a":1},{"a":2},{"a":3}]"#)
            .expect("query");
        assert_eq!(out, vec![serde_json::json!(2), serde_json::json!(3)]);
    }

    #[test]
    fn run_query_supports_predicate_and_or() {
        let out = run_json_query(
            r#".[] | select(.a > 1 and .name == "x") | .a"#,
            r#"[{"a":1,"name":"x"},{"a":2,"name":"x"},{"a":3,"name":"y"}]"#,
        )
        .expect("query");
        assert_eq!(out, vec![serde_json::json!(2)]);

        let out_or = run_json_query(
            r#".[] | select(.a == 1 or .name == "y") | .a"#,
            r#"[{"a":1,"name":"x"},{"a":2,"name":"x"},{"a":3,"name":"y"}]"#,
        )
        .expect("query");
        assert_eq!(out_or, vec![serde_json::json!(1), serde_json::json!(3)]);
    }

    #[test]
    fn scalar_path_detection() {
        assert!(is_scalar_path(&compile_dot_path(".a.b[0]").expect("path")));
        assert!(!is_scalar_path(&compile_dot_path(".a[]").expect("path")));
    }

    #[test]
    fn issue2593_fast_path_array_identity() {
        let src = serde_json::json!(["a", "b", "c"]);
        let out = issue2593_lookup(&src);
        let got = serde_json::Value::Array(out);
        assert_eq!(got, src);
    }

    #[test]
    fn issue2593_fast_path_object_sorted_by_key() {
        let src = serde_json::json!({"b": 2, "a": 1, "c": 3});
        let out = issue2593_lookup(&src);
        assert_eq!(out, vec![serde_json::json!(1), serde_json::json!(2), serde_json::json!(3)]);
    }

    #[test]
    fn keys_list_is_sorted_for_objects() {
        let src = serde_json::json!({"z": 1, "a": 2, "m": 3});
        let keys = keys_list(&src);
        assert_eq!(
            keys,
            vec![
                serde_json::json!("a"),
                serde_json::json!("m"),
                serde_json::json!("z")
            ]
        );
    }

    #[test]
    fn predicate_single_fast_works_for_scalar_dot_path() {
        let q = compile_query(".a.b").expect("compile");
        let src = serde_json::json!({"a":{"b": 7}});
        let v = eval_compiled_query_single_fast(&q, &src).expect("fast");
        assert_eq!(v, serde_json::json!(7));
    }

    #[test]
    fn predicate_single_fast_works_for_literal_stage() {
        let q = compile_query("42").expect("compile");
        let src = serde_json::json!({"a": 1});
        let v = eval_compiled_query_single_fast(&q, &src).expect("fast");
        assert_eq!(v, serde_json::json!(42));
    }

    #[test]
    fn predicate_single_fast_skips_collect_query() {
        let q = compile_query("[.a]").expect("compile");
        let src = serde_json::json!({"a": 1});
        assert!(eval_compiled_query_single_fast(&q, &src).is_none());
    }

    fn compat_file(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("compat")
            .join(name)
    }
}
