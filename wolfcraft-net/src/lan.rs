//! Playing together without typing addresses: a hosted server announces itself on every local
//! network (Wi-Fi, the phone's own hotspot, cable) with a small UDP broadcast once a second, and
//! the "Рядом" list picks those announcements up. Short invite codes stand in for "IP:port".
//!
//! Mobile data cannot carry a direct game: operators block incoming connections (CGNAT). The
//! way to play "through the mobile network" is the host phone's hotspot: friends join its
//! Wi-Fi (no internet needed for that) and see the server in "Рядом".

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// UDP port of the server announcements.
pub const BEACON_PORT: u16 = 25571;
const MAGIC: &str = "WOLFCRAFT1";

/// What kind of network an interface is, for the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetKind {
    Wifi,
    Hotspot,
    Mobile,
    Cable,
    Other,
}

impl NetKind {
    pub fn label(self) -> &'static str {
        match self {
            NetKind::Wifi => "Wi-Fi",
            NetKind::Hotspot => "Точка доступа",
            NetKind::Mobile => "Мобильный интернет",
            NetKind::Cable => "Кабель",
            NetKind::Other => "Сеть",
        }
    }
}

/// One local IPv4 network of this device.
#[derive(Clone, Debug)]
pub struct LocalNet {
    pub name: String,
    pub kind: NetKind,
    pub ip: Ipv4Addr,
    pub broadcast: Option<Ipv4Addr>,
}

fn kind_of(name: &str, ip: Ipv4Addr) -> NetKind {
    let n = name.to_lowercase();
    if n.starts_with("rmnet") || n.starts_with("ccmni") || n.starts_with("wwan") || n.starts_with("pdp") || n.starts_with("seth") || n.starts_with("v4-") {
        NetKind::Mobile
    } else if n.starts_with("ap") || n.starts_with("swlan") || n.starts_with("softap") || (n.starts_with("wlan") && n != "wlan0") || ip.octets()[..3] == [192, 168, 43] {
        NetKind::Hotspot
    } else if n.starts_with("wlan") || n.starts_with("wl") {
        NetKind::Wifi
    } else if n.starts_with("eth") || n.starts_with("en") || n.starts_with("rndis") || n.starts_with("usb") {
        NetKind::Cable
    } else {
        NetKind::Other
    }
}

/// IPv4 networks of this device (no loopback, no link-local).
pub fn local_networks() -> Vec<LocalNet> {
    let mut out = Vec::new();
    if let Ok(list) = if_addrs::get_if_addrs() {
        for i in list {
            if let if_addrs::IfAddr::V4(v4) = &i.addr {
                if v4.ip.is_loopback() || v4.ip.is_link_local() || v4.ip.is_unspecified() {
                    continue;
                }
                let broadcast = v4.broadcast.or_else(|| {
                    let (ip, mask) = (u32::from(v4.ip), u32::from(v4.netmask));
                    (mask != 0 && mask != u32::MAX).then(|| Ipv4Addr::from(ip | !mask))
                });
                out.push(LocalNet { kind: kind_of(&i.name, v4.ip), name: i.name.clone(), ip: v4.ip, broadcast });
            }
        }
    }
    out
}

