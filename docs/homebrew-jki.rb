class Jki < Formula
  desc "High-speed MFA & Identity Session Manager for CLI Power Users"
  homepage "https://github.com/lichih/just-keep-identity"
  version "0.1.0-alpha"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/lichih/just-keep-identity/releases/download/v#{version}/jki-macos-arm64.tar.gz"
    sha256 "05061149f364c6ac3dfdda3792e44a863bbef35c82fb8f667ad1c95f4facc80d" # TODO: Update after release
  else
    # Fallback to source build for other architectures
    url "https://github.com/lichih/just-keep-identity/archive/refs/tags/v#{version}.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000" # TODO: Update after release
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
      system "cargo", "install", *std_cargo_args(path: "crates/jki")
      system "cargo", "install", *std_cargo_args(path: "crates/jkim")
      system "cargo", "install", *std_cargo_args(path: "crates/jki-agent")
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
