class Jki < Formula
  desc "Extreme speed MFA & Identity Session Manager for CLI Power Users"
  homepage "https://github.com/creart-tw/just-keep-identity"
  version "0.1.0-alpha"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/creart-tw/just-keep-identity/releases/download/v0.1.0-alpha/jki-macos-arm64.tar.gz"
    sha256 "578c71a978fdeb82cd1b21050f760473891fd624c05114b489db4ff1d747e632"
  else
    # Fallback to source build for other architectures
    url "https://github.com/creart-tw/just-keep-identity/archive/refs/tags/v0.1.0-alpha.tar.gz"
    sha256 "REPLACE_WITH_SOURCE_SHA256"
    depends_on "rust" => :build
  end

  def install
    if OS.mac? && Hardware::CPU.arm?
      # Install pre-built binaries
      bin.install "jki"
      bin.install "jkim"
      bin.install "jki-agent"
    else
      # Build from source
      system "cargo", "build", "--release", "--workspace"
      bin.install "target/release/jki"
      bin.install "target/release/jkim"
      bin.install "target/release/jki-agent"
    end
  end

  def caveats
    <<~EOS
      JKI consists of three components:
        - jki: The search and OTP generator.
        - jkim: The management hub (vault, git, config).
        - jki-agent: The background security agent.

      To start the background agent, run:
        jkim agent start

      It is recommended to add the agent to your login items for the best experience.
    EOS
  end

  test do
    # Simple version check to verify installation
    assert_match "jki #{version}", shell_output("#{bin}/jki --version")
    assert_match "jkim #{version}", shell_output("#{bin}/jkim --version")
  end
end
