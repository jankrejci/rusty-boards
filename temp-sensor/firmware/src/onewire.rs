//! OneWire bus driver using ESP32-C3 RMT peripheral for bit-banged timing.
//!
//! The RMT (Remote Control Transceiver) generates and captures precise pulse
//! waveforms without CPU intervention. TX drives the bus with timed low/high
//! pulses, RX captures device responses. Both channels share one GPIO pin via
//! open-drain output with internal pull-up.
//!
//! ESP32-C3 RMT has no hardware RX stop. esp-hal works around this by
//! manipulating idle threshold and filter registers, then spin-waiting for an
//! end event. After many rapid RMT operations this stop hack may leave
//! residual state, causing subsequent transactions to see zero-length pulses.
//! A settle delay before each reset mitigates this.

use defmt::trace;
use embassy_futures::join::join;
use embassy_time::Timer;
use esp_hal::{
    gpio::{Flex, Level, Pin},
    rmt::{
        self, Channel, PulseCode, Rx, RxChannelConfig, RxChannelCreator, Tx, TxChannelConfig,
        TxChannelCreator,
    },
    Async,
};

use crate::config::OneWireConfig;

// OneWire timing constants in RMT ticks. With CLK_DIVIDER=80 on a 80 MHz
// APB clock, each tick is exactly 1 microsecond.
const RESET_LOW: u16 = 500;
// Master release window must cover the full presence response: devices wait
// 15-60 us after release, then pull low for 60-240 us. Total up to 300 us.
const RESET_RELEASE: u16 = 480;
const WRITE_0_LOW: u16 = 62;
const WRITE_0_RELEASE: u16 = 5;
const WRITE_1_LOW: u16 = 2;
const WRITE_1_RELEASE: u16 = 65;
const READ_SLOT_LOW: u16 = 2;
const READ_SLOT_RELEASE: u16 = 65;

// Presence pulse detection thresholds in microseconds.
const PRESENCE_MIN_US: u16 = 60;
const PRESENCE_MAX_US: u16 = 240;

// Threshold for distinguishing bit 0 vs bit 1 in read slots.
// If device holds line low longer than this, it is a 0 bit.
const READ_BIT_THRESHOLD: u16 = 10;

// RMT RX idle threshold in ticks. When the line stays idle this long, the
// receiver considers the signal complete. Must exceed the longest expected
// bus idle gap, which is the presence response window (~300 us after reset
// release). 1000 us provides generous margin.
const RX_IDLE_THRESHOLD: u16 = 1000;

// RMT RX filter threshold. Pulses shorter than this many ticks are filtered
// as noise. The OneWire protocol does not use pulses below 1 microsecond.
const RX_FILTER_THRESHOLD: u8 = 2;

// OneWire ROM commands.
const CMD_SEARCH_ROM: u8 = 0xF0;
const CMD_SKIP_ROM: u8 = 0xCC;
const CMD_MATCH_ROM: u8 = 0x55;

// Maximum bytes that can be written in a single RMT transaction. Each byte
// takes 8 PulseCode entries, plus one end marker. With memsize=2, the channel
// has 96 entries total.
const MAX_WRITE_BYTES: usize = 11;
const MAX_WRITE_PULSES: usize = MAX_WRITE_BYTES * 8 + 1;

// Maximum bytes that can be read in a single RMT transaction.
const MAX_READ_BYTES: usize = 11;
const MAX_READ_PULSES: usize = MAX_READ_BYTES * 8 + 1;

