use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "happ", about = "happ imports Helm chart render output or raw manifests into a helm-apps-based consumer chart")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Chart(ImportArgs),
    Manifests(ImportArgs),
    Compose(ImportArgs),
    Validate(ValidateArgs),
    #[command(
        about = "Run jq-like query syntax on JSON or YAML input",
        long_about = "Run jq-like query syntax on input data.\nInput may be JSON or YAML; parsing is automatic."
    )]
    Jq(QueryArgs),
    #[command(
        about = "Run yq-like query syntax on YAML or JSON input",
        long_about = "Run yq-like query syntax on input data.\nInput may be YAML or JSON; parsing is automatic."
    )]
    Yq(QueryArgs),
    Inspect(InspectArgs),
    #[command(name = "compose-inspect")]
    ComposeInspect(ComposeInspectArgs),
    Dyff(DyffArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct QueryArgs {
    #[arg(long = "query", help = "Query expression in jq/yq language syntax")]
    pub query: String,
    #[arg(
        long = "input",
        default_value = "-",
        help = "Input file path or '-' for stdin. Supports both JSON and YAML."
    )]
    pub input: String,
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub compact: bool,
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub raw_output: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ValidateArgs {
    #[arg(long)]
    pub values: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn parses_validate_subcommand() {
        let cli = Cli::try_parse_from(["happ", "validate", "--values", "/tmp/values.yaml"])
            .expect("parse validate");
        match cli.command {
            Command::Validate(args) => assert_eq!(args.values, "/tmp/values.yaml"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_compose_inspect_web_flags() {
        let cli = Cli::try_parse_from([
            "happ",
            "compose-inspect",
            "--path",
            "/tmp/compose.yaml",
            "--web=true",
            "--addr",
            "127.0.0.1:9900",
            "--open-browser=false",
        ])
        .expect("parse compose-inspect");
        match cli.command {
            Command::ComposeInspect(args) => {
                assert!(args.web);
                assert_eq!(args.addr, "127.0.0.1:9900");
                assert!(!args.open_browser);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_dyff_extended_flags() {
        let cli = Cli::try_parse_from([
            "happ",
            "dyff",
            "--from",
            "a.yaml",
            "--to",
            "b.yaml",
            "--format",
            "json",
            "--color",
            "never",
            "--stats",
            "--label-from",
            "source",
            "--label-to",
            "generated",
        ])
        .expect("parse dyff");
        match cli.command {
            Command::Dyff(args) => {
                assert_eq!(args.format, "json");
                assert_eq!(args.color, "never");
                assert!(args.stats);
                assert_eq!(args.label_from.as_deref(), Some("source"));
                assert_eq!(args.label_to.as_deref(), Some("generated"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_dyff_github_summary_only_flags() {
        let cli = Cli::try_parse_from([
            "happ",
            "dyff",
            "--from",
            "a.yaml",
            "--to",
            "b.yaml",
            "--format",
            "github",
            "--summary-only",
        ])
        .expect("parse dyff github");
        match cli.command {
            Command::Dyff(args) => {
                assert_eq!(args.format, "github");
                assert!(args.summary_only);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_jq_subcommand() {
        let cli = Cli::try_parse_from([
            "happ",
            "jq",
            "--query",
            ".a",
            "--input",
            "in.json",
            "--compact",
            "--raw-output",
        ])
        .expect("parse jq");
        match cli.command {
            Command::Jq(args) => {
                assert_eq!(args.query, ".a");
                assert_eq!(args.input, "in.json");
                assert!(args.compact);
                assert!(args.raw_output);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_yq_subcommand() {
        let cli = Cli::try_parse_from(["happ", "yq", "--query", ".a", "--input", "in.yaml"])
            .expect("parse yq");
        match cli.command {
            Command::Yq(args) => {
                assert_eq!(args.query, ".a");
                assert_eq!(args.input, "in.yaml");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn jq_help_mentions_json_and_yaml_input() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.find_subcommand_mut("jq")
            .expect("jq subcommand")
            .write_long_help(&mut buf)
            .expect("write help");
        let help = String::from_utf8(buf).expect("utf8");
        assert!(help.contains("Input may be JSON or YAML"));
        assert!(help.contains("Supports both JSON and YAML."));
    }

    #[test]
    fn yq_help_mentions_yaml_and_json_input() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.find_subcommand_mut("yq")
            .expect("yq subcommand")
            .write_long_help(&mut buf)
            .expect("write help");
        let help = String::from_utf8(buf).expect("utf8");
        assert!(help.contains("Input may be YAML or JSON"));
        assert!(help.contains("Supports both JSON and YAML."));
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct ImportArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value = "dev")]
    pub env: String,
    #[arg(long, default_value = "apps-k8s-manifests")]
    pub group_name: String,
    #[arg(long, default_value = "apps-k8s-manifests")]
    pub group_type: String,
    #[arg(long, default_value_t = 24)]
    pub min_include_bytes: usize,
    #[arg(long, action = ArgAction::SetTrue)]
    pub include_status: bool,
    #[arg(long)]
    pub output: Option<String>,
    #[arg(long)]
    pub out_chart_dir: Option<String>,
    #[arg(long)]
    pub chart_name: Option<String>,
    #[arg(long)]
    pub library_chart_path: Option<String>,
    #[arg(long, default_value = "raw")]
    pub import_strategy: String,

    #[arg(long, default_value = "imported")]
    pub release_name: String,
    #[arg(long)]
    pub namespace: Option<String>,
    #[arg(long = "values")]
    pub values_files: Vec<String>,
    #[arg(long = "set")]
    pub set_values: Vec<String>,
    #[arg(long = "set-string")]
    pub set_string_values: Vec<String>,
    #[arg(long = "set-file")]
    pub set_file_values: Vec<String>,
    #[arg(long = "set-json")]
    pub set_json_values: Vec<String>,
    #[arg(long)]
    pub kube_version: Option<String>,
    #[arg(long = "api-version")]
    pub api_versions: Vec<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub include_crds: bool,
    #[arg(long)]
    pub write_rendered_output: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InspectArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value = "inspect")]
    pub release_name: String,
    #[arg(long)]
    pub namespace: Option<String>,
    #[arg(long = "values")]
    pub values_files: Vec<String>,
    #[arg(long = "set")]
    pub set_values: Vec<String>,
    #[arg(long = "set-string")]
    pub set_string_values: Vec<String>,
    #[arg(long = "set-file")]
    pub set_file_values: Vec<String>,
    #[arg(long = "set-json")]
    pub set_json_values: Vec<String>,
    #[arg(long)]
    pub kube_version: Option<String>,
    #[arg(long = "api-version")]
    pub api_versions: Vec<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub include_crds: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    pub web: bool,
    #[arg(long, default_value = "127.0.0.1:8088")]
    pub addr: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ComposeInspectArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value = "yaml")]
    pub format: String,
    #[arg(long)]
    pub output: Option<String>,
    #[arg(long, default_value_t = false, action = ArgAction::Set)]
    pub web: bool,
    #[arg(long, default_value = "127.0.0.1:8089")]
    pub addr: String,
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    pub open_browser: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct DyffArgs {
    #[arg(long)]
    pub from: String,
    #[arg(long)]
    pub to: String,
    #[arg(long, action = ArgAction::SetTrue)]
    pub ignore_order: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub ignore_whitespace: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub quiet: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub fail_on_diff: bool,
    #[arg(long, default_value = "text")]
    pub format: String,
    #[arg(long, default_value = "auto")]
    pub color: String,
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub stats: bool,
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub summary_only: bool,
    #[arg(long)]
    pub label_from: Option<String>,
    #[arg(long)]
    pub label_to: Option<String>,
    #[arg(long)]
    pub output: Option<String>,
}
