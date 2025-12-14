use anyhow::{Context, Result, anyhow};
use std::{fs, thread, time::Duration};

mod cli;
mod config;
mod enternet;
mod icmp;
mod ip;
mod udp;
mod telemetry;

use cli::cli::{Mode, parse_cli};
use enternet::datalink::{datalink_recv, datalink_send};
use enternet::net::{iface_ipv4, iface_mac};
use udp::{bind, closesocket, recvfrom, sendto, socket_on_iface, SockAddrIn};
use telemetry::{init as telemetry_init, spawn_status_server};
use std::env;

fn main() -> Result<()> {
    let args = parse_cli()?;
    let src_mac = iface_mac(&args.iface)?;
    let src_ip = iface_ipv4(&args.iface)?;

    let tel = telemetry_init();
    tel.set_iface(&args.iface, &crate::enternet::frame::fmt_ipv4(&src_ip), &crate::enternet::frame::fmt_mac(&src_mac));

    // Start in-process status server so the frontend can read live telemetry from this run.
    let status_port = env::var("STATUS_PORT").unwrap_or_else(|_| "5174".to_string());
    let _status_server = spawn_status_server(&format!("0.0.0.0:{status_port}"));
    println!(
        "启动参数: iface={} 本机IP={} 本机MAC={}",
        &args.iface,
        crate::enternet::frame::fmt_ipv4(&src_ip),
        crate::enternet::frame::fmt_mac(&src_mac)
    );
    match args.mode {
        Mode::Send {
            dest_ip,
            protocol,
            manual_dest_mac,
        } => datalink_send(
            &args.iface,
            src_mac,
            src_ip,
            dest_ip,
            protocol,
            manual_dest_mac,
        ),
        Mode::Recv => datalink_recv(&args.iface, src_mac, src_ip),
        Mode::UdpSendFile {
            dest_ip,
            dest_port,
            src_port,
            file,
        } => run_udp_send_file(&args.iface, dest_ip, dest_port, src_port, &file),
        Mode::UdpRecvFile { listen_port, output } => {
            run_udp_recv_file(&args.iface, listen_port, &output)
        }
    }
}

const UDP_CHUNK: usize = 1200;

fn run_udp_send_file(
    iface: &str,
    dest_ip: [u8; 4],
    dest_port: u16,
    src_port: Option<u16>,
    path: &str,
) -> Result<()> {
    let data = fs::read(path).with_context(|| format!("读取文件 {path} 失败"))?;
    if data.is_empty() {
        return Err(anyhow!("文件为空"));
    }

    let tel = telemetry_init();

    let sock = socket_on_iface(iface)?;
    if let Some(port) = src_port {
        bind(
            sock,
            SockAddrIn {
                ip: iface_ipv4(iface)?,
                port,
            },
        )?;
    }

    let total_chunks = ((data.len() + UDP_CHUNK - 1) / UDP_CHUNK) as u32;
    tel.start_transfer("send", path, total_chunks as u64);
    println!(
        "[UDP SEND] iface={} dst={}:{} file={} size={}B chunks={}",
        iface,
        enternet::frame::fmt_ipv4(&dest_ip),
        dest_port,
        path,
        data.len(),
        total_chunks
    );

    for (idx, chunk) in data.chunks(UDP_CHUNK).enumerate() {
        let idx_u32 = idx as u32;
        let mut buf = Vec::with_capacity(12 + chunk.len());
        buf.extend_from_slice(&total_chunks.to_be_bytes());
        buf.extend_from_slice(&idx_u32.to_be_bytes());
        buf.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        buf.extend_from_slice(chunk);

        let sent = sendto(
            sock,
            &buf,
            0,
            SockAddrIn {
                ip: dest_ip,
                port: dest_port,
            },
        )?;
        println!("[UDP SEND] chunk {}/{} bytes={} sent={}", idx + 1, total_chunks, chunk.len(), sent);
        tel.inc_udp_send(1);
        tel.inc_tx(1);
        tel.update_transfer_done((idx_u32 + 1) as u64);
        // 轻微节流，避免过快导致丢包
        thread::sleep(Duration::from_millis(2));
    }

    tel.finish_transfer();

    closesocket(sock).ok();
    Ok(())
}

fn run_udp_recv_file(iface: &str, listen_port: u16, output: &str) -> Result<()> {
    let local_ip = iface_ipv4(iface)?;
    let local_mac = iface_mac(iface)?;
    let tel = telemetry_init();

    // 启动链路层接收线程，用于将 UDP 报文交付到 socket 队列。
    let iface_name = iface.to_string();
    thread::spawn(move || {
        if let Err(err) = datalink_recv(&iface_name, local_mac, local_ip) {
            eprintln!("[UDP RECV] datalink 线程异常: {err}");
        }
    });

    let sock = socket_on_iface(iface)?;
    bind(sock, SockAddrIn { ip: local_ip, port: listen_port })?;
    println!(
        "[UDP RECV] iface={} listen={} file={}",
        iface,
        listen_port,
        output
    );

    let mut total_chunks = None;
    let mut received = Vec::new();

    loop {
        let mut buf = vec![0u8; 2048];
        let (n, src) = recvfrom(sock, &mut buf, 0)?;
        if n < 12 {
            eprintln!("[UDP RECV] 丢弃过短分片 len={}", n);
            continue;
        }
        let recv_total = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let idx = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        let chunk_len = u32::from_be_bytes(buf[8..12].try_into().unwrap()) as usize;
        if 12 + chunk_len > n {
            eprintln!("[UDP RECV] 长度不匹配 idx={} len={} n={}", idx, chunk_len, n);
            continue;
        }
        let payload = &buf[12..12 + chunk_len];
        if total_chunks.is_none() {
            total_chunks = Some(recv_total as usize);
            received.resize(recv_total as usize, None);
            tel.start_transfer("recv", output, recv_total as u64);
            println!(
                "[UDP RECV] 开始接收: 源 {}:{} chunks={} 文件={}",
                enternet::frame::fmt_ipv4(&src.ip),
                src.port,
                recv_total,
                output
            );
        }

        if let Some(total) = total_chunks {
            if idx as usize >= total {
                eprintln!("[UDP RECV] 分片索引超界 idx={} total={}", idx, total);
                continue;
            }
            if received[idx as usize].is_none() {
                received[idx as usize] = Some(payload.to_vec());
            }

            tel.inc_udp_recv(1);
            tel.inc_rx(1);
            tel.update_transfer_done((idx as u64) + 1);

            let have = received.iter().filter(|c| c.is_some()).count();
            println!("[UDP RECV] 收到分片 {}/{} (idx={}) len={}", have, total, idx, chunk_len);

            if have == total {
                let mut file_data = Vec::new();
                for chunk in received.into_iter() {
                    file_data.extend_from_slice(&chunk.unwrap());
                }
                fs::write(output, &file_data).with_context(|| format!("写入 {output} 失败"))?;
                println!("[UDP RECV] 文件接收完成: 写入 {} ({}B)", output, file_data.len());
                tel.finish_transfer();
                closesocket(sock).ok();
                break;
            }
        }
    }

    Ok(())
}
