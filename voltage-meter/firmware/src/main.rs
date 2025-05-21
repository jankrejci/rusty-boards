#![no_std]
#![no_main]

use core::cell::RefCell;
use critical_section::Mutex;
use defmt::{info, trace};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config, I2c},
    peripherals::TIMG0,
    time::{self, Rate},
    timer::{
        systimer::SystemTimer,
        timg::{MwdtStage, TimerGroup, Wdt},
    },
    Async,
};
use static_cell::StaticCell;

use esp_backtrace as _;
use esp_println as _;

use adc::AdcReader;
use lm75::{Lm75Reader, LM75_I2C_ADDRESS};
use metrics::{MetricsExporter, METRICS_CHANNEL};

mod adc;
mod lm75;
mod metrics;

pub type I2cDevice = CriticalSectionDevice<'static, I2c<'static, Async>>;

static I2C_BUS: StaticCell<Mutex<RefCell<I2c<'static, Async>>>> = StaticCell::new();

defmt::timestamp!(
    "{=u64:us}",
    time::Instant::now().duration_since_epoch().as_micros()
);

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Embassy initialized!");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    spawner
        .spawn(watchdog_feed_task(timg0.wdt))
        .expect("BUG: Failed to spawn watchdog task");

    let metrics_exporter = MetricsExporter::new(
        METRICS_CHANNEL
            .subscriber()
            .expect("BUG: Not enough subscribers left"),
    );
    spawner
        .spawn(metrics::metrics_exporter_task(metrics_exporter))
        .expect("BUG: Failed to spawn metrics task");

    let i2c_bus = I2C_BUS.init(Mutex::new(RefCell::new(
        I2c::new(
            peripherals.I2C0,
            Config::default().with_frequency(Rate::from_khz(400)),
        )
        .expect("Failed to build I2C bus")
        .with_sda(peripherals.GPIO8)
        .with_scl(peripherals.GPIO9)
        .into_async(),
    )));

    const LM75_READING_PERIOD: Duration = Duration::from_millis(1000);
    let lm75_reader = Lm75Reader::new(CriticalSectionDevice::new(i2c_bus), LM75_I2C_ADDRESS);
    spawner
        .spawn(lm75::reader_task(
            lm75_reader,
            LM75_READING_PERIOD,
            METRICS_CHANNEL
                .publisher()
                .expect("BUG: Not enough publishers left"),
        ))
        .expect("BUG: Failed to spawn LM75 reader task");

    const RESISTOR_A: f32 = 12_000.0;
    const RESISTOR_B: f32 = 1_000.0;
    const ADC_READING_PERIOD: Duration = Duration::from_millis(1000);
    let divider_ratio = (RESISTOR_A + RESISTOR_B) / RESISTOR_B;
    let adc_reader = AdcReader::new(peripherals.ADC1, peripherals.GPIO0, divider_ratio);
    spawner
        .spawn(adc::reader_task(
            adc_reader,
            ADC_READING_PERIOD,
            METRICS_CHANNEL
                .publisher()
                .expect("BUG: Not enough publishers left"),
        ))
        .expect("BUG: Failed to spawn LM75 reader task");
}

#[embassy_executor::task]
pub async fn watchdog_feed_task(mut watchdog: Wdt<TIMG0>) {
    const WATCHDOG_TIMEOUT: esp_hal::time::Duration = esp_hal::time::Duration::from_millis(2000);
    const WATCHDOG_FEED_PERIOD: embassy_time::Duration = embassy_time::Duration::from_millis(500);

    watchdog.enable();
    watchdog.set_timeout(MwdtStage::Stage0, WATCHDOG_TIMEOUT);

    info!("Watchdog feeding task started");
    loop {
        watchdog.feed();
        trace!("Feeding the watchdog");
        Timer::after(WATCHDOG_FEED_PERIOD).await;
    }
}
