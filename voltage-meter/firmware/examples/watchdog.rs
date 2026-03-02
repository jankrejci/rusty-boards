#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use defmt::{debug, info};
use embassy_executor::Spawner;
use embassy_time::{self, Timer};
use esp_hal::{
    self,
    peripherals::TIMG0,
    timer::timg::{MwdtStage, TimerGroup, Wdt},
};

use esp_backtrace as _;
use esp_println as _;

defmt::timestamp!(
    "{=u64:us}",
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_micros()
);

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    info!("Embassy runtime initialized");

    spawner
        .spawn(watchdog_feed_task(timg0.wdt))
        .expect("BUG: Failed to spawn watchdog task");
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
        debug!("Feediing the watchdog");
        Timer::after(WATCHDOG_FEED_PERIOD).await;
    }
}
