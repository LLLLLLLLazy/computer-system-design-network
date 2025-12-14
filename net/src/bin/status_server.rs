// Bring shared telemetry module into this binary
#[path = "../telemetry.rs"]
mod telemetry;

use std::net::SocketAddr;
use std::env;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use serde_json::json;

use telemetry::snapshot;

#[derive(Serialize)]
struct IfaceInfo {
    name: String,
    ip: String,
    mac: String,
}

#[derive(Serialize)]
struct Counters {
    rx_packets: u64,
    tx_packets: u64,
    crc_errors: u64,
    arp_miss: u64,
    udp_recv: u64,
    udp_send: u64,
}

#[derive(Serialize)]
struct Throughput {
    rx_bps: u64,
    tx_bps: u64,
}

#[derive(Serialize)]
struct QueueInfo {
    depth: u64,
    drops: u64,
}

#[derive(Serialize)]
struct Transfer {
    role: String,
    file: String,
    progress: f64,
    chunks_done: u64,
    chunks_total: u64,
}

#[derive(Serialize)]
struct ArpEntry {
    ip: String,
    mac: String,
    state: String,
    ttl: String,
}

#[derive(Serialize)]
struct EventRow {
    ts: String,
    kind: String,
    text: String,
}

#[derive(Serialize)]
struct Snapshot {
    iface: IfaceInfo,
    counters: Counters,
    throughput: Throughput,
    queues: Queues,
    transfers: Vec<Transfer>,
    arp: Vec<ArpEntry>,
    events: Vec<EventRow>,
}

#[derive(Serialize)]
struct Queues {
    send: QueueInfo,
    recv: QueueInfo,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO: 将此处静态示例替换为从真实模块读取的实时数据。
    let state = telemetry::init();
    let app = Router::new().route("/api/status", get(status_handler)).with_state(state);

    let port = env::var("STATUS_PORT").unwrap_or_else(|_| "5174".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
    println!("[status_server] listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn status_handler(axum::extract::State(state): axum::extract::State<&'static telemetry::Telemetry>) -> Json<serde_json::Value> {
    let snap = snapshot(state);
    Json(json!(snap))
}