/// How friends can reach a server on `port`, one line per network, for the chat.
pub fn host_lines(port: u16) -> Vec<String> {
    let nets = local_networks();
    let mut out = Vec::new();
    for n in &nets {
        if n.kind == NetKind::Mobile {
            continue;
        }
        let code = invite_code(n.ip, port).map(|c| format!(" · код {c}")).unwrap_or_default();
        out.push(format!("{}: {}:{port}{code}", n.kind.label(), n.ip));
    }
    if out.is_empty() {
        out.push("Нет Wi-Fi: включите точку доступа на этом телефоне, друзья подключатся к ней".into());
    } else if nets.iter().any(|n| n.kind == NetKind::Mobile) && !nets.iter().any(|n| n.kind == NetKind::Hotspot) {
        out.push("Через мобильный интернет напрямую нельзя: включите точку доступа, друзья увидят сервер в «Рядом»".into());
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Invite codes: 8 letters for "IP:port" (32 bits of address + port offset from the default).

const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// "ABCD-EFGH" for an address. The port must be within 255 of the default one.
pub fn invite_code(ip: Ipv4Addr, port: u16) -> Option<String> {
    let off = port.checked_sub(crate::DEFAULT_PORT).filter(|o| *o < 256)?;
    let v: u64 = ((u32::from(ip) as u64) << 8) | off as u64;
    let s: String = (0..8).rev().map(|i| ALPHABET[((v >> (i * 5)) & 31) as usize] as char).collect();
    Some(format!("{}-{}", &s[..4], &s[4..]))
}

/// Address of an invite code, if `text` is one (case, spaces and the dash are ignored).
pub fn parse_invite(text: &str) -> Option<String> {
    let clean: Vec<u8> = text.trim().to_uppercase().bytes().filter(|b| *b != b'-' && *b != b' ').collect();
    if clean.len() != 8 {
        return None;
    }
    let mut v: u64 = 0;
    for b in clean {
        let d = ALPHABET.iter().position(|a| *a == b)? as u64;
        v = (v << 5) | d;
    }
    let ip = Ipv4Addr::from((v >> 8) as u32);
    let port = crate::DEFAULT_PORT + (v & 0xff) as u16;
    Some(format!("{ip}:{port}"))
}

/// What the player typed: an invite code or an address; returns "host:port".
pub fn resolve_input(text: &str) -> String {
    parse_invite(text).unwrap_or_else(|| crate::with_port(text.trim()))
}

// ---------------------------------------------------------------------------------------------
// Announcements

/// Sends "I am a server" once a second on every local network until `running` turns false.
pub fn start_beacon(name: String, port: u16, players: Arc<dyn Fn() -> (usize, usize) + Send + Sync>, running: Arc<AtomicBool>) {
    let _ = std::thread::Builder::new().name("server-beacon".into()).spawn(move || {
        let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else { return };
        let _ = sock.set_broadcast(true);
        while running.load(Ordering::SeqCst) {
            let (n, max) = players();
            let clean: String = name.chars().filter(|c| *c != '|' && !c.is_control()).take(40).collect();
            let msg = format!("{MAGIC}|{port}|{n}|{max}|{clean}");
            let mut targets = vec![Ipv4Addr::BROADCAST];
            for net in local_networks() {
                if let Some(b) = net.broadcast {
                    targets.push(b);
                }
            }
            for t in targets {
                let _ = sock.send_to(msg.as_bytes(), SocketAddr::from((t, BEACON_PORT)));
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
    });
}

/// A server heard on the local network.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    pub addr: String,
    pub players: usize,
    pub max: usize,
}

/// Listens for announcements in the background; dropping it stops the listener.
pub struct Discovery {
    seen: Arc<Mutex<HashMap<String, (Found, Instant)>>>,
    running: Arc<AtomicBool>,
}

impl Discovery {
    /// None when the port is taken (another listener on this device).
    pub fn start() -> Option<Discovery> {
        let sock = UdpSocket::bind(("0.0.0.0", BEACON_PORT)).ok()?;
        let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
        let seen: Arc<Mutex<HashMap<String, (Found, Instant)>>> = Arc::default();
        let running = Arc::new(AtomicBool::new(true));
        let (s2, r2) = (seen.clone(), running.clone());
        std::thread::Builder::new()
            .name("lan-discovery".into())
            .spawn(move || {
                let mut buf = [0u8; 256];
                while r2.load(Ordering::SeqCst) {
                    let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
                    let text = String::from_utf8_lossy(&buf[..n]);
                    let mut parts = text.splitn(5, '|');
                    if parts.next() != Some(MAGIC) {
                        continue;
                    }
                    let (Some(port), Some(pl), Some(max), Some(name)) = (parts.next().and_then(|p| p.parse::<u16>().ok()), parts.next().and_then(|p| p.parse().ok()), parts.next().and_then(|p| p.parse().ok()), parts.next()) else { continue };
                    let addr = format!("{}:{port}", from.ip());
                    let f = Found { name: name.to_string(), addr: addr.clone(), players: pl, max };
                    s2.lock().unwrap().insert(addr, (f, Instant::now()));
                }
            })
            .ok()?;
        Some(Discovery { seen, running })
    }

    /// Servers heard during the last few seconds, by name.
    pub fn servers(&self) -> Vec<Found> {
        let mut m = self.seen.lock().unwrap();
        m.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(5));
        let mut v: Vec<Found> = m.values().map(|(f, _)| f.clone()).collect();
        v.sort_by(|a, b| a.name.cmp(&b.name).then(a.addr.cmp(&b.addr)));
        v
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_codes_roundtrip() {
        let ip = Ipv4Addr::new(192, 168, 43, 1);
        let code = invite_code(ip, crate::DEFAULT_PORT).unwrap();
        assert_eq!(code.len(), 9);
        assert_eq!(parse_invite(&code).unwrap(), format!("192.168.43.1:{}", crate::DEFAULT_PORT));
        assert_eq!(parse_invite(&code.to_lowercase().replace('-', " ")).unwrap(), format!("192.168.43.1:{}", crate::DEFAULT_PORT));
        assert!(parse_invite("192.168.0.5").is_none());
        assert_eq!(resolve_input("192.168.0.5"), format!("192.168.0.5:{}", crate::DEFAULT_PORT));
    }

    #[test]
    fn announcements_are_heard() {
        let Some(d) = Discovery::start() else { return };
        let running = Arc::new(AtomicBool::new(true));
        start_beacon("Тест".into(), crate::DEFAULT_PORT, Arc::new(|| (1, 16)), running.clone());
        let t = Instant::now();
        while d.servers().is_empty() && t.elapsed() < Duration::from_secs(4) {
            std::thread::sleep(Duration::from_millis(100));
        }
        running.store(false, Ordering::SeqCst);
        let list = d.servers();
        if let Some(f) = list.first() {
            assert_eq!(f.name, "Тест");
            assert_eq!(f.players, 1);
        }
    }
}
