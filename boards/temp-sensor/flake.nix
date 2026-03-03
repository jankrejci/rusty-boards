{
  description = "DS18B20 temperature sensor board";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    crane,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
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
        targets = ["riscv32imc-unknown-none-elf"];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
      src = craneLib.cleanCargoSource ./firmware;
    in {
      checks = {
        # No buildDepsOnly: crane's dummy crate is incompatible with
        # no_std/no_main targets that use build-std. Clippy rebuilds deps
        # each time but is correct.
        clippy = craneLib.cargoClippy {
          inherit src;
          cargoArtifacts = null;
          cargoClippyExtraArgs = "-- -D warnings";
        };
        fmt = craneLib.cargoFmt {inherit src;};
      };

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
          echo "ESP32-C3 temp-sensor dev environment ready"
          echo ""
          echo "  cd firmware"
          echo "  cargo build --release       Build firmware"
          echo "  cargo run --release         Build, flash, and monitor via probe-rs"
          echo "  probe-rs list               List connected probes"
          echo ""
        '';
      };
    });
}
