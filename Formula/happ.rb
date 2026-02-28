class Happ < Formula
  desc "Helm chart and manifest importer/converter for helm-apps"
  homepage "https://github.com/alvnukov/helm-apps"
  version "latest"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/latest/download/happ_darwin_arm64"
      sha256 :no_check
    else
      url "https://github.com/alvnukov/helm-apps/releases/latest/download/happ_darwin_amd64"
      sha256 :no_check
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/latest/download/happ_linux_arm64"
      sha256 :no_check
    else
      url "https://github.com/alvnukov/helm-apps/releases/latest/download/happ_linux_amd64"
      sha256 :no_check
    end
  end

  def install
    bin.install Dir["happ_*"].first => "happ"
  end

  test do
    assert_match "happ", shell_output("#{bin}/happ --help 2>&1")
  end
end
