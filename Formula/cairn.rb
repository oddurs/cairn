# Homebrew formula. Update `version` and the four sha256 values from the
# SHA256SUMS file attached to the release; everything else stays put.
class Cairn < Formula
  desc "Markdown-native roadmap and issue manager that lives in your repository"
  homepage "https://github.com/oddurs/cairn"
  version "0.1.0"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_DARWIN_SHA256"
    end
    on_intel do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_DARWIN_SHA256"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_LINUX_SHA256"
    end
    on_intel do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_X86_64_LINUX_SHA256"
    end
  end

  def install
    bin.install "cairn"
    generate_completions_from_executable(bin/"cairn", "completions")
    man1.install Utils.safe_popen_read(bin/"cairn", "man") => "cairn.1"
  end

  test do
    system bin/"cairn", "--version"
    system bin/"cairn", "init", "--bare", "--name", "test"
    assert_predicate testpath/"cairn.toml", :exist?
    system bin/"cairn", "new", "A thing", "-q"
    assert_match "A thing", shell_output("#{bin}/cairn list")
  end
end
