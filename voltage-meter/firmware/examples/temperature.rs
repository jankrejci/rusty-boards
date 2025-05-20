#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use core::cell::RefCell;
use critical_section::Mutex;
use defmt::{debug, error, info};
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::{
    i2c::master::{Config, I2c},
    time::{self, Rate},
    timer::timg::TimerGroup,
    Async,
};
use static_cell::StaticCell;

use esp_backtrace as _;
use esp_println as _;

use voltage_meter::{
    lm75::{Lm75Reader, LM75_I2C_ADDRESS},
    I2cDevice,
};

static I2C_BUS: StaticCell<Mutex<RefCell<I2c<'static, Async>>>> = StaticCell::new();

defmt::timestamp!(
    "{=u64:us}",
    time::Instant::now().duration_since_epoch().as_micros()
);

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    info!("Embassy runtime initialized");

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
    spawner
        .spawn(simple_reader_task(lm75_reader))
        .expect("BUG: Failed to spawn LM75 reader task");
}

// This simple reader is simillar to the one from lm75 module
// unless this reader does not need the metrics channel
#[embassy_executor::task]
pub async fn simple_reader_task(mut reader: Lm75Reader<I2cDevice>) {
    const DEFAULT_READING_PERIOD: Duration = Duration::from_millis(1000);

    if let Err(e) = reader.init().await {
        error!("Failed to initialize LM75 sensor, {}", e);
        return;
    };

    info!("Lm75Reader task started");

    let mut ticker = Ticker::every(DEFAULT_READING_PERIOD);
    loop {
        ticker.next().await;
        debug!("Measuring the temperature");

        let temperature = match reader.read_temperature().await {
            Ok(temperature) => temperature,
            Err(e) => {
                error!("LM75, {}", e);
                continue;
            }
        };
        info!("LM75 temperature {}", temperature)
    }
}
