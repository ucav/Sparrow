class Sparrow < Formula
  desc "Local-first Rust agent cockpit — route, run, replay, rewind"
  homepage "https://github.com/ucav/Sparrow"
  version "0.5.4"
  license "MIT"

  # Only the arches that ship a prebuilt binary today are wired here.
  # Apple Intel (x86_64) and Linux aarch64 are not yet produced by the
  # release pipeline — those users should `cargo install sparrow-cli`.
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-macos-arm64"
      sha256 "82d04186b97cb238aae5890844893a0416e90fbe40d1f3816d707e8075d49efb"
    else
      odie "No prebuilt Intel-mac binary yet. Run: cargo install sparrow-cli"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-linux-x86_64"
      sha256 "d7b25225ce2445678ff7330c4d253cec590d6a290a0713946bdd1aec52453c2f"
    else
      odie "No prebuilt Linux aarch64 binary yet. Run: cargo install sparrow-cli"
    end
  end

  def install
    # The release asset is a bare single-file binary named sparrow-{os}-{arch}.
    bin_name = Dir["sparrow-*"].first || "sparrow"
    bin.install bin_name => "sparrow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/sparrow --version")
  end
end
