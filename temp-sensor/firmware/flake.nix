{
  description = "ESP32-C3 Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "rustfmt"
            "llvm-tools"
          ];
          targets = [ "riscv32imc-unknown-none-elf" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            probe-rs-tools
            espflash
            pkg-config
            libudev-zero
            picocom
            usbutils
          ];

          RUST_ANALYZER_SERVER_PATH = "${rustToolchain}/bin/rust-analyzer";

          shellHook = ''
            echo "ESP32-C3 Rust dev environment ready"
            echo ""
            echo "  cargo build --release       Build firmware"
            echo "  cargo run --release         Build, flash, and monitor via probe-rs"
            echo "  probe-rs list               List connected probes"
            echo ""
          '';
        };
      }
    );
}
