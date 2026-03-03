#![no_std]
#![no_main]

use core::cell::RefCell;
use critical_section::Mutex;
use defmt::{info, trace};
use embassy_executor::Spawner;
use embassy_time::Timer;
use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::{
    clock::CpuClock,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers,
    i2c::{master::Config as I2cConfig, master::I2c},
    peripherals::TIMG1,
    spi::{self, master::Spi},
    timer::timg::{MwdtStage, TimerGroup, Wdt},
    Async,
};
use static_cell::StaticCell;

use esp_backtrace as _;
use esp_println as _;

use adc::AdcReader;
use config::{AdcConfig, I2cConfig as I2cConstants, Lm75Config, SpiConfig};
use lm75::Lm75Reader;
use metrics::{MetricsExporter, METRICS_CHANNEL};

mod adc;
mod config;
mod display;
mod kalman;
mod lm75;
mod metrics;

pub type I2cDevice = CriticalSectionDevice<'static, I2c<'static, Async>>;

static I2C_BUS: StaticCell<Mutex<RefCell<I2c<'static, Async>>>> = StaticCell::new();
static DISPLAY_BUFFER: StaticCell<[u8; 32000]> = StaticCell::new();

defmt::timestamp!("{=u64}", embassy_time::Instant::now().as_micros());

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    info!("Embassy initialized!");

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    spawner
        .spawn(watchdog_feed_task(timg1.wdt))
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
            I2cConfig::default().with_frequency(I2cConstants::BUS_SPEED),
        )
        .unwrap()
        .with_sda(peripherals.GPIO8)
        .with_scl(peripherals.GPIO9)
        .into_async(),
    )));

    spawner
        .spawn(lm75::reader_task(
            Lm75Reader::new(CriticalSectionDevice::new(i2c_bus), Lm75Config::I2C_ADDRESS),
            Lm75Config::READING_PERIOD,
            METRICS_CHANNEL
                .publisher()
                .expect("BUG: Not enough publishers left"),
        ))
        .expect("BUG: Failed to spawn LM75 reader task");

    let adc_reader = AdcReader::new(
        peripherals.ADC1,
        peripherals.GPIO0,
        AdcConfig::divider_ratio(),
    );

    spawner
        .spawn(adc::reader_task(
            adc_reader,
            AdcConfig::READING_PERIOD,
            METRICS_CHANNEL
                .publisher()
                .expect("BUG: Not enough publishers left"),
        ))
        .expect("BUG: Failed to spawn ADC reader task");

    // Upstream: esp-hal dma_buffers! macro uses manual div-ceil internally.
    #[allow(clippy::manual_div_ceil)]
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
        dma_buffers!(SpiConfig::DMA_BUFFER_SIZE);
    let dma_rx_buf =
        DmaRxBuf::new(rx_descriptors, rx_buffer).expect("BUG: Failed to create dma rx buffer");
    let dma_tx_buf =
        DmaTxBuf::new(tx_descriptors, tx_buffer).expect("BUG: Failed to create dma tx buffer");

    let spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(SpiConfig::BUS_SPEED)
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO4)
    .with_mosi(peripherals.GPIO6)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let buffer = DISPLAY_BUFFER.init([0_u8; 32000]);
    display::setup_display(
        spi_bus,
        peripherals.GPIO7,
        peripherals.GPIO10,
        peripherals.GPIO5,
        buffer,
        spawner,
        METRICS_CHANNEL
            .subscriber()
            .expect("BUG: Not enough subscribers left"),
    );

    info!("All tasks spawned, entering main loop");

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
pub async fn watchdog_feed_task(mut watchdog: Wdt<TIMG1<'static>>) {
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
