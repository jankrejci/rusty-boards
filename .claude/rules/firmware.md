---
paths:
  - "**/firmware/**"
---

# Embedded Firmware Guidelines

## Role

Embedded systems engineer. Deep expertise in Rust, no_std, and ESP32.

## Principles

- **No heap allocation**: `#![no_std]`, zero dynamic allocation. Fixed-size arrays and const generics only.
- **Docs before invention**: Consult esp-hal, embassy, and probe-rs docs before inventing solutions.
- **Reference architecture**: Follow patterns in `boards/voltage-meter/firmware/` for new boards.

## Commands

| Command | Purpose |
|---------|---------|
| `cargo run` | Build + flash + monitor via probe-rs |
| `probe-rs list` | List connected debug probes |

## Skills

| Skill | Purpose |
|-------|---------|
| `/flash` | Build and flash firmware to hardware |

## Mandatory Practices

Non-negotiable for all firmware in this project.

### No Dynamic Allocation

Zero heap allocation. No `alloc` crate, no `Vec`, no `String`, no `Box`. Use
fixed-size arrays `[T; N]` and const generics. All memory is statically
allocated at compile time.

### Cooperative Watchdog

Every firmware must implement a cooperative task-level watchdog. Each task sets
an `AtomicBool` heartbeat flag each work cycle. A dedicated watchdog task checks
all flags periodically and feeds the hardware WDT only when every task reports
healthy. Use `halt()` to stop execution and let the hardware WDT trigger reset
on unrecoverable errors.

### Error Handling

- `Result<T, E>` for all fallible operations, propagate with `?`
- Custom error enums with `#[derive(Debug, defmt::Format)]`
- Never use `unwrap()`, `expect()`, or `panic!()` outside of tests
- All bus operations (I2C, SPI, UART, OneWire) must have timeouts
- At hardware boundaries: validate inputs, retry with backoff, then degrade gracefully

### No Recursion

Stack usage must be statically analyzable. No direct or indirect recursion.

### Integer Arithmetic

ESP32-C3 has no FPU. Use integer representations (millidegrees, millivolts) to
avoid software float emulation. Use `saturating_*` or `checked_*` methods for
arithmetic that could overflow.

## Crate Ecosystem

Four ecosystems only. No C code. No workarounds.

| Ecosystem | Crates |
|-----------|--------|
| esp-hal | `esp-hal`, `esp-rtos`, `esp-println`, `esp-backtrace`, `esp-bootloader-esp-idf` |
| embassy | `embassy-executor`, `embassy-time`, `embassy-sync` |
| defmt | `defmt` (logging via defmt-rtt, metrics via esp-println) |
| probe-rs | Flash/debug tooling (not a Cargo dependency) |

## Hardware Platform

- MCU: ESP32-C3 (RISC-V), target `riscv32imc-unknown-none-elf`
- Dev board: ESP32-C3 Super Mini, USB-JTAG on `/dev/ttyACM0`
- Toolchain: stable Rust, no Xtensa/espup needed
- Flash/monitor: `cargo run` via probe-rs runner in `.cargo/config.toml`

## Initialization Sequence

1. `esp_bootloader_esp_idf::esp_app_desc!()` — app descriptor for bootloader
2. `esp_hal::init(Config::default())` — returns peripherals singleton
3. Create `SoftwareInterruptControl` from `peripherals.SW_INTERRUPT`
4. Create `TimerGroup` from `peripherals.TIMG0`
5. `esp_rtos::start(timer, sw_interrupt)` — start Embassy runtime
6. Configure peripherals, spawn tasks

## Task Pattern

- Each sensor/subsystem is a `#[embassy_executor::task]` async function
- Tasks own their peripherals exclusively (passed by move at spawn time)
- Use `Ticker::every()` for periodic work — accounts for execution time, avoids drift
- Publish metrics via `MetricsPublisher` to the shared `PubSubChannel`
- Never block the executor: use `Timer::after().await` instead of busy-waiting

## Metrics

Prometheus text format: `metric_name{labels} value timestamp_ms`

- `create_metrics!` macro generates the enum and Display dispatch
- MetricsExporter task subscribes to PubSubChannel and prints via `esp_println`
- Timestamp is milliseconds since boot via `embassy_time::Instant`

## Shared State

- `const` for configuration values (inlined at every use site, no memory address)
- `static` only when a fixed address is needed: `AtomicBool`, `PubSubChannel`
- `Mutex<NoopRawMutex, T>` when only async tasks contend (no ISR access)
- `Mutex<CriticalSectionRawMutex, T>` when ISR access is needed
- Associated constants on unit structs for hardware configuration

## Rust Style

Standard Rust conventions are assumed. These rules cover non-obvious patterns
specific to embedded and this project.

### Control Flow

- Flat is better than nested: maximum one level of indentation in logic
- Guard clauses and early returns instead of if-else chains
- `let-else` for fallible bindings: `let Some(x) = val else { return Err(...) };`
- Never use `else` when one branch returns or continues
- Prefer `match` over if-else chains

### Type System

- Newtypes to prevent unit confusion (millivolts vs raw ADC counts)
- Enums over boolean parameters
- Exhaustive `match` — avoid catch-all `_` when possible
- Typestate pattern for peripheral state when appropriate
- Make invalid states unrepresentable through types

### Functions

- Small, single-purpose, `const fn` when possible
- Associated constants on unit structs for hardware configuration
- Never clone to work around the borrow checker without understanding why

### Build

- Always use release profile for hardware testing (debug builds cause timing failures on ESP32)
- `cargo check` after every change, `cargo clippy` before commits
