use crate::kalman::Kalman;
use defmt::{debug, info, trace};
use embassy_time::{Duration, Ticker};
use esp_hal::{
    analog::adc::{Adc, AdcCalCurve, AdcConfig, AdcPin, Attenuation},
    peripherals::{ADC1, GPIO0},
    Async,
};

use crate::metrics::{MetricsPublisher, VoltageFeedback};

pub struct AdcReader {
    adc: Adc<'static, ADC1<'static>, Async>,
    pin: AdcPin<GPIO0<'static>, ADC1<'static>, AdcCalCurve<ADC1<'static>>>,
    divider_ratio: f32,
    kalman: Kalman,
}

impl AdcReader {
    pub fn new(adc_device: ADC1<'static>, gpio_pin: GPIO0<'static>, divider_ratio: f32) -> Self {
        let mut adc_config = AdcConfig::new();
        let pin = adc_config
            .enable_pin_with_cal::<GPIO0, AdcCalCurve<ADC1>>(gpio_pin, Attenuation::_11dB);
        let adc = Adc::new(adc_device, adc_config).into_async();

        let kalman = Kalman::new(0.1, 0.5, 12.0);

        debug!("AdcReader initialized successfully");

        Self {
            adc,
            pin,
            divider_ratio,
            kalman,
        }
    }

    pub async fn read_voltage(&mut self) -> f32 {
        const NUM_SAMPLES: usize = 10;
        let mut sample_sum = 0.0;
        for _ in 0..NUM_SAMPLES {
            sample_sum += self.adc.read_oneshot(&mut self.pin).await as f32;
        }

        // Convert ADC reading to millivolts
        let voltage = self.divider_ratio * sample_sum / NUM_SAMPLES as f32 / 1000.0;
        self.kalman.update(voltage);

        debug!(
            "AdcReader, sample_avg: {} mV, voltage: {} V, kalman: {} V",
            sample_sum / NUM_SAMPLES as f32,
            voltage,
            self.kalman.value()
        );

        self.kalman.value()
    }
}

#[embassy_executor::task]
pub async fn reader_task(
    mut adc_reader: AdcReader,
    reading_period: Duration,
    metrics_publisher: MetricsPublisher,
) {
    info!("AdcReader task started");

    let mut ticker = Ticker::every(reading_period);
    loop {
        ticker.next().await;
        trace!("Measuring the voltage");

        let voltage = adc_reader.read_voltage().await;

        let metrics = VoltageFeedback::build(voltage);
        metrics_publisher.publish(metrics).await;
    }
}
