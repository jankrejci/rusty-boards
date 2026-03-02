use embassy_time::Duration;
use esp_hal::time::Rate;

pub struct AdcConfig {}

impl AdcConfig {
    const RESISTOR_A: f32 = 12_000.0;
    const RESISTOR_B: f32 = 1_000.0;
    const CALIBRATION_OFFSET: f32 = 0.996;
    pub const READING_PERIOD: Duration = Duration::from_millis(1000);

    pub fn divider_ratio() -> f32 {
        Self::CALIBRATION_OFFSET * (Self::RESISTOR_A + Self::RESISTOR_B) / Self::RESISTOR_B
    }
}

pub struct I2cConfig {}

impl I2cConfig {
    pub const BUS_SPEED: Rate = Rate::from_khz(400);
}

pub struct SpiConfig {}

impl SpiConfig {
    pub const BUS_SPEED: Rate = Rate::from_mhz(40);
    pub const DMA_BUFFER_SIZE: usize = 32_000;
}

pub struct Lm75Config {}

impl Lm75Config {
    pub const I2C_ADDRESS: u8 = 0x48;
    pub const READING_PERIOD: Duration = Duration::from_millis(1000);
}
