# Rusty Boards

Monorepo for ESP32 sensor boards and supporting services. KiCAD hardware
projects with `firmware/` subdirectories containing embedded Rust crates, plus
host-side services for data collection.

## Repository Structure

```
boards/<name>/pcb/          KiCAD hardware project
boards/<name>/firmware/     Embedded Rust crate
boards/<name>/flake.nix     Nix dev environment and checks
tools/<name>/               Host-side Rust services
lib/kicad/                  Shared KiCAD symbols, footprints, 3D models
```

Boards: `voltage-meter`, `temp-sensor`, `dummy-button`
Tools: `sensor-server`

## Hardware Platform

- Most boards: ESP32-C3 (RISC-V), target `riscv32imc-unknown-none-elf`, stable Rust
- dummy-button: ESP32-S3 (Xtensa), target `xtensa-esp32s3-none-elf`, espup toolchain
- Flash/debug: probe-rs
- Logging: defmt via defmt-rtt over JTAG; esp-println for serial Prometheus metrics

## Dev Environment

Each board and tool has its own Nix flake:

```sh
nix develop boards/voltage-meter    # firmware dev shell
nix develop tools/sensor-server     # host service dev shell
```

## Commands

| Command | Purpose |
|---------|---------|
| `cargo check` | Verify compilation (after every change) |
| `cargo clippy` | Lint (before commits) |
| `cargo fmt` | Format (before commits) |
| `nix flake check` | Run all Nix checks (clippy, fmt, DRC, ERC) |
