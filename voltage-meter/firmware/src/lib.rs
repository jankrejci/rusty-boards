#![no_std]
#![no_main]

use embedded_hal_bus::i2c::CriticalSectionDevice;
use esp_hal::{i2c::master::I2c, Async};

pub mod lm75;
pub mod metrics;

pub type I2cDevice = CriticalSectionDevice<'static, I2c<'static, Async>>;
