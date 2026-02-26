---
name: flash
description: Build and flash firmware to connected ESP32-C3 hardware via probe-rs
user_invocable: true
arguments: "[crate-dir]"
allowed-tools: Bash, Read
---

# Flash Skill

Build and flash firmware to the connected ESP32-C3 dev board.

## Usage

```
/flash                     # Flash from current firmware directory
/flash temp-sensor         # Flash specific board firmware
```

## Pre-Flash Checklist

1. **Verify hardware connection**:
   ```bash
   probe-rs list
   ```
   Expect to see ESP32-C3 on /dev/ttyACM0.

2. **Build firmware**:
   ```bash
   cargo build --release
   ```
   Must succeed before flashing.

3. **Check for errors**:
   ```bash
   cargo check
   ```

## Flash Process

From the firmware crate directory:

```bash
cargo run --release
```

This uses the probe-rs runner configured in `.cargo/config.toml` which:
- Flashes the binary to the ESP32-C3
- Starts RTT output for defmt logging
- Catches hardfaults and prints stack traces

## Monitoring Only

To monitor RTT output without reflashing:
```bash
probe-rs run --chip esp32c3 target/riscv32imc-unknown-none-elf/release/<binary-name>
```

## Timeout Wrapper

For automated testing, wrap with timeout to capture initial output:
```bash
timeout 15s cargo run --release 2>&1
```

## Troubleshooting

- **"No probe found"**: Check USB cable and /dev/ttyACM0 permissions
- **"Connecting to target failed"**: Hold BOOT button while connecting, or reset the board
- **Stale defmt output**: Clean and rebuild: `cargo clean && cargo build --release`

## Rules

- ALWAYS verify probe-rs can see the device before flashing
- NEVER flash without a successful build first
- Show RTT output to user after flashing so they can verify behavior