/// Dallas CRC8 lookup table.
#[rustfmt::skip]
static CRC8_TABLE: [u8; 256] = [
    0x00, 0x5E, 0xBC, 0xE2, 0x61, 0x3F, 0xDD, 0x83,
    0xC2, 0x9C, 0x7E, 0x20, 0xA3, 0xFD, 0x1F, 0x41,
    0x9D, 0xC3, 0x21, 0x7F, 0xFC, 0xA2, 0x40, 0x1E,
    0x5F, 0x01, 0xE3, 0xBD, 0x3E, 0x60, 0x82, 0xDC,
    0x23, 0x7D, 0x9F, 0xC1, 0x42, 0x1C, 0xFE, 0xA0,
    0xE1, 0xBF, 0x5D, 0x03, 0x80, 0xDE, 0x3C, 0x62,
    0xBE, 0xE0, 0x02, 0x5C, 0xDF, 0x81, 0x63, 0x3D,
    0x7C, 0x22, 0xC0, 0x9E, 0x1D, 0x43, 0xA1, 0xFF,
    0x46, 0x18, 0xFA, 0xA4, 0x27, 0x79, 0x9B, 0xC5,
    0x84, 0xDA, 0x38, 0x66, 0xE5, 0xBB, 0x59, 0x07,
    0xDB, 0x85, 0x67, 0x39, 0xBA, 0xE4, 0x06, 0x58,
    0x19, 0x47, 0xA5, 0xFB, 0x78, 0x26, 0xC4, 0x9A,
    0x65, 0x3B, 0xD9, 0x87, 0x04, 0x5A, 0xB8, 0xE6,
    0xA7, 0xF9, 0x1B, 0x45, 0xC6, 0x98, 0x7A, 0x24,
    0xF8, 0xA6, 0x44, 0x1A, 0x99, 0xC7, 0x25, 0x7B,
    0x3A, 0x64, 0x86, 0xD8, 0x5B, 0x05, 0xE7, 0xB9,
    0x8C, 0xD2, 0x30, 0x6E, 0xED, 0xB3, 0x51, 0x0F,
    0x4E, 0x10, 0xF2, 0xAC, 0x2F, 0x71, 0x93, 0xCD,
    0x11, 0x4F, 0xAD, 0xF3, 0x70, 0x2E, 0xCC, 0x92,
    0xD3, 0x8D, 0x6F, 0x31, 0xB2, 0xEC, 0x0E, 0x50,
    0xAF, 0xF1, 0x13, 0x4D, 0xCE, 0x90, 0x72, 0x2C,
    0x6D, 0x33, 0xD1, 0x8F, 0x0C, 0x52, 0xB0, 0xEE,
    0x32, 0x6C, 0x8E, 0xD0, 0x53, 0x0D, 0xEF, 0xB1,
    0xF0, 0xAE, 0x4C, 0x12, 0x91, 0xCF, 0x2D, 0x73,
    0xCA, 0x94, 0x76, 0x28, 0xAB, 0xF5, 0x17, 0x49,
    0x08, 0x56, 0xB4, 0xEA, 0x69, 0x37, 0xD5, 0x8B,
    0x57, 0x09, 0xEB, 0xB5, 0x36, 0x68, 0x8A, 0xD4,
    0x95, 0xCB, 0x29, 0x77, 0xF4, 0xAA, 0x48, 0x16,
    0xE9, 0xB7, 0x55, 0x0B, 0x88, 0xD6, 0x34, 0x6A,
    0x2B, 0x75, 0x97, 0xC9, 0x4A, 0x14, 0xF6, 0xA8,
    0x74, 0x2A, 0xC8, 0x96, 0x15, 0x4B, 0xA9, 0xF7,
    0xB6, 0xE8, 0x0A, 0x54, 0xD7, 0x89, 0x6B, 0x35,
];

pub fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        crc = CRC8_TABLE[(crc ^ byte) as usize];
    }
    crc
}

/// OneWire 64-bit ROM code parsed into its components.
/// Byte 0: family code, bytes 1-6: 48-bit serial number (little-endian),
/// byte 7: CRC8 of bytes 0-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub struct RomCode {
    family_code: u8,
    serial_number: u64,
    crc: u8,
}

impl RomCode {
    /// Parse a ROM code from the 8 raw bytes read from the bus.
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        let mut serial_bytes = [0u8; 8];
        serial_bytes[..6].copy_from_slice(&bytes[1..7]);
        Self {
            family_code: bytes[0],
            serial_number: u64::from_le_bytes(serial_bytes),
            crc: bytes[7],
        }
    }

    /// Reconstruct the 8 raw bytes for transmission on the bus.
    pub fn to_bytes(self) -> [u8; 8] {
        let serial_bytes = self.serial_number.to_le_bytes();
        let mut bytes = [0u8; 8];
        bytes[0] = self.family_code;
        bytes[1..7].copy_from_slice(&serial_bytes[..6]);
        bytes[7] = self.crc;
        bytes
    }

    pub fn family_code(&self) -> u8 {
        self.family_code
    }

    pub fn serial_number(&self) -> u64 {
        self.serial_number
    }

    fn is_crc_valid(&self) -> bool {
        let bytes = self.to_bytes();
        crc8(&bytes[..7]) == bytes[7]
    }
}

#[derive(Debug, defmt::Format)]
pub enum Error {
    NoPresence,
    Rmt(rmt::Error),
    Crc,
    BufTooLarge,
    /// RMT receive returned fewer pulse entries than expected. This
    /// indicates an RMT hardware issue rather than a bus protocol error.
    RxShortRead,
}

/// Result of a single ROM search iteration.
#[derive(Debug, defmt::Format)]
pub enum SearchResult {
    Found(RomCode),
    Done,
}

