# Temperature Sensor

ESP32-C3 board with DS18B20 OneWire temperature sensors. Reads sensor data and
exports Prometheus metrics over USB serial for collection by sensor-server.

## Architecture

- OneWire bus driven via ESP32-C3 RMT peripheral
- Embassy async tasks: sensor reading, metrics export, watchdog
- Metrics published via PubSub channel to serial output
- Output format: Prometheus text (`metric_name{labels} value timestamp_ms`)
