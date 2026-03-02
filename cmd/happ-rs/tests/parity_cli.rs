use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_happ")
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures_dir() -> PathBuf {
    root().join("tests/parity/fixtures")
}

fn fixture(name: &str) -> String {
    fixtures_dir().join(name).to_string_lossy().to_string()
}

fn run_happ(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(root())
        .output()
        .expect("run happ")
}

fn stdout_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

fn stderr_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n")
}

fn assert_ok(out: &Output, context: &str) {
    assert!(
        out.status.success(),
        "{context}\nstatus={:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        stdout_text(out),
        stderr_text(out)
    );
}

fn assert_fail(out: &Output, context: &str) {
    assert!(
        !out.status.success(),
        "{context}\nexpected failure, got success\nstdout:\n{}\nstderr:\n{}",
        stdout_text(out),
        stderr_text(out)
    );
}

#[test]
fn parity_help_contract() {
    let out = run_happ(&["--help"]);
    assert_ok(&out, "--help");
    let text = stdout_text(&out);
    for token in [
        "chart",
        "manifests",
        "compose",
        "validate",
        "completion",
        "jq",
        "yq",
        "inspect",
        "compose-inspect",
        "dyff",
    ] {
        assert!(text.contains(token), "help must include subcommand: {token}");
    }
}

#[test]
fn parity_validate_contract() {
    let ok = run_happ(&["validate", "--values", &fixture("valid-values.yaml")]);
    assert_ok(&ok, "validate valid values");
    assert_eq!(stdout_text(&ok).trim(), "OK");

    let bad = run_happ(&["validate", "--values", &fixture("invalid-values.yaml")]);
    assert_fail(&bad, "validate invalid values");
    assert!(
        stderr_text(&bad).contains("happ failed:"),
        "invalid validate should include error prefix"
    );
}

#[test]
fn parity_jq_contract() {
    let out = run_happ(&[
        "jq",
        "--query",
        ".global.env",
        "--input",
        &fixture("valid-values.yaml"),
        "--doc-mode",
        "first",
        "--raw-output",
    ]);
    assert_ok(&out, "jq query");
    assert_eq!(stdout_text(&out).trim(), "dev");
}

#[test]
fn parity_yq_contract() {
    let out = run_happ(&[
        "yq",
        "--query",
        ".global.env",
        "--input",
        &fixture("valid-values.yaml"),
        "--doc-mode",
        "first",
        "--raw-output",
    ]);
    assert_ok(&out, "yq query");
    assert_eq!(stdout_text(&out).trim(), "dev");
}

#[test]
fn parity_dyff_contract() {
    let from = root().join("target/parity-dyff-from.yaml");
    let to = root().join("target/parity-dyff-to.yaml");
    std::fs::create_dir_all(from.parent().expect("target dir")).expect("mkdir target");
    std::fs::write(&from, "a: 1\n").expect("write from");
    std::fs::write(&to, "a: 2\n").expect("write to");
    let from_s = from.to_string_lossy().to_string();
    let to_s = to.to_string_lossy().to_string();

    let text = run_happ(&["dyff", "--from", &from_s, "--to", &to_s]);
    assert_ok(&text, "dyff text");
    assert!(
        stdout_text(&text).contains("changed"),
        "dyff text must show diff"
    );

    let json = run_happ(&[
        "dyff",
        "--from",
        &from_s,
        "--to",
        &to_s,
        "--format",
        "json",
    ]);
    assert_ok(&json, "dyff json");
    assert!(
        stdout_text(&json).contains("\"summary\""),
        "dyff json must include summary"
    );
}

#[test]
fn parity_manifests_import_contract() {
    let out = run_happ(&[
        "manifests",
        "--path",
        &fixture("manifests.yaml"),
        "--import-strategy",
        "helpers",
    ]);
    assert_ok(&out, "manifests import");
    let text = stdout_text(&out);
    assert!(text.contains("global:"), "must include global section");
    assert!(
        text.contains("apps-configmaps:") || text.contains("apps-k8s-manifests:"),
        "must include converted app group"
    );
    assert!(
        text.contains("apps-services:") || text.contains("apps-k8s-manifests:"),
        "must include service conversion"
    );
}

#[test]
fn parity_compose_import_contract() {
    let out = run_happ(&[
        "compose",
        "--path",
        &fixture("compose.yaml"),
        "--import-strategy",
        "helpers",
    ]);
    assert_ok(&out, "compose import");
    let text = stdout_text(&out);
    assert!(text.contains("apps-stateless:"), "must include stateless apps");
    assert!(
        text.contains("apps-services:") || text.contains("apps-k8s-manifests:"),
        "must include service projection"
    );
    assert!(text.contains("web:"), "must include compose service name");
}

#[test]
fn parity_completion_contract() {
    let out = run_happ(&["completion", "--shell", "zsh"]);
    assert_ok(&out, "completion zsh");
    let text = stdout_text(&out);
    assert!(
        text.contains("_happ") || text.contains("compdef happ"),
        "zsh completion script must contain happ completion function"
    );
}

#[test]
fn parity_build_asset_contract() {
    let chart = root().join("target/parity-chart-out");
    if chart.exists() {
        std::fs::remove_dir_all(&chart).expect("clean old chart");
    }
    let chart_s = chart.to_string_lossy().to_string();
    let out = run_happ(&[
        "manifests",
        "--path",
        &fixture("manifests.yaml"),
        "--out-chart-dir",
        &chart_s,
    ]);
    assert_ok(&out, "generate chart with embedded library");
    let embedded_chart_yaml = chart.join(Path::new("charts/helm-apps/Chart.yaml"));
    assert!(
        embedded_chart_yaml.exists(),
        "generated chart must include embedded library Chart.yaml"
    );
}
