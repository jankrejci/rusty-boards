use defmt::{debug, info, trace};
use embassy_time::{Duration, Ticker};
use esp_hal::{
    analog::adc::{Adc, AdcCalCurve, AdcChannel, AdcConfig, AdcPin, Attenuation},
    gpio::AnalogPin,
    Async,
};

use crate::config as cfg;
use crate::metrics::{MetricsPublisher, VoltageFeedback};

pub type AdcCal = AdcCalCurve<cfg::AdcDevice>;
pub type VoltageReader = AdcReader<'static, cfg::AdcDevice, cfg::AdcPin, AdcCal>;

pub struct AdcReader<'d, ADCI, PIN, CS> {
    device: Adc<'d, ADCI, Async>,
    pin: AdcPin<PIN, ADCI, CS>,
    divider_ratio: f32,
}

impl<'d, PIN> AdcReader<'d, cfg::AdcDevice, PIN, AdcCal>
where
    PIN: AnalogPin + AdcChannel,
{
    pub fn new(adc_deivce: cfg::AdcDevice, gpio_pin: PIN, divider_ratio: f32) -> Self {
        let mut adc_config = AdcConfig::new();
        let pin = adc_config.enable_pin_with_cal::<PIN, AdcCal>(gpio_pin, Attenuation::_11dB);
        let device = Adc::new(adc_deivce, adc_config).into_async();

        debug!("AdcReader initialized successfully");
        Self {
            device,
            pin,
            divider_ratio,
        }
    }

    pub async fn read_voltage(&mut self) -> f32 {
        let sample = self.device.read_oneshot(&mut self.pin).await;
        let voltage = self.divider_ratio * sample as f32 / 1000.0;

        trace!("AdcReader, sample: {}, voltage: {} V", sample, voltage,);

        voltage
    }
}

#[embassy_executor::task]
pub async fn reader_task(
    mut reader: VoltageReader,
    reading_period: Duration,
    metrics_publisher: MetricsPublisher,
) {
    info!("AdcReader task started");

    let mut ticker = Ticker::every(reading_period);
    loop {
        ticker.next().await;
        trace!("Measuring the voltage");

        let voltage = reader.read_voltage().await;
        let metrics = VoltageFeedback::build(voltage);
        metrics_publisher.publish(metrics).await;
    }
}
