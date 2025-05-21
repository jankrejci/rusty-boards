#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_hal::{time, timer::timg::TimerGroup};

use esp_backtrace as _;
use esp_println as _;

use voltage_meter::metrics::{
    self, AmbientTemperature, MetricsExporter, MetricsPublisher, METRICS_CHANNEL,
};

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

    spawner
        .spawn(simple_publisher_task(
            METRICS_CHANNEL
                .publisher()
                .expect("BUG: Not enough publishers left"),
        ))
        .expect("BUG: Failed to spawn ADC reader task");

    let metrics_handler = MetricsExporter::new(
        METRICS_CHANNEL
            .subscriber()
            .expect("BUG: Not enough subscribers left"),
    );
    spawner
        .spawn(metrics::metrics_exporter_task(metrics_handler))
        .expect("BUG: Failed to spawn metrics task");
}

#[embassy_executor::task]
pub async fn simple_publisher_task(metrics_publisher: MetricsPublisher) {
    const DEFAULT_PUBLISHING_PERIOD: Duration = Duration::from_millis(1000);

    info!("Simple publisher task started");

    let mut ticker = Ticker::every(DEFAULT_PUBLISHING_PERIOD);
    loop {
        ticker.next().await;

        let temperature = 99.9;
        let metrics = AmbientTemperature::build(temperature);
        metrics_publisher.publish(metrics).await;
    }
}