impl From<rmt::Error> for Error {
    fn from(e: rmt::Error) -> Self {
        Error::Rmt(e)
    }
}

/// Persistent state for iterative ROM search across multiple calls.
pub struct SearchState {
    rom_code: [u8; 8],
    last_discrepancy: Option<u8>,
    done: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            rom_code: [0; 8],
            last_discrepancy: None,
            done: false,
        }
    }
}

pub struct OneWireBus<'a> {
    tx: Channel<'a, Async, Tx>,
    rx: Channel<'a, Async, Rx>,
    /// GPIO number stored separately because the pin is consumed by Flex::new
    /// and split into RX/TX signals, so we cannot query it later.
    pin_number: u8,
}

impl<'a> OneWireBus<'a> {
    /// Create a new OneWire bus using RMT channels 0 and 2 on the given GPIO.
    ///
    /// The GPIO pin is wrapped in Flex and split into input and output signals
    /// so both TX and RX channels can share it. After channel creation, GPIO
    /// registers are patched to enable open-drain mode and internal pull-up,
    /// which is required for the OneWire protocol.
    pub fn new(rmt: esp_hal::peripherals::RMT<'a>, pin: impl Pin + 'a) -> Result<Self, Error> {
        let rmt = esp_hal::rmt::Rmt::new(rmt, OneWireConfig::RMT_FREQ)?.into_async();

        let pin_number = pin.number();
        let flex = Flex::new(pin);
        let (rx_signal, tx_signal) = flex.split();

        let tx_config = TxChannelConfig::default()
            .with_clk_divider(OneWireConfig::CLK_DIVIDER)
            .with_idle_output_level(Level::High)
            .with_idle_output(true)
            .with_memsize(2);

        let rx_config = RxChannelConfig::default()
            .with_clk_divider(OneWireConfig::CLK_DIVIDER)
            .with_idle_threshold(RX_IDLE_THRESHOLD)
            .with_filter_threshold(RX_FILTER_THRESHOLD)
            .with_memsize(2);

        let tx = rmt.channel0.configure_tx(tx_signal, tx_config)?;
        let rx = rmt.channel2.configure_rx(rx_signal, rx_config)?;

        // configure_tx calls apply_output_config with OutputConfig::default(),
        // which sets the pin to push-pull with no pull resistors. OneWire
        // requires open-drain with a pull-up. Patch the GPIO registers
        // directly via the PAC to restore the correct configuration.
        set_open_drain_with_pullup(pin_number);

        Ok(Self { tx, rx, pin_number })
    }

    /// Send a reset pulse and detect device presence on the bus.
    pub async fn reset(&mut self) -> Result<(), Error> {
        // Re-apply open-drain config in case RMT operations reset the GPIO.
        set_open_drain_with_pullup(self.pin_number);

        // Let the RMT hardware settle after prior operations. The ESP32-C3
        // RX stop hack manipulates idle threshold and filter registers, then
        // spin-waits for completion. After many rapid operations this can leave
        // residual state that causes the next transaction to capture garbage.
        Timer::after(OneWireConfig::SETTLE_DELAY).await;

        let tx_data = [
            PulseCode::new(Level::Low, RESET_LOW, Level::High, RESET_RELEASE),
            PulseCode::end_marker(),
        ];
        let mut rx_data = [PulseCode::default(); 4];

        // join() polls futures in argument order. TX is first so the 500 us
        // reset pulse is already on the bus before RX starts listening. This
        // is safe because the reset pulse is long enough for RX to catch the
        // subsequent presence response even with a slight start delay.
        let (tx_result, rx_result) =
            join(self.tx.transmit(&tx_data), self.rx.receive(&mut rx_data)).await;
        tx_result?;
        let rx_count = rx_result?;

        trace!(
            "reset: rx_count={}, data=[{:08x}, {:08x}, {:08x}, {:08x}]",
            rx_count,
            rx_data[0].0,
            rx_data[1].0,
            rx_data[2].0,
            rx_data[3].0,
        );

        // The RX captures the entire waveform including our own reset pulse.
        // After the master releases, devices respond with a presence pulse
        // (low for 60-240 microseconds). We need to find this pulse in the
        // captured data, skipping our own initial low pulse.
        for &pulse in rx_data.iter().take(rx_count) {
            // Look for a low phase matching presence pulse timing, but skip
            // the initial reset pulse itself which is much longer.
            if pulse.level1() == Level::Low
                && pulse.length1() >= PRESENCE_MIN_US
                && pulse.length1() <= PRESENCE_MAX_US
            {
                return Ok(());
            }
            if pulse.level2() == Level::Low
                && pulse.length2() >= PRESENCE_MIN_US
                && pulse.length2() <= PRESENCE_MAX_US
            {
                return Ok(());
            }
        }

        Err(Error::NoPresence)
    }

