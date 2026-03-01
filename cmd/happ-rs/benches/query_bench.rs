use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::path::{Path, PathBuf};
use std::process::Command;

fn bench_query_engines(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bench_file = root.join("target").join("bench-query.json");
    let bench_yaml = root.join("target").join("bench-query.yaml");
    ensure_bench_input(&bench_file);
    ensure_bench_yaml_input(&bench_yaml);

    let happ_bin = root.join("target").join("release").join("happ");
    if !happ_bin.exists() {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&root)
            .status()
            .expect("build happ");
        assert!(status.success(), "cargo build --release failed");
    }

    let mut g = c.benchmark_group("query-engines");
    let scenarios = [
        ("select-hot-path", ".[] | select(.a == 5) | .b"),
        ("map-path", "map(.b)"),
        ("collect-select", "[.[] | select(.a == 5) | .b]"),
        ("keys-lookup-issue2593", ". as $o | keys[] | $o[.]"),
    ];
    for (scenario, query) in scenarios {
        g.bench_with_input(BenchmarkId::new("jq", scenario), &bench_file, |b, file| {
            b.iter(|| {
                let status = Command::new("jq")
                    .arg(query)
                    .arg(file)
                    .stdout(std::process::Stdio::null())
                    .status()
                    .expect("run jq");
                assert!(status.success(), "jq failed");
            })
        });
        g.bench_with_input(BenchmarkId::new("happ-jq", scenario), &bench_file, |b, file| {
            b.iter(|| {
                let status = Command::new(&happ_bin)
                    .arg("jq")
                    .arg("--query")
                    .arg(query)
                    .arg("--input")
                    .arg(file)
                    .arg("--compact")
                    .stdout(std::process::Stdio::null())
                    .status()
                    .expect("run happ jq");
                assert!(status.success(), "happ jq failed");
            })
        });
    }
    g.finish();

    let mut gy = c.benchmark_group("yaml-stream");
    let yq_scenarios = [
        ("doc-first", "first", ".base.k"),
        ("doc-all", "all", ".base.k"),
        ("merge-read", "all", ".svc.v"),
    ];
    for (scenario, doc_mode, query) in yq_scenarios {
        gy.bench_with_input(BenchmarkId::new("happ-yq", scenario), &bench_yaml, |b, file| {
            b.iter(|| {
                let status = Command::new(&happ_bin)
                    .arg("yq")
                    .arg("--query")
                    .arg(query)
                    .arg("--input")
                    .arg(file)
                    .arg("--doc-mode")
                    .arg(doc_mode)
                    .arg("--compact")
                    .stdout(std::process::Stdio::null())
                    .status()
                    .expect("run happ yq");
                assert!(status.success(), "happ yq failed");
            })
        });
    }
    gy.finish();
}

fn ensure_bench_input(path: &Path) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let mut arr = Vec::with_capacity(200_000);
    for i in 0..200_000u64 {
        arr.push(serde_json::json!({"a": i % 10, "b": i}));
    }
    let body = serde_json::to_vec(&arr).expect("encode");
    std::fs::write(path, body).expect("write bench input");
}

fn ensure_bench_yaml_input(path: &Path) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let mut docs = String::new();
    for i in 0..20_000u64 {
        docs.push_str(&format!(
            r#"---
base: &base
  k: {i}
  m: {m}
svc:
  <<: *base
  v: {v}
"#,
            m = i % 10,
            v = i + 1
        ));
    }
    std::fs::write(path, docs).expect("write bench yaml");
}

criterion_group!(benches, bench_query_engines);
criterion_main!(benches);
