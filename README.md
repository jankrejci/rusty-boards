# Rusty Boards

Monorepo for ESP32 sensor boards and supporting services. KiCAD hardware
designs with embedded Rust firmware, plus a host-side metrics server.

Boards export Prometheus metrics over USB serial. The sensor-server collects
them and serves a combined `/metrics` endpoint for Prometheus scraping.

## Repository structure

```
boards/
  voltage-meter/        Voltage + temp + LCD board
    pcb/                KiCAD schematic and PCB
    firmware/           Embedded Rust (esp-hal, embassy)
  temp-sensor/          DS18B20 temperature sensor board
    firmware/           Embedded Rust (esp-hal, embassy)
  dummy-button/         Rotary encoder + USB-C power board
    pcb/                KiCAD schematic and PCB
    firmware/           Embedded Rust (esp-hal, embassy)
tools/
  sensor-server/        Host-side HTTP server bridging serial to Prometheus
lib/
  kicad/                Shared KiCAD symbols, footprints, and 3D models
```

## Boards

### voltage-meter

ADC voltage measurement with calibration curve and Kalman filter, LM75
temperature sensor over I2C, ST7735s 160x80 LCD display over SPI. Embassy
async tasks for reading, display, metrics export, and watchdog.

### temp-sensor

DS18B20 OneWire temperature sensors driven via the ESP32-C3 RMT peripheral.
Embassy async tasks for sensor reading, metrics export, and watchdog.

### dummy-button

Rotary encoder (PEC11R) with push button and TP5400 USB-C charging/boost
converter. ESP32-S3 (Xtensa) with hello world firmware.

## Tools

### sensor-server

HTTP server that auto-detects ESP32-C3 boards on USB (VID `0x303a`, PID
`0x1001`), reads Prometheus-formatted metrics from serial, and serves them on
`GET /metrics`. Uses inotify for instant device hot-plug detection.

## Hardware platform

Most boards use ESP32-C3 (RISC-V). dummy-button uses ESP32-S3 (Xtensa).

- ESP32-C3: target `riscv32imc-unknown-none-elf`, stable Rust, `build-std = ["core"]`
- ESP32-S3: target `xtensa-esp32s3-none-elf`, espup Xtensa toolchain
- Dev board: ESP32-C3 Super Mini, USB-JTAG on `/dev/ttyACM0`
- Flash/debug: probe-rs

## Development

Each project has its own nix flake with dev shells and checks.

```sh
cd boards/temp-sensor && nix develop     # firmware dev shell
cd boards/voltage-meter && nix develop   # firmware dev shell
cd boards/dummy-button && nix develop    # Xtensa firmware dev shell
cd boards/dummy-button && nix develop .#hardware  # KiCAD shell
cd tools/sensor-server && nix develop    # Rust dev shell
```

### Firmware workflow

```sh
cd boards/temp-sensor
nix develop
cd firmware
cargo build --release       # build
cargo run --release         # build + flash + monitor via probe-rs
```

### Static builds (sensor-server)

```sh
cd tools/sensor-server
nix develop .#x86_64-static     # x86_64 musl
nix develop .#aarch64-static    # aarch64 musl cross
cargo build --release
```

### Checks

Each flake defines checks that run with `nix flake check`:

| Project | Checks |
|---------|--------|
| `tools/sensor-server` | clippy, fmt, test |
| `boards/temp-sensor` | clippy, fmt |
| `boards/voltage-meter` | clippy, fmt, DRC, ERC |
| `boards/dummy-button` | DRC, ERC |

Cargo checks use [crane](https://github.com/ipetkov/crane). KiCAD checks use
`kicad-cli` for design rule checking (DRC) and electrical rule checking (ERC).

## Metrics format

All boards emit Prometheus text format over USB serial:

```
metric_name{label="value"} 42.0 1709500000000
```

The sensor-server validates, timestamps, and aggregates these lines for
Prometheus scraping.
