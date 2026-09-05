# Homebrew formula. Update `version` and the four sha256 values from the
# SHA256SUMS file attached to the release; everything else stays put.
class Cairn < Formula
  desc "Markdown-native roadmap and issue manager that lives in your repository"
  homepage "https://github.com/oddurs/cairn"
  version "0.1.0-rc.1"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "640cf5d260187d2b41faa166d129bab933df37d86811cb793b2b3897fc1e6e1f"
    end
    on_intel do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "bb83bcba68432f7f290f967eb9ce0d1ecd2db0b401710c6cdcd9ad628af258f3"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "4fe2f6205a5ff6d0ec9edaf491e8e26d9308e42f861bb42df7e2e22e8b79d315"
    end
    on_intel do
      url "https://github.com/oddurs/cairn/releases/download/v#{version}/cairn-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "19cb4ce5c8730fab4c19191672ca1e8d48f95edf356c3267b617283ee509d28a"
    end
  end

  def install
    bin.install "cairn"
    generate_completions_from_executable(bin/"cairn", "completions")
    # `man` writes the page to stdout, so it has to become a file before
    # Homebrew can install it: `install` takes a path, not contents.
    (buildpath/"cairn.1").write Utils.safe_popen_read(bin/"cairn", "man")
    man1.install "cairn.1"
  end

  test do
    system bin/"cairn", "--version"
    system bin/"cairn", "init", "--bare", "--name", "test"
    assert_predicate testpath/"cairn.toml", :exist?
    system bin/"cairn", "new", "A thing", "-q"
    assert_match "A thing", shell_output("#{bin}/cairn list")
  end
end
