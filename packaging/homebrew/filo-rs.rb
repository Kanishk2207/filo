# Formula token is "filo-rs" (the binary it installs is still "filo").
# The class name is the token camel-cased with separators removed: filo-rs -> FiloRs.
class FiloRs < Formula
  desc "Your Downloads folder cleans itself — safely, predictably, and reversibly"
  homepage "https://github.com/Kanishk2207/filo"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/Kanishk2207/filo/releases/download/v#{version}/filo-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_aarch64-apple-darwin_SHA256"
    end
    on_intel do
      url "https://github.com/Kanishk2207/filo/releases/download/v#{version}/filo-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_x86_64-apple-darwin_SHA256"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/Kanishk2207/filo/releases/download/v#{version}/filo-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_aarch64-unknown-linux-gnu_SHA256"
    end
    on_intel do
      url "https://github.com/Kanishk2207/filo/releases/download/v#{version}/filo-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_x86_64-unknown-linux-gnu_SHA256"
    end
  end

  def install
    bin.install "filo"
  end

  test do
    assert_match "filo", shell_output("#{bin}/filo --version")
  end
end
