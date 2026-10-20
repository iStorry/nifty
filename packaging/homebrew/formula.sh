#!/usr/bin/env bash
# Prints the Homebrew formula for a release: formula.sh <version> <macOS archive>
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version> <macos-archive>" >&2
  exit 2
fi

version=$1
archive=$2
repo=${GITHUB_REPOSITORY:-istorry/nifty}
sha256=$(shasum -a 256 "$archive" | cut -d ' ' -f 1)

cat <<EOF
class NiftyCli < Formula
  desc "Build native desktop apps with the Nifty language"
  homepage "https://github.com/${repo}"
  url "https://github.com/${repo}/releases/download/v${version}/$(basename "$archive")"
  version "${version}"
  sha256 "${sha256}"
  license any_of: ["MIT", "Apache-2.0"]

  depends_on :macos

  def install
    # `nf build` looks for the runtime library it links apps against next to itself.
    bin.install "nf", "libnifty_rt.a"
  end

  test do
    assert_match "nf #{version}", shell_output("#{bin}/nf --version")
  end
end
EOF
