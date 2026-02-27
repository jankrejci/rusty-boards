# Sensor Server

Host-side HTTP server bridging serial sensor data to Prometheus. Reads
Prometheus metrics from ESP32 sensors over USB serial and serves them via
HTTP for Prometheus scraping.

## Role

Backend systems engineer. Standard Rust with tokio async runtime.

## Tech Stack

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime |
| axum | HTTP server (`GET /metrics`) |
| serialport | Port enumeration and serial reading |
| inotify | Device connect/disconnect detection |
| tokio-util | CancellationToken for reader lifecycle |
| clap | CLI arguments |
| log + env_logger | Logging |
| anyhow | Error handling |

## Architecture

Watches `/dev/` via inotify for instant device detection, with a 60-second
fallback poll. Each detected ESP32-C3 port (VID `0x303a`, PID `0x1001`) gets
a blocking reader task that sends validated, wall-clock-timestamped metric
lines through a channel to the metrics store. The HTTP handler concatenates
all stored metrics on `GET /metrics`.

## Principles

- Standard Rust idioms with std library
- `anyhow::Result` for error propagation
- No panics in production code (no `unwrap()` on fallible operations)
- Validate Prometheus metric format before storing

## Commands

| Command | Purpose |
|---------|---------|
| `cargo run` | Start the server |
| `cargo test` | Run tests |
