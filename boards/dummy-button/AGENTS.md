# Dummy Button

ESP32-S3 board with rotary encoder (PEC11R) and USB-C power management (TP5400).

## Hardware

- Rotary encoder: PEC11R-4215F-S0024 with push button
- Power: TP5400 USB-C charging and boost converter
- MCU: ESP32-S3-WROOM-1 (Xtensa)

## Architecture

- Xtensa toolchain via espup (not stable Rust)
- esp-hal 1.0.0 + esp-rtos 0.2.0
- defmt logging over RTT via defmt-rtt
- `rust-toolchain.toml` specifies `channel = "esp"`

## Dev Commands

```sh
nix develop                           # enter Xtensa firmware dev shell
nix develop .#hardware                # enter KiCAD hardware dev shell
cd firmware && cargo check            # type-check firmware
cd firmware && cargo build --release  # build firmware
cd firmware && cargo run --release    # flash and run via probe-rs
```

## Checks

```sh
nix flake check                       # DRC, ERC (hardware only)
cd firmware && cargo clippy           # lint firmware (requires nix develop)
cd firmware && cargo fmt --check      # format check
```
