#![no_std]
#![no_main]

use defmt::{debug, info};
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_hal::{time, timer::timg::TimerGroup};

use esp_backtrace as _;
use esp_println as _;

use voltage_meter::adc::{AdcReader, VoltageReader};

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

    const ADC_DIVIDER_RATIO: f32 = 13.0;
    let adc_reader = AdcReader::new(peripherals.ADC1, peripherals.GPIO0, ADC_DIVIDER_RATIO);
    spawner
        .spawn(simple_reader_task(adc_reader))
        .expect("BUG: Failed to spawn LM75 reader task");
}

// This simple reader is simillar to the one from adc module
// unless this reader does not need the metrics channel
#[embassy_executor::task]
pub async fn simple_reader_task(mut reader: VoltageReader) {
    const DEFAULT_READING_PERIOD: Duration = Duration::from_millis(1000);
    info!("AdcReader task started");

    let mut ticker = Ticker::every(DEFAULT_READING_PERIOD);
    loop {
        ticker.next().await;
        let voltage = reader.read_voltage().await;

        debug!("AdcReader, voltage: {} V", voltage,);
    }
}
