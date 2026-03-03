use defmt::warn;

use crate::onewire::{Error, OneWireBus, RomCode, SearchResult};

const DS18B20_FAMILY_CODE: u8 = 0x28;

// DS18B20 function commands.
const CMD_CONVERT_T: u8 = 0x44;
const CMD_READ_SCRATCHPAD: u8 = 0xBE;

/// A discovered DS18B20 sensor identified by its ROM code.
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct Ds18b20 {
    pub rom_code: RomCode,
}

/// Discover DS18B20 sensors on the bus. Returns the number found.
///
/// Fills the provided array with discovered sensors, up to its length. Only
/// devices with the DS18B20 family code (0x28) are included.
pub async fn discover<const N: usize>(
    bus: &mut OneWireBus<'_>,
    sensors: &mut [Option<Ds18b20>; N],
) -> Result<usize, Error> {
    let mut search = crate::onewire::SearchState::new();
    let mut count = 0;

    loop {
        match bus.search(&mut search).await {
            Ok(SearchResult::Found(rom)) => {
                if rom.family_code() != DS18B20_FAMILY_CODE {
                    continue;
                }
                if count < N {
                    sensors[count] = Some(Ds18b20 { rom_code: rom });
                    count += 1;
                } else {
                    warn!(
                        "sensor {:X} dropped, array full ({} max)",
                        rom.serial_number(),
                        N,
                    );
                }
            }
            Ok(SearchResult::Done) => break,
            // No devices responded to reset or search. The bus is empty.
            Err(Error::NoPresence) => break,
            Err(e) => return Err(e),
        }
    }

    Ok(count)
}

/// Trigger temperature conversion on all sensors simultaneously.
///
/// After calling this, wait at least 750 ms for 12-bit resolution before
/// reading results.
pub async fn trigger_conversion_all(bus: &mut OneWireBus<'_>) -> Result<(), Error> {
    bus.skip_rom().await?;
    bus.write_bytes(&[CMD_CONVERT_T]).await
}

/// Read the temperature from a specific sensor.
///
/// Returns temperature in millidegrees Celsius as an integer. For example,
/// 25125 means 25.125 degrees C.
pub async fn read_temperature(bus: &mut OneWireBus<'_>, sensor: &Ds18b20) -> Result<i32, Error> {
    bus.match_rom(&sensor.rom_code).await?;
    bus.write_bytes(&[CMD_READ_SCRATCHPAD]).await?;

    let mut scratchpad = [0u8; 9];
    bus.read_bytes(&mut scratchpad).await?;

    // Byte 8 is the CRC of bytes 0-7.
    let crc = crate::onewire::crc8(&scratchpad[..8]);
    if crc != scratchpad[8] {
        return Err(Error::Crc);
    }

    // Bytes 0-1 are the raw temperature in 1/16 degree units, little-endian
    // signed. Multiply by 625 and divide by 10 to convert to millidegrees.
    let raw = i16::from_le_bytes([scratchpad[0], scratchpad[1]]);
    let millideg = (raw as i32).saturating_mul(625) / 10;

    Ok(millideg)
}
