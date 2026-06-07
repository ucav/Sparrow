class Sparrow < Formula
  desc "Local-first Rust agent cockpit — route, run, replay, rewind"
  homepage "https://github.com/ucav/Sparrow"
  version "0.5.3"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-macos-arm64"
      sha256 "REPLACE_WITH_SHA256_AFTER_RELEASE_MACOS_ARM64"
    else
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-macos-x86_64"
      sha256 "REPLACE_WITH_SHA256_AFTER_RELEASE_MACOS_X64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-linux-aarch64"
      sha256 "REPLACE_WITH_SHA256_AFTER_RELEASE_LINUX_ARM64"
    else
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-linux-x86_64"
      sha256 "REPLACE_WITH_SHA256_AFTER_RELEASE_LINUX_X64"
    end
  end

  def install
    # All assets are bare single-file binaries; the downloaded file name varies
    # by platform/arch (`sparrow-{os}-{arch}`) — pick whichever exists.
    bin_name = Dir["sparrow-*"].first || "sparrow"
    bin.install bin_name => "sparrow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/sparrow --version")
  end
end
