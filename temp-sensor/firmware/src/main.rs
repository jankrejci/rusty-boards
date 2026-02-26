//! DS18B20 temperature sensor firmware for ESP32-C3.
//!
//! Discovers sensors on a OneWire bus, reads temperatures periodically, and
//! exports Prometheus metrics over USB-serial. A cooperative watchdog resets
//! the MCU if the sensor task stalls.

#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use defmt::{error, info, warn};
use embassy_executor::Spawner;
use embassy_time::{Ticker, Timer};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::peripherals::TIMG0;
use esp_hal::timer::timg::{MwdtStage, TimerGroup, Wdt};

use defmt_rtt as _;
use esp_backtrace as _;

use config::{Ds18b20Config, WatchdogConfig};
use metrics::{
    metrics_exporter_task, MetricsExporter, MetricsPublisher, SensorTemperature, METRICS_CHANNEL,
};
use onewire::OneWireBus;

/// Cooperative heartbeat flag between the sensor reading task and the watchdog
/// task. The sensor task sets this after each successful reading cycle, and the
/// watchdog task clears it on each check. If the flag is still false when the
/// watchdog checks, the sensor task has stalled and the hardware WDT will reset.
static SENSOR_HEARTBEAT: AtomicBool = AtomicBool::new(false);

mod config;
mod ds18b20;
mod metrics;
mod onewire;

esp_bootloader_esp_idf::esp_app_desc!();
defmt::timestamp!("{=u64:us}", embassy_time::Instant::now().as_micros());

/// Halt the CPU. If the hardware watchdog is armed, it will trigger a reset.
fn halt() -> ! {
    error!("halting");
    #[allow(clippy::empty_loop)]
    loop {}
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    // Spin idle instead of WFI to keep USB-JTAG alive for probe-rs RTT.
    extern "C" fn idle() -> ! {
        #[allow(clippy::empty_loop)]
        loop {}
    }
    esp_rtos::start_with_idle_hook(timg0.timer0, sw_int.software_interrupt0, idle);

    info!("temp-sensor firmware started");

    // Spawn metrics exporter task.
    let subscriber = match METRICS_CHANNEL.subscriber() {
        Ok(s) => s,
        Err(e) => {
            error!("failed to create metrics subscriber: {}", e);
            halt();
        }
    };
    let metrics_exporter = MetricsExporter::new(subscriber);
    if let Err(e) = spawner.spawn(metrics_exporter_task(metrics_exporter)) {
        error!("failed to spawn metrics exporter task: {}", e);
        halt();
    }

    // Initialize OneWire bus on GPIO0.
    let mut bus = match OneWireBus::new(peripherals.RMT, peripherals.GPIO0) {
        Ok(bus) => bus,
        Err(e) => {
            error!("failed to initialize OneWire bus: {}", e);
            halt();
        }
    };

    // Warm-up reset to prime the RMT hardware. The first RMT transaction
    // after channel initialization produces garbage data, so discard it.
    let _ = bus.reset().await;

    // Discover DS18B20 sensors.
    let mut sensors = [None; Ds18b20Config::MAX_SENSORS];
    let count = match ds18b20::discover(&mut bus, &mut sensors).await {
        Ok(count) => count,
        Err(e) => {
            error!("sensor discovery failed: {}", e);
            halt();
        }
    };
    info!("found {} DS18B20 sensors", count);
    // discover() fills slots 0..count contiguously, so take(count) covers all
    // found sensors. The flatten() skips None entries defensively.
    for sensor in sensors.iter().take(count).flatten() {
        info!("  sensor {:X}", sensor.rom_code.serial_number());
    }

    let publisher = match METRICS_CHANNEL.publisher() {
        Ok(p) => p,
        Err(e) => {
            error!("failed to create metrics publisher: {}", e);
            halt();
        }
    };

    // Configure hardware watchdog. Feed once before spawning tasks to
    // cover the startup window while the sensor task initializes.
    let mut wdt = timg0.wdt;
    wdt.set_timeout(MwdtStage::Stage0, WatchdogConfig::TIMEOUT);
    wdt.enable();
    wdt.feed();

    // Spawn sensor reading task.
    if let Err(e) = spawner.spawn(sensor_reading_task(bus, sensors, publisher)) {
        error!("failed to spawn sensor reading task: {}", e);
        halt();
    }

    // Spawn watchdog task.
    if let Err(e) = spawner.spawn(watchdog_task(wdt)) {
        error!("failed to spawn watchdog task: {}", e);
        halt();
    }
}

#[embassy_executor::task]
async fn sensor_reading_task(
    mut bus: OneWireBus<'static>,
    sensors: [Option<ds18b20::Ds18b20>; Ds18b20Config::MAX_SENSORS],
    publisher: MetricsPublisher,
) {
    let mut ticker = Ticker::every(Ds18b20Config::READING_PERIOD);
    loop {
        if let Err(e) = ds18b20::trigger_conversion_all(&mut bus).await {
            warn!("conversion trigger error: {}", e);
            ticker.next().await;
            continue;
        }

        // Wait for 12-bit conversion to complete.
        Timer::after(Ds18b20Config::CONVERSION_WAIT).await;

        let mut any_success = false;
        for slot in &sensors {
            let Some(sensor) = slot.as_ref() else {
                continue;
            };
            match ds18b20::read_temperature(&mut bus, sensor).await {
                Ok(millideg) => {
                    any_success = true;
                    publisher
                        .publish(SensorTemperature::build(
                            sensor.rom_code.serial_number(),
                            millideg,
                        ))
                        .await;
                }
                Err(e) => {
                    warn!(
                        "sensor {:X} read error: {}",
                        sensor.rom_code.serial_number(),
                        e,
                    );
                }
            }
        }

        // Only signal health when at least one sensor read succeeded.
        // If the bus is degraded (reset works but all reads fail), the
        // watchdog will starve and the hardware WDT will reset.
        if any_success {
            SENSOR_HEARTBEAT.store(true, Ordering::Relaxed);
        }
        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn watchdog_task(mut wdt: Wdt<TIMG0<'static>>) {
    loop {
        Timer::after(WatchdogConfig::CHECK_INTERVAL).await;

        if !SENSOR_HEARTBEAT.load(Ordering::Relaxed) {
            error!("sensor heartbeat missed, WDT will reset");
            continue;
        }

        SENSOR_HEARTBEAT.store(false, Ordering::Relaxed);
        wdt.feed();
    }
}
