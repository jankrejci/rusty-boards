{
  description = "Dummy button board";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = ["rust-src" "rust-analyzer"];
      };

      patchEspBinaries = pkgs.writeShellScriptBin "patch-esp-binaries" ''
        if [ -f /etc/NIXOS ] || [ ! -f /lib64/ld-linux-x86-64.so.2 ]; then
          echo "Patching ESP toolchain binaries for NixOS..."
          find "$RUSTUP_HOME/toolchains/esp" -type f | while read -r file; do
            if file "$file" 2>/dev/null | grep -q 'ELF'; then
              ${pkgs.patchelf}/bin/patchelf \
                --set-interpreter "$(cat ${pkgs.stdenv.cc}/nix-support/dynamic-linker)" \
                "$file" 2>/dev/null || true
              ${pkgs.patchelf}/bin/patchelf \
                --set-rpath "${pkgs.lib.makeLibraryPath [
            pkgs.stdenv.cc.cc.lib
            pkgs.glibc
            pkgs.zlib
            pkgs.libxml2
            pkgs.openssl
            pkgs.libffi
          ]}:$RUSTUP_HOME/toolchains/esp/lib:$(dirname "$file")" \
                "$file" 2>/dev/null || true
            fi
          done
          echo "Patching complete."
        fi
      '';
    in {
      checks = {
        drc = pkgs.runCommand "drc" {
          nativeBuildInputs = [pkgs.kicad-small];
        } ''
          export HOME=$(mktemp -d)
          kicad-cli pcb drc \
            --format json \
            --output $out \
            ${./pcb/dummy-button.kicad_pcb}
        '';

        erc = pkgs.runCommand "erc" {
          nativeBuildInputs = [pkgs.kicad-small];
        } ''
          export HOME=$(mktemp -d)
          kicad-cli sch erc \
            --severity-error \
            --format json \
            --output $out \
            ${./pcb/dummy-button.kicad_sch}
        '';
      };

      devShells = {
        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            espup
            espflash
            probe-rs-tools
            patchelf
            patchEspBinaries
            pkg-config
            libudev-zero
            perl
            stdenv.cc.cc.lib
            glibc
            zlib
            libxml2
            openssl
            libffi
          ];

          NIX_LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
            pkgs.stdenv.cc.cc.lib
            pkgs.glibc
            pkgs.zlib
            pkgs.libxml2
            pkgs.openssl
            pkgs.libffi
          ];
          NIX_LD = "${pkgs.glibc}/lib/ld-linux-x86-64.so.2";

          RUST_ANALYZER_SERVER_PATH = "${rustToolchain}/bin/rust-analyzer";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          shellHook = ''
            export RUSTUP_HOME="$PWD/.rustup"
            export CARGO_HOME="$PWD/.cargo"
            mkdir -p "$RUSTUP_HOME" "$CARGO_HOME"

            # These paths contain version fragments that correspond to the espup
            # toolchain version below. When updating --toolchain-version, verify
            # the installed paths match by running: ls $RUSTUP_HOME/toolchains/esp/
            XTENSA_GCC_PATH="$RUSTUP_HOME/toolchains/esp/xtensa-esp-elf/esp-14.2.0_20240906/xtensa-esp-elf/bin"
            LIBCLANG_TOOLCHAIN_PATH="$RUSTUP_HOME/toolchains/esp/xtensa-esp32-elf-clang/esp-19.1.2_20250225/esp-clang/lib"

            if [ ! -d "$XTENSA_GCC_PATH" ] || [ ! -d "$LIBCLANG_TOOLCHAIN_PATH" ]; then
              echo "Installing ESP32-S3 Xtensa toolchain..."
              if ! espup install --targets esp32s3 --toolchain-version 1.88.0; then
                echo "ERROR: espup install failed" >&2
                return 1
              fi
              patch-esp-binaries
            fi

            export PATH="$XTENSA_GCC_PATH:$RUSTUP_HOME/toolchains/esp/bin:$CARGO_HOME/bin:$PATH"

            if [ -d "$LIBCLANG_TOOLCHAIN_PATH" ]; then
              export LIBCLANG_PATH="$LIBCLANG_TOOLCHAIN_PATH"
            fi
          '';
        };

        hardware = pkgs.mkShell {
          packages = [pkgs.kicad-small];
        };
      };
    });
}
