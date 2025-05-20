#![no_std]
#![no_main]

use core::cell::RefCell;
use critical_section::Mutex;
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::Duration;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config, I2c},
    time::{self, Rate},
    timer::systimer::SystemTimer,
    Async,
};
use static_cell::StaticCell;

use esp_backtrace as _;
use esp_println as _;

use lm75::{Lm75Reader, LM75_I2C_ADDRESS};

mod lm75;

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

    let lm75_reader = Lm75Reader::new(CriticalSectionDevice::new(i2c_bus), LM75_I2C_ADDRESS);
    let lm75_reading_period = Duration::from_millis(1000);
    spawner
        .spawn(lm75::reader_task(lm75_reader, lm75_reading_period))
        .expect("BUG: Failed to spawn LM75 reader task");
}
