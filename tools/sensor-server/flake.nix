{
  description = "Sensor server";

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

      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      version = cargoToml.package.version;

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = ["rust-src" "rust-analyzer" "rustfmt"];
        targets = [
          "x86_64-unknown-linux-musl"
          "aarch64-unknown-linux-musl"
        ];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
      src = craneLib.cleanCargoSource ./.;

      commonArgs = {
        inherit src;
        nativeBuildInputs = [pkgs.pkg-config];
        buildInputs = [pkgs.libudev-zero];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      x86MuslCC = pkgs.pkgsCross.musl64.stdenv.cc;
      x86MuslLibudev = pkgs.pkgsCross.musl64.libudev-zero;

      aarch64MuslCC = pkgs.pkgsCross.aarch64-multiplatform-musl.stdenv.cc;
      aarch64MuslLibudev = pkgs.pkgsCross.aarch64-multiplatform-musl.libudev-zero;

      staticPkgs = pkgs.pkgsStatic;

      isLinux = pkgs.lib.hasSuffix "-linux" system;

      debArch = {
        "x86_64-linux" = "amd64";
        "aarch64-linux" = "arm64";
      };
    in {
      checks = {
        clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "-- -D warnings";
          });
        fmt = craneLib.cargoFmt {inherit src;};
        test = craneLib.cargoTest (commonArgs // {inherit cargoArtifacts;});
      } // pkgs.lib.optionalAttrs isLinux {
        package = self.packages.${system}.default;
        deb = self.packages.${system}.deb;
      };

      packages = pkgs.lib.optionalAttrs isLinux {
        default = staticPkgs.rustPlatform.buildRustPackage {
          pname = "sensor-server";
          inherit version;
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [staticPkgs.pkg-config];
          buildInputs = [staticPkgs.libudev-zero];
        };

        deb = let
          bin = self.packages.${system}.default;

          postinst = pkgs.writeScript "postinst" ''
            #!/bin/sh
            set -e
            systemctl daemon-reload
            systemctl enable sensor-server
            systemctl restart sensor-server || true
          '';

          prerm = pkgs.writeScript "prerm" ''
            #!/bin/sh
            set -e
            systemctl stop sensor-server || true
          '';

          postrm = pkgs.writeScript "postrm" ''
            #!/bin/sh
            set -e
            systemctl daemon-reload
          '';

          config = pkgs.writeText "nfpm.yaml" ''
            name: sensor-server
            version: "${version}"
            arch: ${debArch.${system} or (throw "unsupported deb arch: ${system}")}
            maintainer: jkr
            description: |
              Sensor metrics server.
              Bridges serial sensor data to Prometheus over HTTP.
            contents:
              - src: ${bin}/bin/sensor-server
                dst: /usr/local/bin/sensor-server
              - src: ${./sensor-server.service}
                dst: /lib/systemd/system/sensor-server.service
            scripts:
              postinstall: ${postinst}
              preremove: ${prerm}
              postremove: ${postrm}
          '';
        in
          pkgs.runCommand "sensor-server-${version}.deb" {
            nativeBuildInputs = [pkgs.nfpm];
          } ''
            mkdir -p $out
            nfpm package --config ${config} --packager deb --target $out/
          '';
      };

      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustToolchain
          pkg-config
          libudev-zero
        ];

        RUST_ANALYZER_SERVER_PATH = "${rustToolchain}/bin/rust-analyzer";

        shellHook = ''
          echo "Sensor server dev environment ready"
          echo ""
          echo "  cargo run                   Start the server"
          echo "  cargo test                  Run tests"
          echo ""
          echo "Packages:"
          echo "  nix build                   Build static x86_64 binary"
          echo "  nix build .#deb             Build Debian package"
          echo ""
          echo "Static build shells:"
          echo "  nix develop .#x86_64-static   x86_64 musl static binary"
          echo "  nix develop .#aarch64-static  aarch64 musl cross binary"
          echo ""
        '';
      };

      devShells.x86_64-static = pkgs.mkShell {
        nativeBuildInputs = [
          rustToolchain
          pkgs.pkg-config
        ];

        CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
        CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "${x86MuslCC}/bin/${x86MuslCC.targetPrefix}cc";
        CC_x86_64_unknown_linux_musl = "${x86MuslCC}/bin/${x86MuslCC.targetPrefix}cc";
        PKG_CONFIG_PATH = "${x86MuslLibudev}/lib/pkgconfig";

        shellHook = ''
          echo "x86_64 musl static build environment"
          echo ""
          echo "  cargo build --release       Build static x86_64 binary"
          echo ""
        '';
      };

      devShells.aarch64-static = pkgs.mkShell {
        nativeBuildInputs = [
          rustToolchain
          pkgs.pkg-config
        ];

        CARGO_BUILD_TARGET = "aarch64-unknown-linux-musl";
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER = "${aarch64MuslCC}/bin/${aarch64MuslCC.targetPrefix}cc";
        CC_aarch64_unknown_linux_musl = "${aarch64MuslCC}/bin/${aarch64MuslCC.targetPrefix}cc";
        PKG_CONFIG_PATH = "${aarch64MuslLibudev}/lib/pkgconfig";
        PKG_CONFIG_ALLOW_CROSS = "1";

        shellHook = ''
          echo "aarch64 musl cross build environment"
          echo ""
          echo "  cargo build --release       Build static aarch64 binary"
          echo ""
        '';
      };
    });
}
