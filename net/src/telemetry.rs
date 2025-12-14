use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

#[derive(Default, Clone)]
pub struct Transfer {
    pub role: String,         // "send" or "recv"
    pub file: String,
    pub progress: f64,
    pub chunks_done: u64,
    pub chunks_total: u64,
}

#[derive(Default)]
pub struct Telemetry {
    pub iface_name: Mutex<String>,
    pub iface_ip: Mutex<String>,
    pub iface_mac: Mutex<String>,

    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub crc_errors: AtomicU64,
    pub arp_miss: AtomicU64,
    pub udp_recv: AtomicU64,
    pub udp_send: AtomicU64,

    pub send_depth: AtomicU64,
    pub send_drops: AtomicU64,
    pub recv_depth: AtomicU64,
    pub recv_drops: AtomicU64,

    pub transfer: Mutex<Option<Transfer>>,
}

pub static TELEMETRY: OnceLock<&'static Telemetry> = OnceLock::new();

pub fn init() -> &'static Telemetry {
    TELEMETRY.get_or_init(|| Box::leak(Box::new(Telemetry::default())))
}

impl Telemetry {
    pub fn set_iface(&self, name: &str, ip: &str, mac: &str) {
        *self.iface_name.lock().unwrap() = name.to_string();
        *self.iface_ip.lock().unwrap() = ip.to_string();
        *self.iface_mac.lock().unwrap() = mac.to_string();
    }

    pub fn inc_rx(&self, n: u64) {
        self.rx_packets.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_tx(&self, n: u64) {
        self.tx_packets.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_udp_recv(&self, n: u64) {
        self.udp_recv.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_udp_send(&self, n: u64) {
        self.udp_send.fetch_add(n, Ordering::Relaxed);
    }

    pub fn set_send_depth(&self, depth: u64) {
        self.send_depth.store(depth, Ordering::Relaxed);
    }
    pub fn set_recv_depth(&self, depth: u64) {
        self.recv_depth.store(depth, Ordering::Relaxed);
    }
    pub fn inc_send_drops(&self, n: u64) {
        self.send_drops.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_recv_drops(&self, n: u64) {
        self.recv_drops.fetch_add(n, Ordering::Relaxed);
    }

    pub fn start_transfer(&self, role: &str, file: &str, chunks_total: u64) {
        let mut guard = self.transfer.lock().unwrap();
        *guard = Some(Transfer {
            role: role.to_string(),
            file: file.to_string(),
            progress: 0.0,
            chunks_done: 0,
            chunks_total,
        });
    }

    pub fn update_transfer_done(&self, chunks_done: u64) {
        let mut guard = self.transfer.lock().unwrap();
        if let Some(t) = guard.as_mut() {
            t.chunks_done = chunks_done;
            if t.chunks_total > 0 {
                t.progress = (chunks_done as f64) / (t.chunks_total as f64);
            }
        }
    }

    pub fn finish_transfer(&self) {
        let mut guard = self.transfer.lock().unwrap();
        if let Some(t) = guard.as_mut() {
            t.chunks_done = t.chunks_total;
            t.progress = 1.0;
        }
    }
}

// Snapshot structs for JSON response
use axum::{routing::get, Json, Router};
use serde::Serialize;
use serde_json::json;
use tokio::runtime::Builder;

#[derive(Serialize)]
pub struct QueueInfo {
    depth: u64,
    drops: u64,
}

#[derive(Serialize)]
pub struct Snapshot {
    iface: IInfo,
    counters: Counters,
    throughput: Throughput,
    queues: Queues,
    transfers: Vec<TransferOut>,
    arp: Vec<ArpEntry>,
    events: Vec<EventRow>,
}

#[derive(Serialize)]
pub struct IInfo { pub name: String, pub ip: String, pub mac: String }
#[derive(Serialize)]
pub struct Counters { pub rx_packets: u64, pub tx_packets: u64, pub crc_errors: u64, pub arp_miss: u64, pub udp_recv: u64, pub udp_send: u64 }
#[derive(Serialize)]
pub struct Throughput { pub rx_bps: u64, pub tx_bps: u64 }
#[derive(Serialize)]
pub struct Queues { pub send: QueueInfo, pub recv: QueueInfo }
#[derive(Serialize)]
pub struct TransferOut { pub role: String, pub file: String, pub progress: f64, pub chunks_done: u64, pub chunks_total: u64 }
#[derive(Serialize)]
pub struct ArpEntry { pub ip: String, pub mac: String, pub state: String, pub ttl: String }
#[derive(Serialize)]
pub struct EventRow { pub ts: String, pub kind: String, pub text: String }

pub fn snapshot(state: &Telemetry) -> Snapshot {
    let transfer = state.transfer.lock().unwrap().clone();
    let transfers = transfer
        .into_iter()
        .map(|t| TransferOut {
            role: t.role,
            file: t.file,
            progress: t.progress,
            chunks_done: t.chunks_done,
            chunks_total: t.chunks_total,
        })
        .collect();
    Snapshot {
        iface: IInfo {
            name: state.iface_name.lock().unwrap().clone(),
            ip: state.iface_ip.lock().unwrap().clone(),
            mac: state.iface_mac.lock().unwrap().clone(),
        },
        counters: Counters {
            rx_packets: state.rx_packets.load(Ordering::Relaxed),
            tx_packets: state.tx_packets.load(Ordering::Relaxed),
            crc_errors: state.crc_errors.load(Ordering::Relaxed),
            arp_miss: state.arp_miss.load(Ordering::Relaxed),
            udp_recv: state.udp_recv.load(Ordering::Relaxed),
            udp_send: state.udp_send.load(Ordering::Relaxed),
        },
        throughput: Throughput { rx_bps: 0, tx_bps: 0 },
        queues: Queues {
            send: QueueInfo { depth: state.send_depth.load(Ordering::Relaxed), drops: state.send_drops.load(Ordering::Relaxed) },
            recv: QueueInfo { depth: state.recv_depth.load(Ordering::Relaxed), drops: state.recv_drops.load(Ordering::Relaxed) },
        },
        transfers,
        arp: vec![],
        events: vec![],
    }
}

// Spawn a lightweight HTTP status server in-process so telemetry is shared with the running binary.
pub fn spawn_status_server(addr: &str) -> anyhow::Result<JoinHandle<()>> {
    let state = init();
    let addr: SocketAddr = addr.parse()?;

    // Build a dedicated Tokio runtime for the server.
    let rt = Builder::new_multi_thread().enable_all().build()?;

    let handle = std::thread::spawn(move || {
        let listener = rt.block_on(async { tokio::net::TcpListener::bind(addr).await })
            .expect("bind status server");

        let app = Router::new()
            .route("/api/status", get(|axum::extract::State(state): axum::extract::State<&'static Telemetry>| async move {
                let snap = snapshot(state);
                Json(json!(snap))
            }))
            .with_state(state);

        rt.block_on(async {
            axum::serve(listener, app).await.expect("serve status server");
        });
    });

    Ok(handle)
}
