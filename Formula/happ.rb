class Happ < Formula
  desc "Helm chart and manifest importer/converter for helm-apps"
  homepage "https://github.com/alvnukov/helm-apps"
  version "1.8.1"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_darwin_arm64"
      sha256 "adc51dad7f8287e519d6cdac988b6363a971858e5a070a147794b4407f4a9852"
    else
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_darwin_amd64"
      sha256 "8fbe16b2bd92519f0c10e3cc75058ec48d21bd01de1796ef4a903d676534e0ad"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_linux_arm64"
      sha256 "61a457e3b27c39d2765af730c96ae678b46a206797d9871ff62f16ca7cf0de77"
    else
      url "https://github.com/alvnukov/helm-apps/releases/download/helm-apps-#{version}/happ_linux_amd64"
      sha256 "ac63f9e37e9b7ed5e065ceb93ce915eb792967ad26e09269f03471ccfec3827d"
    end
  end

  def install
    bin.install Dir["happ_*"].first => "happ"
  end

  test do
    assert_match "happ", shell_output("#{bin}/happ --help 2>&1")
  end
end
