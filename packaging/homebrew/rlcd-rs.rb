# Generated from packaging/homebrew/rlcd-rs.rb in apiplant/rlcd-rs by the
# release workflow, which fills in the version and checksums and commits the
# result to apiplant/homebrew-tap as Formula/rlcd-rs.rb. Changes belong in
# the source repository: the next release overwrites this file.
class LayaRs < Formula
  desc "Rust (candle) inference for Laya's typed-decision engine"
  homepage "https://github.com/apiplant/rlcd-rs"
  version "@VERSION@"
  license "Apache-2.0"

  # No bottles: the release archives *are* the binary, so the formula only
  # unpacks what the tagged workflow already built for each platform.
  on_macos do
    on_arm do
      url "https://github.com/apiplant/rlcd-rs/releases/download/v@VERSION@/rlcd-rs-v@VERSION@-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA_MACOS_ARM64@"
    end
  end
  on_linux do
    on_intel do
      url "https://github.com/apiplant/rlcd-rs/releases/download/v@VERSION@/rlcd-rs-v@VERSION@-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA_LINUX_X86_64@"
    end
    on_arm do
      url "https://github.com/apiplant/rlcd-rs/releases/download/v@VERSION@/rlcd-rs-v@VERSION@-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA_LINUX_ARM64@"
    end
  end

  def install
    bin.install "rlcd"
    doc.install "README.md"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/rlcd --version").split(" ").last
  end
end
