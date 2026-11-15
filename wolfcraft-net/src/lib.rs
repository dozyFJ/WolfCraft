//! WolfCraft network protocol: one JSON message per line over TCP.
//!
//! Clients generate terrain and render it locally. The server relays block edits, players
//! and chat, and owns the fixed-step WolfTech simulation and durable machine exchanges.

pub mod lan;
pub mod server;
pub mod technology;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub const PROTOCOL: u32 = 4;
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
    /// `voice`: this client speaks the voice chat (older ones leave it out).
    Hello {
        protocol: u32,
        nick: String,
        skin: u8,
        #[serde(default)]
        voice: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_token: Option<String>,
    },
    TechOpen {cell:[i32;4],hell:bool},
    TechChange(technology::Change),
    TechRecover(technology::Change),
    TechConfirm(String),
    State(PlayerState),
    Edit { cell: [i32; 4], block: u16, hell: bool },
    Chat(String),
    /// Voice chat: one frame of the speaker's voice (see `voice` in the game), base64.
    Voice(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum S2C {
    /// `world_toml` is the server's `world.toml` (seed, planet size, mode…).
    Welcome {
        id: u32,
        world_toml: String,
        motd: String,
        /// The server relays voice chat (older servers leave it out).
        #[serde(default)]
        voice: bool,
    },
    /// A batch of the edits made so far.
    Edits { hell: bool, list: Vec<Edit> },
    /// All earlier edits have been sent.
    Ready,
    TechSnapshot {hell:bool,reset:bool,time:f64,weather:(wolfcraft_engine::weather::Sky,f32,f32),machines:Vec<technology::Machine>},
    TechResult(technology::Reply),
    TechBroken {cell:[i32;4],hell:bool,block:u16},
    Players(Vec<PlayerState>),
    Edit { cell: [i32; 4], block: u16, hell: bool },
    Chat { from: String, text: String },
    Kick(String),
    /// A voice frame from player `id` (sent only to clients that said `voice` in Hello).
    Voice { id: u32, data: String },
}

/// Messages longer than this go compressed (like the Krypton mod): the world's edit list on
/// join, long chats, voice frames. Short ones (player states) stay plain — cheaper to send.
const COMPRESS_OVER: usize = 512;

pub fn write_msg<T: Serialize>(w: &mut impl Write, m: &T) -> std::io::Result<()> {
    let json = serde_json::to_string(m).map_err(std::io::Error::other)?;
    let mut line = if json.len() > COMPRESS_OVER { compress_line(&json).unwrap_or(json) } else { json };
    line.push('\n');
    w.write_all(line.as_bytes())
}

/// `~` + base64(deflate(json)); a line starting with `~` is never JSON.
fn compress_line(json: &str) -> Option<String> {
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
    enc.write_all(json.as_bytes()).ok()?;
    let packed = enc.finish().ok()?;
    let out = format!("~{}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(packed));
    (out.len() < json.len()).then_some(out)
}

fn decompress_line(line: &str) -> std::io::Result<String> {
    use base64::Engine;
    use std::io::Read;
    let packed = base64::engine::general_purpose::STANDARD_NO_PAD.decode(line.trim_start_matches('~')).map_err(std::io::Error::other)?;
    let mut out = String::new();
    flate2::read::DeflateDecoder::new(&packed[..]).take(64 << 20).read_to_string(&mut out)?;
    Ok(out)
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
        let text = line.trim();
        if text.starts_with('~') {
            let json = decompress_line(text)?;
            return serde_json::from_str(&json).map(Some).map_err(std::io::Error::other);
        }
        return serde_json::from_str(text).map(Some).map_err(std::io::Error::other);
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
    /// The server relays voice chat.
    pub voice: bool,
    stream: TcpStream,
    rx: Receiver<S2C>,
    alive: Arc<AtomicBool>,
}

impl Client {
    pub fn connect(addr: &str, nick: &str, skin: u8) -> Result<Client, String> {
        Self::connect_with_access(addr,nick,skin,None)
    }

    pub fn connect_with_access(addr: &str, nick: &str, skin: u8, access_token:Option<String>) -> Result<Client, String> {
        let full = with_port(addr);
        let sock = full.to_socket_addrs().map_err(|e| format!("адрес «{full}»: {e}"))?.next().ok_or_else(|| format!("адрес «{full}» не найден"))?;
        let stream = TcpStream::connect_timeout(&sock, Duration::from_secs(5)).map_err(|e| format!("не удалось подключиться к {full}: {e}"))?;
        let _ = stream.set_nodelay(true);
        let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(8)));
        let mut w = stream.try_clone().map_err(|e| e.to_string())?;
        write_msg(&mut w, &C2S::Hello { protocol: PROTOCOL, nick: nick.to_string(), skin, voice: true, access_token }).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let (id, world_toml, motd, voice) = match read_msg::<S2C>(&mut reader).map_err(|e| format!("сервер не ответил: {e}"))? {
            Some(S2C::Welcome { id, world_toml, motd, voice }) => (id, world_toml, motd, voice),
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
        Ok(Client { id, world_toml, motd, voice, stream, rx, alive })
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
    fn long_messages_go_compressed_and_come_back_whole() {
        let long = C2S::Chat("Привет, мир! ".repeat(200));
        let mut data = Vec::new();
        write_msg(&mut data, &long).unwrap();
        assert_eq!(data[0], b'~', "compressed on the wire");
        assert!(data.len() < serde_json::to_string(&long).unwrap().len() / 3);
        let short = C2S::Chat("hi".into());
        write_msg(&mut data, &short).unwrap();
        let mut r = std::io::BufReader::new(data.as_slice());
        assert_eq!(read_msg::<C2S>(&mut r).unwrap().unwrap(), long);
        assert_eq!(read_msg::<C2S>(&mut r).unwrap().unwrap(), short);
    }

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



