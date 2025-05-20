#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::clock::CpuClock;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::{i2c::master::I2c, Async};
use {esp_backtrace as _, esp_println as _};

mod lm75;

pub type I2cDevice = CriticalSectionDevice<'static, I2c<'static, Async>>;

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Embassy initialized!");

    loop {
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
    }
}
