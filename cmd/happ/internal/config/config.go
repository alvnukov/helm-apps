package config

import "time"

const (
	DefaultGroupName                  = "apps-k8s-manifests"
	DefaultGroupType                  = "apps-k8s-manifests"
	ImportStrategyRaw                 = "raw"
	ImportStrategyHelpersExperimental = "helpers-experimental"
	CommandImport                     = "import"
	CommandInspect                    = "inspect"
	CommandComposeInspect             = "compose-inspect"
	CommandDyff                       = "dyff"
)

type Config struct {
	Command            string
	SourceMode         string
	Input              string
	ReleaseName        string
	Namespace          string
	ValuesFiles        []string
	SetValues          []string
	SetStringValues    []string
	SetFileValues      []string
	SetJSONValues      []string
	KubeVersion        string
	APIVersions        []string
	IncludeCRDs        bool
	HelmExecTimeout    time.Duration
	PipelineTimeout    time.Duration
	AnalyzeTimeout     time.Duration
	VerifyTimeout      time.Duration
	RenderedOutput     string
	OutChartDir        string
	ConsumerChartName  string
	LibraryChartPath   string
	VerifyEquivalence  bool
	ImportStrategy     string
	AnalyzeTemplates   bool
	AnalyzeReportPath  string
	InspectWeb         bool
	InspectAddr        string
	InspectOpenBrowser bool

	Env                          string
	GroupName                    string
	GroupType                    string
	MinIncludeBytes              int
	IncludeStatus                bool
	Output                       string
	RendererOutput               string
	ComposeFormat                string
	ComposeHealthcheckToLiveness bool
	DyffFrom                     string
	DyffTo                       string
	DyffIgnoreOrder              bool
	DyffIgnoreWhitespace         bool
	DyffAdditionalIdentifiers    []string
	DyffQuiet                    bool
	DyffFailOnDiff               bool
	DyffColor                    string
	DyffFormat                   string
	DyffStats                    bool
	DyffLabelFrom                string
	DyffLabelTo                  string
}
