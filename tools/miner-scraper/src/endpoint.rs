//! Miner endpoint abstraction and cgminer TCP client.
//!
//! Provides the `Endpoint` enum representing cgminer API commands or HTTP
//! paths with associated scrape tiers, firmware detection, and the low-level
//! cgminer socket client.

use std::net::IpAddr;
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[cfg(test)]
#[path = "tests/endpoint.rs"]
mod tests;

/// Timeout for establishing a TCP connection to a miner.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Timeout for reading a complete response from a miner.
const READ_TIMEOUT: Duration = Duration::from_secs(3);

/// Default cgminer API port.
const CGMINER_PORT: u16 = 4028;

/// Scrape frequency tier for endpoints.
///
/// Groups endpoints by how often their data changes, allowing the scraper
/// to poll real-time hardware data frequently while checking stable
/// configuration data less often.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrapeTier {
    /// Real-time hardware data: temperatures, hashrates, fan speeds.
    High,
    /// Aggregated data: summaries, device details, voltage.
    Mid,
    /// Stable configuration: version, pools, coin settings.
    Low,
}

impl ScrapeTier {
    /// Return tiers in priority order from highest to lowest.
    pub const fn priority_order() -> [ScrapeTier; 3] {
        [ScrapeTier::High, ScrapeTier::Mid, ScrapeTier::Low]
    }
}

/// Response from a miner endpoint.
pub enum Response {
    /// Parsed JSON from a cgminer TCP command.
    Json(Value),
    /// Raw text from an HTTP endpoint.
    Text(String),
}

/// Miner communication endpoint.
///
/// Represents a specific command or path to query on a miner, paired with
/// its scrape tier. The cgminer variant sends JSON commands over TCP port
/// 4028. The HTTP variant sends GET requests to a firmware-specific port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endpoint {
    /// Cgminer TCP API command (e.g. "stats", "summary").
    Cgminer(&'static str, ScrapeTier),
    /// HTTP endpoint path with port (e.g. "readvol" on port 6060).
    Http(&'static str, u16, ScrapeTier),
}

/// All known endpoints for miner scraping.
///
/// Each endpoint specifies the protocol, command or path, and scrape tier.
/// Endpoints are probed sequentially on first contact to discover which
/// ones the miner supports.
pub const ENDPOINTS: &[Endpoint] = &[
    // High: real-time hardware data.
    Endpoint::Cgminer("stats", ScrapeTier::High),
    // Mid: aggregated data and device details.
    Endpoint::Cgminer("summary", ScrapeTier::Mid),
    Endpoint::Cgminer("devs", ScrapeTier::Mid),
    Endpoint::Cgminer("devdetails", ScrapeTier::Mid),
    Endpoint::Cgminer("temps", ScrapeTier::Mid),
    Endpoint::Cgminer("fans", ScrapeTier::Mid),
    Endpoint::Cgminer("power", ScrapeTier::Mid),
    Endpoint::Http("readvol", 6060, ScrapeTier::Mid),
    // Low: stable configuration data.
    Endpoint::Cgminer("pools", ScrapeTier::Low),
    Endpoint::Cgminer("version", ScrapeTier::Low),
    Endpoint::Cgminer("tunerstatus", ScrapeTier::Low),
    Endpoint::Cgminer("tempctrl", ScrapeTier::Low),
    Endpoint::Cgminer("profiles", ScrapeTier::Low),
    Endpoint::Cgminer("limits", ScrapeTier::Low),
    Endpoint::Cgminer("config", ScrapeTier::Low),
    Endpoint::Cgminer("events", ScrapeTier::Low),
    Endpoint::Cgminer("healthctrl", ScrapeTier::Low),
    Endpoint::Cgminer("hashboardopts", ScrapeTier::Low),
    Endpoint::Cgminer("atm", ScrapeTier::Low),
    Endpoint::Cgminer("poolopts", ScrapeTier::Low),
    Endpoint::Cgminer("coin", ScrapeTier::Low),
];

impl Endpoint {
    /// Return the command or path string for this endpoint.
    pub fn command(self) -> &'static str {
        match self {
            Self::Cgminer(cmd, _) | Self::Http(cmd, _, _) => cmd,
        }
    }

    /// Return the scrape tier for this endpoint.
    pub fn tier(self) -> ScrapeTier {
        match self {
            Self::Cgminer(_, tier) | Self::Http(_, _, tier) => tier,
        }
    }

    /// Send this endpoint's command to the given host and return the response.
    ///
    /// For cgminer endpoints, sends a JSON command over TCP port 4028.
    /// For HTTP endpoints, sends a GET request to the path on the given port.
    pub async fn fetch(self, host: IpAddr) -> Result<Response> {
        match self {
            Self::Cgminer(cmd, _) => cgminer_command(host, cmd).await.map(Response::Json),
            Self::Http(path, port, _) => http_fetch(host, port, path).await.map(Response::Text),
        }
    }
}

