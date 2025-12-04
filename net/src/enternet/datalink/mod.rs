mod recv;
mod send;

use anyhow::{Context, Result};
use pcap::{Active, Capture};

pub use recv::datalink_recv;
pub use send::datalink_send;

pub(crate) fn open_device(name: &str) -> Result<Capture<Active>> {
    Capture::from_device(name)
        .context("定位网卡失败")?
        .promisc(true)
        .snaplen(65_536)
        .timeout(1_000)
        .open()
        .with_context(|| format!("无法打开设备 {name}"))
}
