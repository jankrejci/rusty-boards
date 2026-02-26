//! Hardware and runtime constants for the temp-sensor firmware.
//!
//! Each struct groups related constants as associated items. All values are
//! compile-time constants with no runtime cost.

use embassy_time::Duration;

pub struct OneWireConfig;

impl OneWireConfig {
    /// RMT peripheral source clock frequency.
    pub const RMT_FREQ: esp_hal::time::Rate = esp_hal::time::Rate::from_mhz(80);

    /// 80 MHz APB clock / 80 = 1 microsecond per RMT tick.
    pub const CLK_DIVIDER: u8 = 80;

    /// Settle delay before each bus reset to let RMT hardware stabilize after prior operations.
    pub const SETTLE_DELAY: Duration = Duration::from_millis(1);
}

pub struct Ds18b20Config;

impl Ds18b20Config {
    pub const MAX_SENSORS: usize = 8;
    pub const READING_PERIOD: Duration = Duration::from_secs(2);

    /// Wait time for 12-bit temperature conversion to complete.
    pub const CONVERSION_WAIT: Duration = Duration::from_millis(750);
}

pub struct MetricsConfig;

impl MetricsConfig {
    pub const CHANNEL_SIZE: usize = 10;
    pub const NUM_SUBSCRIBERS: usize = 1;
    pub const NUM_PUBLISHERS: usize = 1;
}

pub struct WatchdogConfig;

impl WatchdogConfig {
    /// Hardware WDT timeout. Must be greater than CHECK_INTERVAL to allow
    /// at least one check before the hardware resets.
    ///
    /// Uses esp_hal::time::Duration because Wdt::set_timeout requires the
    /// HAL duration type, not embassy_time::Duration. These are distinct
    /// types with different internal representations.
    pub const TIMEOUT: esp_hal::time::Duration = esp_hal::time::Duration::from_secs(10);

    /// How often the watchdog task checks the sensor heartbeat.
    pub const CHECK_INTERVAL: Duration = Duration::from_secs(5);
}
