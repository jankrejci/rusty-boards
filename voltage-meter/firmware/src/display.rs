use defmt::error;
use defmt::{info, trace};
use embassy_executor::Spawner;
use embassy_time::Delay;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::text::Alignment;
use embedded_graphics::{mono_font::MonoTextStyle, pixelcolor::Rgb565, prelude::*, text::Text};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    gpio::{Level, Output, OutputConfig, OutputPin},
    peripheral::Peripheral,
    spi::master::SpiDmaBus,
    Async,
};
use mipidsi::{
    interface::SpiInterface,
    models::ST7735s,
    options::{ColorOrder, Orientation, Rotation},
    Builder,
};
use profont::{PROFONT_18_POINT, PROFONT_24_POINT};

use crate::metrics::{Metrics, MetricsSubscriber};

type DisplaySpiDevice = ExclusiveDevice<SpiDmaBus<'static, Async>, Output<'static>, Delay>;

pub struct Display<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    hw: mipidsi::Display<SpiInterface<'static, SPI, Output<'static>>, ST7735s, Output<'static>>,
}

impl<SPI> Display<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    const HEIGHT: u16 = 80;
    const WIDTH: u16 = 160;
    const X_OFFSET: u16 = 28;
    const Y_OFFSET: u16 = 0;
    const VOLTAGE_LINE_Y: i32 = 30;
    const TEMPERATURE_LINE_Y: i32 = 70;
    const VALUE_OFFSET_X: i32 = 45;
    const BACKGROUND_COLOR: Rgb565 = Rgb565::BLACK;
    const VOLTAGE_NOTE_STYLE: MonoTextStyle<'_, Rgb565> = MonoTextStyleBuilder::new()
        .font(&PROFONT_18_POINT)
        .text_color(Rgb565::WHITE)
        .background_color(Self::BACKGROUND_COLOR)
        .build();
    const VOLTAGE_VALUE_STYLE: MonoTextStyle<'_, Rgb565> = MonoTextStyleBuilder::new()
        .font(&PROFONT_24_POINT)
        .text_color(Rgb565::WHITE)
        .background_color(Self::BACKGROUND_COLOR)
        .build();
    const TEMPERATURE_NOTE_STYLE: MonoTextStyle<'_, Rgb565> = MonoTextStyleBuilder::new()
        .font(&PROFONT_18_POINT)
        .text_color(Rgb565::CSS_GRAY)
        .background_color(Self::BACKGROUND_COLOR)
        .build();
    const TEMPERATURE_VALUE_STYLE: MonoTextStyle<'_, Rgb565> = MonoTextStyleBuilder::new()
        .font(&PROFONT_24_POINT)
        .text_color(Rgb565::CSS_GRAY)
        .background_color(Self::BACKGROUND_COLOR)
        .build();

    pub fn new(
        spi_device: SPI,
        rst_pin: impl Peripheral<P = impl OutputPin> + 'static,
        dc_pin: impl Peripheral<P = impl OutputPin> + 'static,
        buffer: &'static mut [u8],
    ) -> Self {
        let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
        let rst = Output::new(rst_pin, Level::Low, OutputConfig::default());

        let di = SpiInterface::new(spi_device, dc, buffer);

        let hw = Builder::new(ST7735s, di)
            .display_size(Self::HEIGHT, Self::WIDTH)
            .color_order(ColorOrder::Bgr)
            .display_offset(Self::X_OFFSET, Self::Y_OFFSET)
            .orientation(
                Orientation::new()
                    .rotate(Rotation::Deg270)
                    .flip_vertical()
                    .flip_horizontal(),
            )
            .reset_pin(rst)
            .init(&mut Delay)
            .expect("BUG: Failed to build display device");

        Self { hw }
    }

    pub fn clear(&mut self) -> Result<(), SPI::Error> {
        const SMALL_FONT_OFFSET: i32 = 2;
        self.hw
            .clear(Self::BACKGROUND_COLOR)
            .expect("BUG: Failed to clear display");

        Text::with_alignment(
            "V:         V",
            Point::new(
                (Self::WIDTH / 2) as i32,
                Self::VOLTAGE_LINE_Y - SMALL_FONT_OFFSET,
            ),
            Self::VOLTAGE_NOTE_STYLE,
            Alignment::Center,
        )
        .draw(&mut self.hw)
        .expect("BUG: Failed to draw the voltage");

        Text::with_alignment(
            "T:        °C",
            Point::new(
                (Self::WIDTH / 2) as i32,
                Self::TEMPERATURE_LINE_Y - SMALL_FONT_OFFSET,
            ),
            Self::TEMPERATURE_NOTE_STYLE,
            Alignment::Center,
        )
        .draw(&mut self.hw)
        .expect("BUG: Failed to draw the temperature");

        Ok(())
    }

    pub fn write_voltage(&mut self, voltage: f32) -> Result<(), SPI::Error> {
        let mut buf = [0u8; 20];

        let value = format_no_std::show(&mut buf, format_args!("{:5.2}", voltage))
            .expect("BUG: Failed to serialize temperature string");

        Text::new(
            value,
            Point::new(Self::VALUE_OFFSET_X, Self::VOLTAGE_LINE_Y),
            Self::VOLTAGE_VALUE_STYLE,
        )
        .draw(&mut self.hw)
        .expect("BUG: Failed to draw the voltage");

        Ok(())
    }

    pub fn write_temperature(&mut self, temperature: f32) -> Result<(), SPI::Error> {
        let mut buf = [0u8; 20];

        let value = format_no_std::show(&mut buf, format_args!("{:5.2}", temperature))
            .expect("BUG: Failed to serialize temperature string");

        Text::new(
            value,
            Point::new(Self::VALUE_OFFSET_X, Self::TEMPERATURE_LINE_Y),
            Self::TEMPERATURE_VALUE_STYLE,
        )
        .draw(&mut self.hw)
        .expect("BUG: Failed to draw the temperature");
        Ok(())
    }
}

