//! Blocking serial port reader for ESP32 sensor metrics.
//!
//! Opens a serial port at 115200 baud and reads lines in a blocking thread.
//! Valid Prometheus metric lines are timestamped and sent as batches through
//! an mpsc channel. Batch boundaries are detected after 100ms of silence.

use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::store::{is_valid_metric_line, stamp_metric_line};

/// A validated batch of Prometheus metric lines from a single serial read cycle.
pub struct MetricBatch {
    pub port: String,
    pub lines: Vec<String>,
}

/// Spawn a blocking serial reader task for a given port.
///
/// Opens the serial port at 115200 baud, reads lines, validates them as
/// Prometheus metrics, and sends complete batches through the channel.
/// A batch boundary is detected after 100ms of silence (no new lines),
/// matching the firmware's periodic output cycle.
pub fn spawn_reader(
    port_name: String,
    tx: mpsc::Sender<MetricBatch>,
    token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = read_loop(&port_name, &tx, &token) {
            if !token.is_cancelled() {
                log::warn!("{}: reader error: {}", port_name, e);
            }
        }
        log::info!("{}: reader stopped", port_name);
    })
}

fn read_loop(
    port_name: &str,
    tx: &mpsc::Sender<MetricBatch>,
    token: &CancellationToken,
) -> anyhow::Result<()> {
    let port = serialport::new(port_name, 115_200)
        .timeout(Duration::from_millis(100))
        .open()?;

    log::info!("{}: reader started", port_name);
    let mut reader = BufReader::new(port);
    let mut batch = Vec::new();
    let mut line_buf = String::new();

    loop {
        if token.is_cancelled() {
            return Ok(());
        }

        line_buf.clear();
        match reader.read_line(&mut line_buf) {
            Ok(0) => return Ok(()), // EOF
            Ok(_) => {
                let trimmed = line_buf.trim_end();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if is_valid_metric_line(trimmed) {
                    batch.push(stamp_metric_line(trimmed));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // 100ms silence: flush the accumulated batch.
                if !batch.is_empty() {
                    let lines = std::mem::take(&mut batch);
                    let msg = MetricBatch {
                        port: port_name.to_owned(),
                        lines,
                    };
                    if tx.blocking_send(msg).is_err() {
                        return Ok(()); // Receiver dropped, shutting down.
                    }
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}
