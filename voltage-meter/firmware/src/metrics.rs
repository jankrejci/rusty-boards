use defmt::{debug, info};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{self, PubSubChannel};
use embassy_time::Instant;
use esp_println::println;

const METRICS_CHANNEL_SIZE: usize = 10;
const NUM_SUBSCRIBERS: usize = 2;
const NUM_PUBLISHERS: usize = 2;
pub static METRICS_CHANNEL: MetricsChannel = MetricsChannel::new();

type MetricsChannel = PubSubChannel<
    CriticalSectionRawMutex,
    Metrics,
    METRICS_CHANNEL_SIZE,
    NUM_SUBSCRIBERS,
    NUM_PUBLISHERS,
>;
pub type MetricsPublisher = pubsub::Publisher<
    'static,
    CriticalSectionRawMutex,
    Metrics,
    METRICS_CHANNEL_SIZE,
    NUM_SUBSCRIBERS,
    NUM_PUBLISHERS,
>;
pub type MetricsSubscriber = pubsub::Subscriber<
    'static,
    CriticalSectionRawMutex,
    Metrics,
    METRICS_CHANNEL_SIZE,
    NUM_SUBSCRIBERS,
    NUM_PUBLISHERS,
>;

macro_rules! create_metrics {
    ($($metric:ident),* $(,)?) => {
        #[derive(Clone)]
        pub enum Metrics {
            $(
                $metric($metric),
            )*
        }

        impl core::fmt::Display for Metrics{
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

create_metrics!(AmbientTemperature);

#[derive(Clone)]
pub struct AmbientTemperature {
    pub timestamp_ms: u64,
    pub temperature: f32,
}

impl AmbientTemperature {
    pub fn build(temperature: f32) -> Metrics {
        let timestamp_ms = Instant::now().as_millis();
        Metrics::AmbientTemperature(Self {
            timestamp_ms,
            temperature,
        })
    }
}

impl core::fmt::Display for AmbientTemperature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "ambient_temperature{{unit=\"C\"}} {} {}",
            self.temperature, self.timestamp_ms,
        )
    }
}

pub struct MetricsExporter {
    metrics_subscriber: MetricsSubscriber,
}

impl MetricsExporter {
    pub fn new(metrics_subscriber: MetricsSubscriber) -> Self {
        debug!("MetricsPublisher initialized succesfully");
        Self { metrics_subscriber }
    }
}

#[embassy_executor::task]
pub async fn metrics_exporter_task(mut handler: MetricsExporter) {
    info!("MetricsHandler task started");

    loop {
        let metrics = handler.metrics_subscriber.next_message_pure().await;
        println!("\n{}", metrics);
    }
}
