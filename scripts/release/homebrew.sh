#!/bin/sh
# Generate the Homebrew formula for a release.
# Usage: homebrew.sh TAG DIST_DIR OUTPUT_FILE

set -eu

tag="$1"
dist="$2"
output="$3"

sha() {
  awk '{print $1}' "$dist/octorun-$1.tar.gz.sha256"
}

mac_arm=$(sha aarch64-apple-darwin)
mac_intel=$(sha x86_64-apple-darwin)
linux_arm=$(sha aarch64-unknown-linux-gnu)
linux_intel=$(sha x86_64-unknown-linux-gnu)

cat > "$output" <<EOF
class Octorun < Formula
  desc "Create disposable self-hosted GitHub Actions runners"
  homepage "https://github.com/CHE3MZ/octorun"
  license "MIT"

  livecheck do
    url "https://github.com/CHE3MZ/octorun/releases/latest"
    strategy :github_latest
  end

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/CHE3MZ/octorun/releases/download/${tag}/octorun-aarch64-apple-darwin.tar.gz"
      sha256 "${mac_arm}"
    else
      url "https://github.com/CHE3MZ/octorun/releases/download/${tag}/octorun-x86_64-apple-darwin.tar.gz"
      sha256 "${mac_intel}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/CHE3MZ/octorun/releases/download/${tag}/octorun-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${linux_arm}"
    else
      url "https://github.com/CHE3MZ/octorun/releases/download/${tag}/octorun-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${linux_intel}"
    end
  end

  def install
    bin.install "octorun"
  end

  test do
    system "#{bin}/octorun", "--version"
  end
end
EOF