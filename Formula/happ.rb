class Happ < Formula
  desc "Helm chart and manifest importer/converter for helm-apps"
  homepage "https://github.com/alvnukov/helm-apps"
  version "1.8.3"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_darwin_arm64"
      sha256 "1cb57b8a68df97c377f2477d1bfe6f06906c324ab038a66574a4f61b406621df"
    else
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_darwin_amd64"
      sha256 "02f839ab5ebaceabe64b80dccbf0b92283d12ca43f49743dbc3a79875287750f"
    end
  end

  on_linux do
    url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_linux_amd64"
    sha256 "576c3e8753b6bd2c60d4e4ac28bf21b52f36a11472d3bc9cbd874e359278420e"
  end

  def install
    if OS.linux? && Hardware::CPU.arm?
      odie "happ: linux arm64 binary is temporarily unavailable in this release"
    end
    bin.install Dir["happ_*"].first => "happ"
  end

  test do
    assert_match "happ", shell_output("#{bin}/happ --help 2>&1")
  end
end
