# Temperature Sensor

ESP32-C3 board with DS18B20 OneWire temperature sensors. Reads sensor data and
exports Prometheus metrics over USB serial for collection by sensor-server.

## Architecture

- OneWire bus driven via ESP32-C3 RMT peripheral
- Embassy async tasks: sensor reading, metrics export, watchdog
- Metrics published via PubSub channel to serial output
- Output format: Prometheus text (`metric_name{labels} value timestamp_ms`)

## Dev Commands

```sh
nix develop                         # enter dev shell
cd firmware && cargo build --release  # build firmware
cd firmware && cargo run --release    # build, flash, and monitor via probe-rs
probe-rs list                         # list connected probes
```

## Checks

```sh
nix flake check     # clippy, fmt
```
