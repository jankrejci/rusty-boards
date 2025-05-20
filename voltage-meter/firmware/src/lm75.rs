use defmt::{self, debug, error, info, trace};
use embassy_time::{Duration, Ticker};
use embedded_hal::i2c;

use crate::I2cDevice;

pub const LM75_I2C_ADDRESS: u8 = 0x48;
pub const BIT_MASK_RESOLUTION_11BIT: u16 = 0b1111_1111_1110_0000;

#[derive(defmt::Format)]
pub enum Error {
    TemperatureReading,
}

pub struct Lm75Reader<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> Lm75Reader<I2C>
where
    I2C: i2c::I2c,
{
    pub fn new(i2c: I2C, address: u8) -> Self {
        debug!("Lm75Reader initialized succesfully");

        Self { i2c, address }
    }

    pub async fn read_temperature(&mut self) -> Result<f32, I2C::Error> {
        let mut data = [0; 2];
        // TODO use I2C async from embedded-hal-async
        self.i2c.write_read(self.address, &[0x00], &mut data)?;
        // .map_err(|e| Error::TemperatureReading)?;

        let temperature = Self::convert_temp_from_register(&data);
        trace!(
            "LM75 address: {}, data: {=[u8;2]}, temperature: {} C",
            self.address,
            data,
            temperature
        );
        Ok(temperature)
    }

    pub fn convert_temp_from_register(data: &[u8; 2]) -> f32 {
        // msb is stored as two's complement
        let msb = f32::from(data[0] as i8);
        let decimal = f32::from((data[1] & BIT_MASK_RESOLUTION_11BIT as u8) >> 5) * 0.125;
        msb + decimal
    }

    pub async fn init(&mut self) -> Result<(), I2C::Error> {
        let temperature = self.read_temperature().await?;
        debug!("Lm75 initialized succesfully, temperature {}", temperature);
        Ok(())
    }
}

#[embassy_executor::task]
pub async fn reader_task(mut reader: Lm75Reader<I2cDevice>, reading_period: Duration) {
    if let Err(e) = reader.init().await {
        error!("Failed to initialize LM75 sensor, {}", e);
        return;
    };

    info!("Lm75Reader task started");

    let mut ticker = Ticker::every(reading_period);
    loop {
        ticker.next().await;
        trace!("Measuring the temperature");

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
