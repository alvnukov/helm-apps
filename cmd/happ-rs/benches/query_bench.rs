use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::path::{Path, PathBuf};
use std::process::Command;

fn bench_query_engines(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bench_file = root.join("target").join("bench-query.json");
    ensure_bench_input(&bench_file);

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

criterion_group!(benches, bench_query_engines);
criterion_main!(benches);
