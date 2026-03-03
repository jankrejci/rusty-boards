# Voltage Meter

ESP32-C3 board with voltage measurement, LM75 temperature sensor, and ST7735s
LCD display. Reads analog voltage via ADC with Kalman filtering and exports
Prometheus metrics over USB serial.

## Architecture

- ADC voltage reading with calibration curve and Kalman filter
- LM75 temperature sensor over I2C
- ST7735s 160x80 LCD display over SPI
- Embassy async tasks: ADC reader, LM75 reader, display, metrics export, watchdog
- Metrics published via PubSub channel to serial output
- Output format: Prometheus text (`metric_name{labels} value timestamp_ms`)

## Pin Assignments

| Peripheral | Pins |
|------------|------|
| ADC | GPIO0 |
| I2C (LM75) | SDA: GPIO8, SCL: GPIO9 |
| SPI (Display) | SCK: GPIO4, MOSI: GPIO6 |
| Display control | DC: GPIO5, RST: GPIO7, CS: GPIO10 |

## Dev Commands

```sh
nix develop                         # enter dev shell
cd firmware && cargo build --release  # build firmware
cd firmware && cargo run --release    # build, flash, and monitor via probe-rs
probe-rs list                         # list connected probes
```

## Checks

```sh
nix flake check     # clippy, fmt, DRC, ERC
```

## Notes

- Uses older esp-hal 1.0.0-rc.0 with esp-hal-embassy (not yet migrated to esp-rtos)
- Peripherals initialized in main() and passed to tasks by move
