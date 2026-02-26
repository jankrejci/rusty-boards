//! Prometheus metrics collection and serial export.
//!
//! Two independent output channels carry different data:
//! - defmt over RTT/JTAG for compressed binary debug logging
//! - esp_println over USB-serial for plain-text Prometheus metrics
//!
//! The serial output is dependency-free: any host can read metrics by opening
//! the serial port. No special tooling or protocol decoding required.

use defmt::{debug, info};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{self, PubSubChannel};
use embassy_time::Instant;
use esp_println::println;

use crate::config::MetricsConfig;

pub static METRICS_CHANNEL: MetricsChannel = MetricsChannel::new();

type MetricsChannel = PubSubChannel<
    CriticalSectionRawMutex,
    Metrics,
    { MetricsConfig::CHANNEL_SIZE },
    { MetricsConfig::NUM_SUBSCRIBERS },
    { MetricsConfig::NUM_PUBLISHERS },
>;
pub type MetricsSubscriber = pubsub::Subscriber<
    'static,
    CriticalSectionRawMutex,
    Metrics,
    { MetricsConfig::CHANNEL_SIZE },
    { MetricsConfig::NUM_SUBSCRIBERS },
    { MetricsConfig::NUM_PUBLISHERS },
>;
pub type MetricsPublisher = pubsub::Publisher<
    'static,
    CriticalSectionRawMutex,
    Metrics,
    { MetricsConfig::CHANNEL_SIZE },
    { MetricsConfig::NUM_SUBSCRIBERS },
    { MetricsConfig::NUM_PUBLISHERS },
>;

/// Generate a `Metrics` enum with a variant for each metric type and a
/// `Display` impl that dispatches to the inner type's `Display`. This
/// enables type-safe metric publishing through the PubSub channel.
macro_rules! create_metrics {
    ($($metric:ident),* $(,)?) => {
        #[derive(Clone, defmt::Format)]
        pub enum Metrics {
            $(
                $metric($metric),
            )*
        }

        impl core::fmt::Display for Metrics {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self {
                    $(
                        Self::$metric(metrics) => metrics.fmt(f),
                    )*
                }
            }
        }
    };
}

create_metrics!(SensorTemperature);

#[derive(Clone, defmt::Format)]
pub struct SensorTemperature {
    pub serial_number: u64,
    /// Temperature in millidegrees Celsius. Stored as integer to avoid
    /// software float emulation on ESP32-C3 RISC-V which has no FPU.
    pub millidegrees: i32,
    pub timestamp_ms: u64,
}

impl SensorTemperature {
    pub fn build(serial_number: u64, millidegrees: i32) -> Metrics {
        let timestamp_ms = Instant::now().as_millis();
        Metrics::SensorTemperature(Self {
            serial_number,
            millidegrees,
            timestamp_ms,
        })
    }
}

impl core::fmt::Display for SensorTemperature {
    /// Prometheus text format. Metric name follows Prometheus naming conventions:
    /// base unit suffix `_celsius` per https://prometheus.io/docs/practices/naming/
    /// Value is degrees Celsius formatted from integer millidegrees without FPU.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let abs = self.millidegrees.unsigned_abs();
        let sign = if self.millidegrees < 0 { "-" } else { "" };
        write!(
            f,
            "temperature_celsius{{sensor=\"{:08X}\"}} {}{}.{:03} {}",
            self.serial_number,
            sign,
            abs / 1000,
            abs % 1000,
            self.timestamp_ms,
        )
    }
}

pub struct MetricsExporter {
    metrics_subscriber: MetricsSubscriber,
}

impl MetricsExporter {
    pub fn new(metrics_subscriber: MetricsSubscriber) -> Self {
        debug!("MetricsExporter initialized");
        Self { metrics_subscriber }
    }
}

#[embassy_executor::task]
pub async fn metrics_exporter_task(mut handler: MetricsExporter) {
    info!("MetricsExporter task started");

    loop {
        let metrics = handler.metrics_subscriber.next_message_pure().await;
        println!("{}", metrics);
    }
}