/// Known firmware types for Bitcoin mining hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    Stock,
    LuxOS,
    Vnish,
    Braiins,
    Mara,
}

impl std::fmt::Display for Firmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Firmware::Stock => write!(f, "stock"),
            Firmware::LuxOS => write!(f, "LuxOS"),
            Firmware::Vnish => write!(f, "vnish"),
            Firmware::Braiins => write!(f, "BraiinsOS"),
            Firmware::Mara => write!(f, "MaraFW"),
        }
    }
}

impl Firmware {
    /// Determine firmware from a stats response.
    ///
    /// Checks the STATUS Description field for `BraiinsOS`, `LuxOS`, and MARA
    /// identifiers. Falls back to the STATS Type field for `Vnish`. Returns
    /// stock firmware if nothing matches.
    pub fn identify(response: &Response) -> Firmware {
        let Response::Json(stats) = response else {
            return Firmware::Stock;
        };

        let description = stats
            .pointer("/STATUS/0/Description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let type_field = stats
            .pointer("/STATS/0/Type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match (description, type_field) {
            (desc, _) if desc.contains("BOSer") => Firmware::Braiins,
            (desc, _) if desc.contains("LUXminer") => Firmware::LuxOS,
            (desc, _) if desc.contains("kaonsu") => Firmware::Mara,
            (_, typ) if typ.contains("(Vnish") => Firmware::Vnish,
            _ => Firmware::Stock,
        }
    }
}

/// Check whether a response indicates an error.
///
/// Only JSON responses can be errors. Inspects the STATUS array for an
/// "E" (error) status code.
pub(crate) fn is_error(response: &Response) -> bool {
    let Response::Json(value) = response else {
        return false;
    };
    value
        .get("STATUS")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("STATUS"))
        .and_then(|s| s.as_str())
        == Some("E")
}

/// Send a cgminer command over TCP and return the parsed JSON response.
///
/// Connects to the cgminer API on the specified host and default port,
/// sends the command as a JSON message, reads the response, strips the
/// trailing NUL byte that cgminer appends, and parses the result.
async fn cgminer_command(host: IpAddr, cmd: &str) -> Result<Value> {
    let addr = format!("{host}:{CGMINER_PORT}");

    let mut stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
        .await
        .map_err(|_| anyhow::anyhow!("connect timeout: {addr}"))??;

    let request = format!("{{\"command\":\"{cmd}\"}}\n");

    stream.write_all(request.as_bytes()).await?;
    stream.shutdown().await?;

    let mut buf = Vec::with_capacity(4096);
    tokio::time::timeout(READ_TIMEOUT, stream.read_to_end(&mut buf))
        .await
        .map_err(|_| anyhow::anyhow!("read timeout: {addr}"))??;

    // Cgminer terminates responses with a NUL byte.
    if buf.last() == Some(&0) {
        buf.pop();
    }

    let value: Value = serde_json::from_slice(&buf)?;
    Ok(value)
}

/// Fetch a plain text response from an HTTP endpoint on a miner.
async fn http_fetch(host: IpAddr, port: u16, path: &str) -> Result<String> {
    let url = format!("http://{host}:{port}/{path}");
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(READ_TIMEOUT)
        .build()?;
    let response = client.get(&url).send().await?;
    Ok(response.text().await?)
}