pub type ConcreteDisplay = Display<DisplaySpiDevice>;

pub fn create_display_device(
    spi_bus: SpiDmaBus<'static, Async>,
    cs_pin: impl Peripheral<P = impl OutputPin> + 'static,
) -> DisplaySpiDevice {
    let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
    ExclusiveDevice::new(spi_bus, cs, Delay).expect("BUG: Failed to create exclusive device")
}

#[embassy_executor::task]
pub async fn display_updater_task(
    mut display: ConcreteDisplay,
    mut metrics_subscriber: MetricsSubscriber,
) {
    display.clear().expect("BUG: Failed to clear the display");

    info!("Display updater task started");

    loop {
        trace!("Updating the display");

        match metrics_subscriber.next_message_pure().await {
            Metrics::VoltageFeedback(metrics) => {
                if let Err(_) = display.write_voltage(metrics.voltage) {
                    error!("Failed to write voltage");
                }
            }
            Metrics::AmbientTemperature(metrics) => {
                if let Err(_) = display.write_temperature(metrics.temperature) {
                    error!("Failed to write temperature");
                }
            }
        }
    }
}

pub fn setup_display(
    spi_bus: SpiDmaBus<'static, Async>,
    cs_pin: impl Peripheral<P = impl OutputPin> + 'static,
    rst_pin: impl Peripheral<P = impl OutputPin> + 'static,
    dc_pin: impl Peripheral<P = impl OutputPin> + 'static,
    buffer: &'static mut [u8],
    spawner: Spawner,
    metrics_subscriber: MetricsSubscriber,
) {
    let spi_device = create_display_device(spi_bus, cs_pin);
    let display = Display::new(spi_device, rst_pin, dc_pin, buffer);
    spawner
        .spawn(display_updater_task(display, metrics_subscriber))
        .expect("BUG: Failed to spawn the display updater task");
}