    /// Write bytes to the bus, LSB first.
    pub async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_WRITE_BYTES {
            return Err(Error::BufTooLarge);
        }

        let bit_count = bytes.len() * 8;
        let mut tx_data = [PulseCode::end_marker(); MAX_WRITE_PULSES];

        for (byte_index, &byte) in bytes.iter().enumerate() {
            for bit in 0..8 {
                let pulse = if byte & (1 << bit) != 0 {
                    PulseCode::new(Level::Low, WRITE_1_LOW, Level::High, WRITE_1_RELEASE)
                } else {
                    PulseCode::new(Level::Low, WRITE_0_LOW, Level::High, WRITE_0_RELEASE)
                };
                tx_data[byte_index * 8 + bit] = pulse;
            }
        }
        tx_data[bit_count] = PulseCode::end_marker();

        self.tx.transmit(&tx_data[..=bit_count]).await?;
        Ok(())
    }

    /// Read bytes from the bus by sending read time slots and interpreting
    /// the device response. Each read slot starts with a brief master low
    /// pulse, then the device either holds the line low for a 0 bit or
    /// releases it for a 1 bit.
    pub async fn read_bytes(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() > MAX_READ_BYTES {
            return Err(Error::BufTooLarge);
        }

        let bit_count = buf.len() * 8;
        let mut tx_data = [PulseCode::end_marker(); MAX_READ_PULSES];
        let mut rx_data = [PulseCode::default(); MAX_READ_BYTES * 8];

        for slot in tx_data.iter_mut().take(bit_count) {
            *slot = PulseCode::new(Level::Low, READ_SLOT_LOW, Level::High, READ_SLOT_RELEASE);
        }
        tx_data[bit_count] = PulseCode::end_marker();

        // RX must be polled before TX here. Read slots have a 2 us master
        // low pulse — too brief for RX to catch if it starts after TX. Unlike
        // reset (500 us pulse), RX must already be listening when the bus is
        // driven low. This RX-first ordering is a known fragility that can
        // cause issues after 128+ rapid RMT ops. See onewire-rmt-debug.md.
        let (rx_result, tx_result) = join(
            self.rx.receive(&mut rx_data[..bit_count]),
            self.tx.transmit(&tx_data[..=bit_count]),
        )
        .await;
        tx_result?;
        let rx_count = rx_result?;

        if rx_count < bit_count {
            return Err(Error::RxShortRead);
        }

        buf.fill(0);
        for bit_index in 0..bit_count {
            let bit_val = decode_read_bit(rx_data[bit_index]);
            if bit_val {
                buf[bit_index / 8] |= 1 << (bit_index % 8);
            }
        }

        Ok(())
    }

    /// Write a single bit to the bus. Used by the ROM search algorithm which
    /// must operate bit-by-bit, unlike normal commands that use write_bytes.
    pub async fn write_bit(&mut self, bit: bool) -> Result<(), Error> {
        let pulse = if bit {
            PulseCode::new(Level::Low, WRITE_1_LOW, Level::High, WRITE_1_RELEASE)
        } else {
            PulseCode::new(Level::Low, WRITE_0_LOW, Level::High, WRITE_0_RELEASE)
        };
        let tx_data = [pulse, PulseCode::end_marker()];
        self.tx.transmit(&tx_data).await?;
        Ok(())
    }

    /// Read a single bit from the bus. Used by the ROM search algorithm which
    /// must operate bit-by-bit, unlike normal commands that use read_bytes.
    pub async fn read_bit(&mut self) -> Result<bool, Error> {
        let tx_data = [
            PulseCode::new(Level::Low, READ_SLOT_LOW, Level::High, READ_SLOT_RELEASE),
            PulseCode::end_marker(),
        ];
        let mut rx_data = [PulseCode::default(); 1];

        // RX before TX: 2 us read slot is too brief for late RX start.
        // See read_bytes comment for full rationale.
        let (rx_result, tx_result) =
            join(self.rx.receive(&mut rx_data), self.tx.transmit(&tx_data)).await;
        tx_result?;
        let rx_count = rx_result?;

        // Zero RX entries means the RMT hardware failed to capture the
        // waveform. Return an error rather than silently assuming a 1 bit,
        // which would corrupt ROM codes during search operations.
        if rx_count == 0 {
            return Err(Error::RxShortRead);
        }

        Ok(decode_read_bit(rx_data[0]))
    }

    /// Perform one iteration of the Maxim AN187 ROM search algorithm.
    ///
    /// Call this repeatedly with the same SearchState until it returns
    /// SearchResult::Done. Each successful call returns a found ROM code.
    pub async fn search(&mut self, state: &mut SearchState) -> Result<SearchResult, Error> {
        if state.done {
            return Ok(SearchResult::Done);
        }

        self.reset().await?;
        self.write_bytes(&[CMD_SEARCH_ROM]).await?;

        let mut new_discrepancy: Option<u8> = None;

        for bit_pos in 0u8..64 {
            // Read the bit and its complement from the bus.
            let bit_val = self.read_bit().await?;
            let complement = self.read_bit().await?;

            let byte_index = bit_pos as usize / 8;
            let bit_mask = 1 << (bit_pos % 8);

            let direction = match (bit_val, complement) {
                // All devices have 0 at this position.
                (false, true) => false,
                // All devices have 1 at this position.
                (true, false) => true,
                // Conflict: both 0 and 1 present on the bus.
                (false, false) => match state.last_discrepancy {
                    Some(d) if bit_pos == d => {
                        // Take the 1 branch this time, completing this path.
                        true
                    }
                    Some(d) if bit_pos < d => {
                        // Below the last branch: follow previous direction.
                        let took_one = state.rom_code[byte_index] & bit_mask != 0;
                        if !took_one {
                            new_discrepancy = Some(bit_pos);
                        }
                        took_one
                    }
                    _ => {
                        // New conflict past our last branch point, or first
                        // search with no prior branch: take 0.
                        new_discrepancy = Some(bit_pos);
                        false
                    }
                },
                // No devices responded.
                (true, true) => return Err(Error::NoPresence),
            };

            // Write the chosen direction back to the bus to select devices.
            self.write_bit(direction).await?;

            // Record the direction in the ROM buffer.
            if direction {
                state.rom_code[byte_index] |= bit_mask;
            } else {
                state.rom_code[byte_index] &= !bit_mask;
            }
        }

        let rom = RomCode::from_bytes(state.rom_code);
        if !rom.is_crc_valid() {
            return Err(Error::Crc);
        }

        state.last_discrepancy = new_discrepancy;
        if new_discrepancy.is_none() {
            state.done = true;
        }

        Ok(SearchResult::Found(rom))
    }

    /// Send SKIP_ROM to address all devices on the bus.
    pub async fn skip_rom(&mut self) -> Result<(), Error> {
        self.reset().await?;
        self.write_bytes(&[CMD_SKIP_ROM]).await
    }

    /// Send MATCH_ROM followed by the device address to select one device.
    pub async fn match_rom(&mut self, rom: &RomCode) -> Result<(), Error> {
        self.reset().await?;
        let mut cmd = [0u8; 9];
        cmd[0] = CMD_MATCH_ROM;
        cmd[1..9].copy_from_slice(&rom.to_bytes());
        self.write_bytes(&cmd).await
    }
}

