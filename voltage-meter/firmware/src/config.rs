use embassy_time::Duration;
use esp_hal::{
    dma::DmaChannel0,
    gpio::GpioPin,
    peripherals::{Peripherals, ADC1, I2C0, SPI2, SYSTIMER, TIMG0},
    spi,
    time::Rate,
};

pub struct BoardConfig {
    pub embassy: EmbassyConfig,
    pub watchdog: WatchdogConfig,
    pub adc: AdcConfig,
    pub i2c: I2cConfig,
    pub spi: SpiConfig,
    pub display: DisplayConfig,
}

impl BoardConfig {
    pub fn new(p: Peripherals) -> Self {
        Self {
            embassy: EmbassyConfig { timer: p.SYSTIMER },
            watchdog: WatchdogConfig { timer: p.TIMG0 },
            adc: AdcConfig {
                pin: p.GPIO0,
                device: p.ADC1,
            },
            i2c: I2cConfig {
                sda_pin: p.GPIO8,
                scl_pin: p.GPIO9,
                device: p.I2C0,
            },
            spi: SpiConfig {
                dma_channel: p.DMA_CH0,
                device: p.SPI2,
                mode: spi::Mode::_0,
                mosi_pin: p.GPIO6,
                sclk_pin: p.GPIO4,
            },
            display: DisplayConfig {
                rst_pin: p.GPIO10,
                cs_pin: p.GPIO7,
                dc_pin: p.GPIO5,
            },
        }
    }
}

pub struct EmbassyConfig {
    pub timer: SYSTIMER,
}

pub struct WatchdogConfig {
    pub timer: TIMG0,
}

pub type AdcPin = GpioPin<0>;
pub type AdcDevice = ADC1;

pub struct AdcConfig {
    pub pin: AdcPin,
    pub device: AdcDevice,
}

impl AdcConfig {
    const RESISTOR_A: f32 = 12_000.0;
    const RESISTOR_B: f32 = 1_000.0;
    pub const READING_PERIOD: Duration = Duration::from_millis(1000);

    pub fn divider_ratio() -> f32 {
        (Self::RESISTOR_A + Self::RESISTOR_B) / Self::RESISTOR_B
    }
}

pub type I2cSdaPin = GpioPin<8>;
pub type I2cSclPin = GpioPin<9>;

pub struct I2cConfig {
    pub sda_pin: I2cSdaPin,
    pub scl_pin: I2cSclPin,
    pub device: I2C0,
}

impl I2cConfig {
    pub const BUS_SPEED: Rate = Rate::from_khz(400);
}

pub type SpiMosiPin = GpioPin<6>;
pub type SpiSclkPin = GpioPin<4>;
pub type SpiDmaChannel = DmaChannel0;

pub struct SpiConfig {
    pub dma_channel: SpiDmaChannel,
    pub device: SPI2,
    pub mode: spi::Mode,
    pub mosi_pin: SpiMosiPin,
    pub sclk_pin: SpiSclkPin,
}

impl SpiConfig {
    pub const BUS_SPEED: Rate = Rate::from_mhz(40);
    pub const DMA_BUFFER_SIZE: usize = 32_000;
}

pub type DisplayRstPin = GpioPin<10>;
pub type DisplayCsPin = GpioPin<7>;
pub type DisplayDcPin = GpioPin<5>;

pub struct DisplayConfig {
    pub rst_pin: DisplayRstPin,
    pub cs_pin: DisplayCsPin,
    pub dc_pin: DisplayDcPin,
}

pub struct Lm75Config {}

impl Lm75Config {
    pub const I2C_ADDRESS: u8 = 0x48;
    pub const READING_PERIOD: Duration = Duration::from_millis(1000);
}
