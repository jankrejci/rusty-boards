# miner-scraper

Prometheus metrics exporter for Bitcoin ASIC miners. Scrapes hardware metrics via the cgminer API and serves them in Prometheus text exposition format over HTTP.

Supported hardware: Antminer S21 family (S21, S21 XP, S21 Pro).

Supported firmwares: Stock, BraiinsOS, LuxOS, Vnish, MARA. Firmware is auto-detected per host.

Scraped metrics: hashrate, temperatures (PCB/chip/PIC), fan speeds, frequencies, hardware errors, pool stats, chip status.

## Build

Requires Nix with flakes enabled.

Static x86_64 binary:
```sh
nix build ./tools/miner-scraper
```

Static aarch64 binary (cross-compile from x86_64):
```sh
nix build ./tools/miner-scraper --system aarch64-linux
```

Debian package:
```sh
nix build ./tools/miner-scraper#deb
```

The `.deb` package is in `result/`.

## Install

### Debian package

```sh
sudo dpkg -i result/miner-scraper-*.deb
```

This installs:
- `/usr/local/bin/miner-scraper`
- `/lib/systemd/system/miner-scraper.service`
- `/etc/miner-scraper/config.toml`

The service is enabled and started automatically after install.

### Manual

```sh
cp result/bin/miner-scraper /usr/local/bin/
cp miner-scraper.service /lib/systemd/system/
mkdir -p /etc/miner-scraper
cp config.toml /etc/miner-scraper/

systemctl daemon-reload
systemctl enable --now miner-scraper
```

## Uninstall

```sh
sudo dpkg -r miner-scraper
```

This stops the service and removes the binary and service file. The config file
in `/etc/miner-scraper/` is preserved. To remove everything including config:

```sh
sudo dpkg -P miner-scraper
```

## Configuration

Config file location: `/etc/miner-scraper/config.toml`.

```toml
listen = "0.0.0.0:8889"
scrape_interval_secs = 15
targets = ["10.0.0.1", "10.0.0.2"]
```

| Field | Default | Description |
|-------|---------|-------------|
| `listen` | `127.0.0.1:8889` | Address and port for the HTTP server |
| `scrape_interval_secs` | `5` | Seconds between scrape cycles |
| `targets` | `[]` | List of miner IP addresses to scrape |

Changes to `targets` and `scrape_interval_secs` are picked up automatically without
restarting the service. Changing `listen` requires a restart.

## CLI

```
miner-scraper [OPTIONS]

Options:
  --config <PATH>     Path to config file [default: /etc/miner-scraper/config.toml]
  --ip <IP>           Listen IP address (overrides config)
  --targets <IP>...   Miner IPs to scrape (overrides config)
```

CLI arguments take precedence over the config file.

## Usage

```sh
# Check that the service is running
systemctl status miner-scraper

# Fetch metrics
curl http://localhost:8889/metrics
```

Example output:
```
stats_ghs_5s{host="10.0.0.1",idx="0"} 275629 1710374159000
stats_fan{host="10.0.0.1",idx="1"} 3540 1710374159000
stats_temp_pcb{host="10.0.0.1",hashboard="1",idx="0"} 43 1710374159000
```

## Logs

The service logs to the systemd journal. View logs with:

```sh
# Recent logs
sudo journalctl -u miner-scraper

# Follow logs in real time
sudo journalctl -u miner-scraper -f

# Logs since last boot
sudo journalctl -u miner-scraper -b

# Only errors and warnings
sudo journalctl -u miner-scraper -p warning
```

Log level is controlled by the `RUST_LOG` environment variable in the service file. Default is `info`. To change it, override the service:

```sh
sudo systemctl edit miner-scraper
```

Add:
```ini
[Service]
Environment=RUST_LOG=debug
```

Then restart:
```sh
sudo systemctl restart miner-scraper
```

## Development

```sh
nix develop ./tools/miner-scraper   # Enter dev shell
cargo test                          # Run tests (from miner-scraper/)
nix flake check ./tools/miner-scraper  # Run clippy, fmt, and tests
```
