class Happ < Formula
  desc "Helm chart and manifest importer/converter for helm-apps"
  homepage "https://github.com/alvnukov/helm-apps"
  version "1.8.3"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-\#{version}/happ_darwin_arm64"
      sha256 "f0556f8147ef33933e4ff4cdafd70d5b861654499020dc7d4300d020d34bbacd"
    else
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-\#{version}/happ_darwin_amd64"
      sha256 "d144b3a264d72a5fafc0ad618b2efa0b8a7d62eb14e9c6fef750d2fb76898aa7"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-\#{version}/happ_linux_arm64"
      sha256 "47bdf19ca697c749e2f8aa98df2c973736124824115ca91e3559f8b830c377cb"
    else
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-\#{version}/happ_linux_amd64"
      sha256 "ecadb3f4b2707f1bde296604990f837907cd6a4c970e16dbcd998898968b12ab"
    end
  end

  def install
    bin.install Dir["happ_*"].first => "happ"
  end

  test do
    assert_match "happ", shell_output("\#{bin}/happ --help 2>&1")
  end
end
