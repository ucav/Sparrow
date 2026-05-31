class Sparrow < Formula
  desc "The only CLI you install — single binary, any model, agentic coding"
  homepage "https://github.com/ucav/Sparrow"
  url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-macos-arm64"
  version "0.1.0"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-macos-arm64"
    else
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-macos-x86_64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-linux-aarch64"
    else
      url "https://github.com/ucav/Sparrow/releases/download/v#{version}/sparrow-linux-x86_64"
    end
  end

  def install
    bin.install "sparrow-#{OS.kernel_name.downcase}-#{Hardware::CPU.arch}" => "sparrow"
  end

  test do
    system "#{bin}/sparrow", "--version"
  end
end
