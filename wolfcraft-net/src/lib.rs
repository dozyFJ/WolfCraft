//! WolfCraft network protocol: one JSON message per line over TCP.
//!
//! The server (WolfServer) does not simulate the world. Every client generates the same planet
//! from the world settings it receives, and the server relays block edits, player states and chat
//! and keeps the list of edits so that late joiners see the same world.

pub mod lan;
pub mod server;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub const PROTOCOL: u32 = 2;
pub const DEFAULT_PORT: u16 = 25570;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub id: u32,
    pub nick: String,
    pub skin: u8,
    pub pos: [f64; 3],
    pub heading: [f64; 3],
    pub pitch: f32,
    pub phase: f32,
    pub amount: f32,
    pub sneak: bool,
    pub swim: bool,
    /// In the Hell dimension.
    pub hell: bool,
}

/// One block change: cell `(face, i, j, k)` and the new block id.
pub type Edit = ([i32; 4], u16);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum C2S {
    Hello { protocol: u32, nick: String, skin: u8 },
    State(PlayerState),
    Edit { cell: [i32; 4], block: u16, hell: bool },
    Chat(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum S2C {
    /// `world_toml` is the server's `world.toml` (seed, planet size, mode…).
    Welcome { id: u32, world_toml: String, motd: String },
    /// A batch of the edits made so far.
    Edits { hell: bool, list: Vec<Edit> },
    /// All earlier edits have been sent.
    Ready,
    Players(Vec<PlayerState>),
    Edit { cell: [i32; 4], block: u16, hell: bool },
    Chat { from: String, text: String },
    Kick(String),
}

pub fn write_msg<T: Serialize>(w: &mut impl Write, m: &T) -> std::io::Result<()> {
    let mut line = serde_json::to_string(m).map_err(std::io::Error::other)?;
    line.push('\n');
    w.write_all(line.as_bytes())
}

/// Reads one message; `Ok(None)` when the peer closed the connection.
pub fn read_msg<T: DeserializeOwned>(r: &mut impl BufRead) -> std::io::Result<Option<T>> {
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line.trim().is_empty() {
            continue;
        }
        return serde_json::from_str(line.trim()).map(Some).map_err(std::io::Error::other);
    }
}

/// `host` or `host:port` -> `host:port` with the default port.
pub fn with_port(addr: &str) -> String {
    let a = addr.trim();
    if a.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) { a.to_string() } else { format!("{a}:{DEFAULT_PORT}") }
}

/// Client side of a connection: a reader thread turns the stream into a queue.
pub struct Client {
    pub id: u32,
    pub world_toml: String,
    pub motd: String,
    stream: TcpStream,
    rx: Receiver<S2C>,
    alive: Arc<AtomicBool>,
}

impl Client {
    pub fn connect(addr: &str, nick: &str, skin: u8) -> Result<Client, String> {
        let full = with_port(addr);
        let sock = full.to_socket_addrs().map_err(|e| format!("адрес «{full}»: {e}"))?.next().ok_or_else(|| format!("адрес «{full}» не найден"))?;
        let stream = TcpStream::connect_timeout(&sock, Duration::from_secs(5)).map_err(|e| format!("не удалось подключиться к {full}: {e}"))?;
        let _ = stream.set_nodelay(true);
        let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(8)));
        let mut w = stream.try_clone().map_err(|e| e.to_string())?;
        write_msg(&mut w, &C2S::Hello { protocol: PROTOCOL, nick: nick.to_string(), skin }).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let (id, world_toml, motd) = match read_msg::<S2C>(&mut reader).map_err(|e| format!("сервер не ответил: {e}"))? {
            Some(S2C::Welcome { id, world_toml, motd }) => (id, world_toml, motd),
            Some(S2C::Kick(reason)) => return Err(format!("сервер отказал: {reason}")),
            _ => return Err("сервер ответил не по протоколу WolfCraft".into()),
        };
        let _ = stream.set_read_timeout(None);
        let (tx, rx) = channel();
        let alive = Arc::new(AtomicBool::new(true));
        let alive2 = alive.clone();
        std::thread::Builder::new()
            .name("net-reader".into())
            .spawn(move || {
                while let Ok(Some(m)) = read_msg::<S2C>(&mut reader) {
                    if tx.send(m).is_err() {
                        break;
                    }
                }
                alive2.store(false, Ordering::SeqCst);
            })
            .map_err(|e| e.to_string())?;
        Ok(Client { id, world_toml, motd, stream, rx, alive })
    }

    pub fn send(&mut self, m: &C2S) {
        if self.alive.load(Ordering::SeqCst) && write_msg(&mut self.stream, m).is_err() {
            self.alive.store(false, Ordering::SeqCst);
        }
    }

    /// Messages received since the last call.
    pub fn poll(&self) -> Vec<S2C> {
        self.rx.try_iter().collect()
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_edits_do_not_truncate_wide_ids() {
        let edit=C2S::Edit {cell:[0,1,2,3],block:2319,hell:false};
        let mut data=Vec::new();write_msg(&mut data,&edit).unwrap();
        let mut reader=std::io::BufReader::new(data.as_slice());
        let actual:C2S=read_msg(&mut reader).unwrap().unwrap();assert_eq!(actual,edit);
    }

    #[test]
    fn messages_round_trip_as_single_lines() {
        let m = S2C::Edits { hell: true, list: vec![([1, -5, 7, 1030], 4)] };
        let mut buf = Vec::new();
        write_msg(&mut buf, &m).unwrap();
        write_msg(&mut buf, &S2C::Chat { from: "Волк".into(), text: "привет\nмир".into() }).unwrap();
        assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 2, "one line per message");
        let mut r = std::io::BufReader::new(&buf[..]);
        assert_eq!(read_msg::<S2C>(&mut r).unwrap(), Some(m));
        assert!(matches!(read_msg::<S2C>(&mut r).unwrap(), Some(S2C::Chat { .. })));
        assert_eq!(read_msg::<S2C>(&mut r).unwrap(), None);
    }

    #[test]
    fn default_port_is_added_only_when_missing() {
        assert_eq!(with_port("192.168.0.10"), format!("192.168.0.10:{DEFAULT_PORT}"));
        assert_eq!(with_port(" 192.168.0.10:1234 "), "192.168.0.10:1234");
    }
}

