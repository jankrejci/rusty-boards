# sensor-server

HTTP server that bridges serial sensor metrics to Prometheus. Watches
`/dev/` for ESP32-C3 devices (VID `0x303a`, PID `0x1001`), reads
Prometheus-format metrics from their serial ports, and serves them on
`GET /metrics` for Prometheus scraping.

## Build

The flake produces a statically linked musl binary via
`pkgsStatic.rustPlatform.buildRustPackage`. This is the correct nixpkgs
approach for portable Linux binaries: `pkgsStatic` sets the target
platform to `x86_64-unknown-linux-musl` with `+crt-static`, and the
`buildRustPackage` hook picks up the target automatically. All C
dependencies (libudev-zero) are also built as static archives.

Note: `buildRustPackage`'s cargo hook hardcodes `--target` from
`stdenv.targetPlatform` at Nix evaluation time. Setting
`CARGO_BUILD_TARGET` as an env var has no effect. Similarly,
`pkgsCross.musl64.rustPlatform` produces dynamically-linked-against-musl
binaries, not static ones. Only `pkgsStatic` gets both right.

Build the Debian package:

```
cd sensor-server
nix build .#deb
```

The `.deb` is at `result/sensor-server_<version>_amd64.deb`. The version
is read from `Cargo.toml` automatically.

To build just the static binary without packaging:

```
nix build
```

The binary is at `result/bin/sensor-server`.

The `.deb` is assembled by [nfpm](https://nfpm.goreleaser.com/), a
package builder for deb/rpm/apk. The nfpm config is generated inline in
the flake with Nix store paths substituted for the binary, service file,
and maintainer scripts.

## Deploy

Install:

```
scp result/sensor-server_*.deb <host>:
ssh <host> sudo dpkg -i sensor-server_*.deb
```

The package installs:
- `/usr/local/bin/sensor-server` -- static binary
- `/lib/systemd/system/sensor-server.service` -- systemd unit

The postinst script runs `systemctl daemon-reload`, enables, and starts
the service.

Upgrade (same command, dpkg replaces the old version):

```
scp result/sensor-server_*.deb <host>:
ssh <host> sudo dpkg -i sensor-server_*.deb
```

Remove:

```
sudo dpkg -r sensor-server
```

Remove and purge config files:

```
sudo dpkg -P sensor-server
```

## Usage

Check status:

```
systemctl status sensor-server
```

Follow logs:

```
journalctl -fu sensor-server
```

Test the metrics endpoint:

```
curl http://localhost:8888/metrics
```

Prometheus scrape config:

```yaml
scrape_configs:
  - job_name: sensors
    static_configs:
      - targets: ['<host>:8888']
```

## CLI

```
sensor-server --listen 0.0.0.0:8888
```

`--listen` defaults to `0.0.0.0:8888`. Set `RUST_LOG` to control log
verbosity (`debug`, `info`, `warn`, `error`).

## Development

```
cd sensor-server
nix develop
cargo run
```

The default dev shell provides the rust-overlay toolchain with
rust-analyzer. Additional dev shells are available for manual static
builds outside of the Nix sandbox:

```
nix develop .#x86_64-static    # then: cargo build --release
nix develop .#aarch64-static   # then: cargo build --release
```

These set `CARGO_BUILD_TARGET` and the musl cross-compiler env vars.
This works in dev shells (where cargo reads the env var directly) but not
in `buildRustPackage` (which ignores it). Use `nix build` for
reproducible static builds.
