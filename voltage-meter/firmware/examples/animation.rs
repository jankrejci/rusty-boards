#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::Delay;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Rectangle, Triangle},
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers,
    gpio::{Level, Output, OutputConfig},
    spi::{
        master::{Config, Spi},
        Mode,
    },
    time::Rate,
    timer::timg::TimerGroup,
};
use mipidsi::{interface::SpiInterface, models::ST7735s, options::Orientation, Builder};

use esp_backtrace as _;
use esp_println as _;

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    info!("Embassy runtime initialized");

    let sclk = peripherals.GPIO4;
    let mosi = peripherals.GPIO6;
    let cs = peripherals.GPIO7;
    let dc = peripherals.GPIO5;
    let rst = peripherals.GPIO10;
    let dma_channel = peripherals.DMA_CH0;

    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(32000);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    const SPI_BUS_SPEED: Rate = Rate::from_mhz(80);
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(SPI_BUS_SPEED)
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_dma(dma_channel)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let cs = Output::new(cs, Level::High, OutputConfig::default());
    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();

    let mut buffer = [0_u8; 32000];
    let dc = Output::new(dc, Level::Low, OutputConfig::default());
    let di = SpiInterface::new(spi_device, dc, &mut buffer);

    // let mut delay = Delay::new();
    let rst = Output::new(rst, Level::Low, OutputConfig::default());
    // Define the display from the display interface and initialize it

    let mut display = Builder::new(ST7735s, di)
        .display_size(80, 160)
        .color_order(mipidsi::options::ColorOrder::Bgr)
        .display_offset(28, 0)
        .orientation(
            Orientation::new()
                .rotate(mipidsi::options::Rotation::Deg270)
                .flip_vertical()
                .flip_horizontal(),
        )
        .reset_pin(rst)
        .init(&mut Delay)
        .expect("BUG: Failed to build display driver");

    // Make the display all black
    display
        .clear(Rgb565::BLACK)
        .expect("BUG: Failed to clear display");

    // Draw a smiley face with white eyes and a red mouth
    draw_smiley(&mut display).expect("BUG: Failed to draw display");
    info!("Done");

    let mut delay = Delay;
    delay.delay_ms(3000);
    display
        .clear(Rgb565::WHITE)
        .expect("BUG: Failed to clear display");

    loop {
        let mut rectangle = Rectangle::new(Point::new(1, 1), Size::new(20, 20));

        // const
        for _ in 0..59 {
            for _ in 0..139 {
                rectangle
                    .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
                    .draw(&mut display)
                    .expect("BUG: Failed to draw the display");
                delay.delay_ms(10);
                display
                    .fill_solid(&rectangle, Rgb565::BLACK)
                    .expect("BUG: Failed to clear display");
                rectangle = rectangle.translate(Point::new(1, 0));
            }
            rectangle = rectangle.translate(Point::new(-139, 1));
        }
    }
}

fn draw_smiley<T: DrawTarget<Color = Rgb565>>(display: &mut T) -> Result<(), T::Error> {
    // Draw the left eye as a circle located at (50, 100), with a diameter of 40, filled with white
    Circle::new(Point::new(40, 0), 80)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::YELLOW))
        .draw(display)?;

    Rectangle::new(Point::new(0, 30), Size::new(20, 20))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
        .draw(display)?;

    Rectangle::new(Point::new(140, 30), Size::new(20, 20))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::BLUE))
        .draw(display)?;

    // Draw the left eye as a circle located at (50, 100), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 20), 20)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw the right eye as a circle located at (50, 200), with a diameter of 40, filled with white
    Circle::new(Point::new(90, 20), 20)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw an upside down red triangle to represent a smiling mouth
    Triangle::new(Point::new(60, 50), Point::new(100, 50), Point::new(80, 70))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
        .draw(display)?;

    // Cover the top part of the mouth with a black triangle so it looks closed instead of open
    Triangle::new(Point::new(70, 50), Point::new(90, 50), Point::new(80, 60))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
        .draw(display)?;

    Ok(())
}
