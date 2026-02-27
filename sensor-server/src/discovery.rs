//! Serial port autodiscovery via inotify.
//!
//! Watches /dev/ for device node creation and deletion. When an ESP32-C3 USB
//! serial port appears or disappears, spawns or tears down the corresponding
//! serial reader task. Falls back to periodic polling if inotify is unavailable.

use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;

use inotify::{Inotify, WatchMask};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::serial::{self, MetricBatch};
use crate::store::MetricsStore;

/// Espressif USB VID.
const ESPRESSIF_VID: u16 = 0x303a;

/// ESP32-C3 USB-JTAG PID.
const ESP32_C3_PID: u16 = 0x1001;

/// Fallback poll interval when no inotify events arrive.
const FALLBACK_POLL: Duration = Duration::from_secs(60);

/// Buffer size for inotify event reads.
const INOTIFY_BUF_SIZE: usize = 512;

type DeviceEventStream = inotify::EventStream<[u8; INOTIFY_BUF_SIZE]>;

struct ReaderHandle {
    token: CancellationToken,
    join: JoinHandle<()>,
}

/// Run the discovery loop with inotify for instant detection and a fallback poll.
///
/// Watches /dev/ for device node creation/deletion. When a ttyACM device
/// appears or disappears, triggers an immediate scan. Falls back to periodic
/// polling every 60 seconds as a safety net.
pub async fn run(
    tx: mpsc::Sender<MetricBatch>,
    store: MetricsStore,
    parent_token: CancellationToken,
) {
    let mut readers: HashMap<String, ReaderHandle> = HashMap::new();
    let mut warned_no_sensors = false;

    let inotify = match Inotify::init() {
        Ok(i) => {
            match i
                .watches()
                .add("/dev/", WatchMask::CREATE | WatchMask::DELETE)
            {
                Ok(_) => {
                    log::info!("watching /dev/ for device changes");
                    Some(i)
                }
                Err(e) => {
                    log::warn!("failed to watch /dev/: {}, falling back to polling", e);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("failed to init inotify: {}, falling back to polling", e);
            None
        }
    };

    // into_event_stream consumes the Inotify instance. If it fails, the fd is
    // dropped and we fall back to polling permanently. This is acceptable since
    // event stream creation failures indicate a systemic issue, not a transient one.
    let mut event_stream =
        inotify.and_then(|i| match i.into_event_stream([0u8; INOTIFY_BUF_SIZE]) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!(
                    "failed to create event stream: {}, falling back to polling",
                    e
                );
                None
            }
        });

    loop {
        if parent_token.is_cancelled() {
            break;
        }

        reconcile(&mut readers, &tx, &store, &parent_token).await;

        if readers.is_empty() && !warned_no_sensors {
            log::warn!("no sensors found");
            warned_no_sensors = true;
        } else if !readers.is_empty() {
            warned_no_sensors = false;
        }

        // Wait for a device event, fallback timeout, or shutdown.
        tokio::select! {
            _ = wait_for_device_event(&mut event_stream) => {}
            _ = tokio::time::sleep(FALLBACK_POLL) => {}
            _ = parent_token.cancelled() => break,
        }
    }

    // Cancel all remaining readers on shutdown.
    for (port, handle) in readers {
        log::info!("{}: shutting down reader", port);
        handle.token.cancel();
        let _ = handle.join.await;
    }
}

/// Wait for an inotify event on /dev/, or pend forever if inotify is unavailable.
///
/// When inotify is not available, this future never resolves. The caller's
/// select! uses a separate fallback sleep to handle the polling case.
async fn wait_for_device_event(stream: &mut Option<DeviceEventStream>) {
    use futures_core::Stream;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    match stream {
        Some(ref mut s) => {
            std::future::poll_fn(
                |cx: &mut Context<'_>| match Pin::new(&mut *s).poll_next(cx) {
                    Poll::Ready(Some(Ok(_))) => Poll::Ready(()),
                    Poll::Ready(Some(Err(e))) => {
                        log::warn!("inotify error: {}", e);
                        Poll::Ready(())
                    }
                    Poll::Ready(None) => Poll::Ready(()),
                    Poll::Pending => Poll::Pending,
                },
            )
            .await;
        }
        None => std::future::pending().await,
    }
}

/// Reconcile the set of active readers with the set of live ports.
async fn reconcile(
    readers: &mut HashMap<String, ReaderHandle>,
    tx: &mpsc::Sender<MetricBatch>,
    store: &MetricsStore,
    parent_token: &CancellationToken,
) {
    // Remove readers whose tasks have finished (device disconnected, read error).
    let dead: Vec<String> = readers
        .iter()
        .filter(|(_, h)| h.join.is_finished())
        .map(|(p, _)| p.clone())
        .collect();

    for port in dead {
        if let Some(handle) = readers.remove(&port) {
            let _ = handle.join.await;
            store.remove(&port).await;
            log::info!("{}: reader exited, cleared metrics", port);
        }
    }

    let live_ports = scan_ports();

    // Tear down readers for ports that disappeared but whose tasks are still running.
    let stale: Vec<String> = readers
        .keys()
        .filter(|p| !live_ports.contains(p.as_str()))
        .cloned()
        .collect();

    for port in stale {
        if let Some(handle) = readers.remove(&port) {
            log::info!("{}: port disappeared, stopping reader", port);
            handle.token.cancel();
            let _ = handle.join.await;
            store.remove(&port).await;
        }
    }

    // Spawn readers for new ports.
    for port in &live_ports {
        if !readers.contains_key(port) {
            log::info!("{}: port discovered, starting reader", port);
            let token = parent_token.child_token();
            let join = serial::spawn_reader(port.clone(), tx.clone(), token.clone());
            readers.insert(port.clone(), ReaderHandle { token, join });
        }
    }
}

/// Scan for serial ports matching the Espressif ESP32-C3 VID/PID.
fn scan_ports() -> HashSet<String> {
    let ports = match serialport::available_ports() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("failed to enumerate serial ports: {}", e);
            return HashSet::new();
        }
    };

    ports
        .into_iter()
        .filter_map(|p| {
            if let serialport::SerialPortType::UsbPort(usb) = &p.port_type {
                if usb.vid == ESPRESSIF_VID && usb.pid == ESP32_C3_PID {
                    return Some(p.port_name);
                }
            }
            None
        })
        .collect()
}
