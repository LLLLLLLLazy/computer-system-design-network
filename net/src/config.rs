use std::{env, sync::OnceLock};

/// 默认子网掩码：255.255.255.0
const DEFAULT_SUBNET_MASK: [u8; 4] = [255, 255, 255, 0];
/// 默认网关：192.168.1.1
const DEFAULT_GATEWAY_IP: [u8; 4] = [192, 168, 1, 1];
/// 默认 DNS 服务器 1：114.114.114.114（国内常用）
const DEFAULT_DNS1: [u8; 4] = [114, 114, 114, 114];
/// 默认 DNS 服务器 2：8.8.8.8（Google DNS）
const DEFAULT_DNS2: [u8; 4] = [8, 8, 8, 8];
/// 默认 DHCP 状态：启用
const DEFAULT_DHCP_ENABLED: bool = true;

static PROFILE: OnceLock<NetworkProfile> = OnceLock::new();

/// 表示网络层的静态配置，用于 ARP 与路由决策。
#[derive(Debug, Clone, Copy)]
pub struct NetworkProfile {
    pub subnet_mask: [u8; 4],
    pub gateway_ip: [u8; 4],
    pub dns_servers: [[u8; 4]; 2],
    pub dhcp_enabled: bool,
}

impl NetworkProfile {
    fn load() -> Self {
        let subnet_mask = env::var("NET_SUBNET_MASK")
            .ok()
            .and_then(|raw| parse_ipv4(&raw))
            .unwrap_or(DEFAULT_SUBNET_MASK);
        let gateway_ip = env::var("NET_GATEWAY_IP")
            .ok()
            .and_then(|raw| parse_ipv4(&raw))
            .unwrap_or(DEFAULT_GATEWAY_IP);
        let dns1 = env::var("NET_DNS1")
            .ok()
            .and_then(|raw| parse_ipv4(&raw))
            .unwrap_or(DEFAULT_DNS1);
        let dns2 = env::var("NET_DNS2")
            .ok()
            .and_then(|raw| parse_ipv4(&raw))
            .unwrap_or(DEFAULT_DNS2);
        let dhcp_enabled = env::var("NET_DHCP")
            .ok()
            .map(|raw| {
                matches!(
                    raw.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(DEFAULT_DHCP_ENABLED);

        Self {
            subnet_mask,
            gateway_ip,
            dns_servers: [dns1, dns2],
            dhcp_enabled,
        }
    }
}

/// 获取全局网络配置（允许通过环境变量覆盖默认值）。
pub fn network_profile() -> &'static NetworkProfile {
    PROFILE.get_or_init(NetworkProfile::load)
}

fn parse_ipv4(input: &str) -> Option<[u8; 4]> {
    let mut bytes = [0u8; 4];
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    for (idx, part) in parts.iter().enumerate() {
        let value: u8 = part.trim().parse().ok()?;
        bytes[idx] = value;
    }
    Some(bytes)
}