/// Decode a received PulseCode into a bit value. If the device held the line
/// low for longer than the threshold, it signaled a 0. Otherwise 1.
fn decode_read_bit(pulse: PulseCode) -> bool {
    // The first phase should be low (master + device holding line low). If
    // it is longer than the threshold, the device is signaling 0.
    if pulse.level1() == Level::Low {
        return pulse.length1() <= READ_BIT_THRESHOLD;
    }
    // Fallback: check the second phase.
    if pulse.level2() == Level::Low {
        return pulse.length2() <= READ_BIT_THRESHOLD;
    }
    // No low pulse detected means the line stayed high, which is a 1.
    true
}

/// Patch GPIO registers to enable open-drain mode and internal pull-up
/// after RMT channel configuration resets them to push-pull.
///
/// esp-hal's RMT configure_tx applies OutputConfig::default which sets
/// push-pull with no pull resistors. There is no safe API to reconfigure
/// the pin after it has been consumed by Flex::split, so we write the
/// PAC registers directly.
fn set_open_drain_with_pullup(pin_number: u8) {
    // SAFETY: GPIO and IO_MUX are MMIO singleton registers. We only modify
    // fields for the pin we own, and no other code touches this pin's config
    // concurrently because the OneWireBus holds exclusive access to the pin.
    let gpio = unsafe { &*esp_hal::peripherals::GPIO::PTR };
    let io_mux = unsafe { &*esp_hal::peripherals::IO_MUX::PTR };

    // Enable open-drain output mode.
    gpio.pin(pin_number as usize)
        .modify(|_, w| w.pad_driver().bit(true));

    // Enable internal pull-up resistor.
    io_mux
        .gpio(pin_number as usize)
        .modify(|_, w| w.fun_wpu().bit(true));
}
