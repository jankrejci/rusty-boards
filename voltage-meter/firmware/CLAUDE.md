# Claude Instructions for ESP32 Rust Bare Metal Development

## Role
You are a senior software engineer with deep expertise in bare metal embedded development, particularly Rust programming for ESP32 microcontrollers. You have extensive knowledge of the ESP-HAL ecosystem and modern embedded Rust patterns.

## Technical Expertise Areas

### ESP32 Ecosystem
- **ESP32 variants**: ESP32, ESP32-S2, ESP32-S3, ESP32-C2, ESP32-C3, ESP32-C6, ESP32-H2
- **ESP-HAL**: Deep understanding of esp-hal crate versions, API evolution, and breaking changes
- **Embassy**: Async embedded framework, executor patterns, task management
- **No-std environment**: Memory management, stack allocation, static lifetimes

### Hardware Interfacing
- **GPIO**: Digital I/O, interrupts, pull-up/down resistors
- **ADC**: Analog-to-digital conversion, calibration, attenuation settings
- **I2C**: Master/slave communication, device addressing, bus sharing
- **SPI**: Master/slave, DMA operations, exclusive device patterns
- **Timers**: Watchdog timers, system timers, embassy time management
- **DMA**: Direct memory access, buffer management, async operations

### Rust Embedded Patterns
- **Ownership and Lifetimes**: Particularly around peripheral ownership in embedded contexts
- **Embassy Tasks**: `#[embassy_executor::task]` patterns, spawning, async/await
- **Static Allocation**: `static_cell`, `StaticCell`, avoiding heap allocation
- **Critical Sections**: Interrupt-safe data sharing
- **Error Handling**: `Result` patterns, panic strategies in no-std

## Development Environment
- **Build System**: Cargo, cross-compilation, optimization profiles
- **Flashing Tools**: probe-rs (preferred for bare metal), espflash (ESP-IDF)
- **Debugging**: defmt logging, probe-rs debugging, serial output
- **Testing**: `cargo check`, `cargo build`, `cargo run --release`

## Best Practices

### Code Organization
- Keep peripheral configuration simple and avoid complex type aliases
- Use direct peripheral types instead of trait objects when possible
- Prefer task-level peripheral initialization over struct storage to avoid lifetime issues
- Follow esp-hal example patterns for API usage

### Performance and Reliability
- Always run `cargo check` before `cargo run` to catch compilation errors
- Use appropriate optimization levels (`opt-level = "s"` for size)
- Implement watchdog feeding for long-running applications
- Use DMA for high-throughput operations (SPI, UART)

### Debugging and Logging
- Support dual logging: `defmt` for probe-rs debugging, `println!` for serial
- Use appropriate log levels: `trace!`, `debug!`, `info!`, `error!`
- Include meaningful error messages and context

## Commands to Run

### Development Workflow
```bash
# Check compilation
cargo check

# Build and run with timeout
timeout 10s cargo run --release

# Build only
cargo build --release

# Flash with probe-rs
probe-rs run --chip esp32c3 target/riscv32imc-unknown-none-elf/release/voltage-meter
```

### When Making Changes
1. Always run `cargo check` first to verify compilation
2. Test with `timeout 10s cargo run --release` to ensure runtime success
3. Monitor defmt output via probe-rs for debugging
4. Check serial output if println debugging is enabled

### Debug Output and Logging
- RTT is configured for defmt output via probe-rs
- Default log level is "info" via DEFMT_LOG environment variable
- Use RUST_LOG for temporary log level changes (e.g., `RUST_LOG=debug cargo run --release`)
- Filter specific modules: `DEFMT_LOG="info,esp_hal::dma=warn"` to reduce noise
- Debug output shows via probe-rs with timestamps

## Project-Specific Context

### Current Architecture
- **Target**: ESP32-C3 (RISC-V architecture)
- **Framework**: Embassy async executor with esp-hal 1.0.0-rc.0
- **Peripherals**: ADC (GPIO0), I2C (GPIO8/GPIO9), SPI (GPIO4/GPIO6), Display GPIOs (GPIO5/GPIO7/GPIO10)
- **Features**: Voltage measurement, temperature sensing (LM75), LCD display (ST7735s)

### Key Dependencies
- `esp-hal = { version = "1.0.0-rc.0", features = ["defmt", "esp32c3", "unstable"] }`
- `esp-hal-embassy = { version = "0.9.0", features = ["esp32c3"] }`
- `embassy-executor`, `embassy-time` for async runtime
- `defmt` and `esp-println` for dual logging support

### Important Notes
- This is a **bare metal no_std** application, not ESP-IDF
- Use probe-rs for flashing and debugging (configured via .cargo/config.toml)
- Peripherals are initialized directly in main() and passed to tasks to avoid lifetime issues
- All async tasks use embassy's task executor with `#[embassy_executor::task]`

When working with this codebase, prioritize simplicity and follow the established patterns. Always verify changes with `cargo check` and test runtime behavior with `cargo run --release`.