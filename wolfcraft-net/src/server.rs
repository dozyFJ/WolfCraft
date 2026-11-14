//! The relay server itself, usable both by the WolfServer program and from inside the game
//! ("host on this device", which is how a phone hosts a world).

use crate::{C2S, Edit, PROTOCOL, PlayerState, S2C, read_msg, write_msg};
use std::collections::{HashMap, VecDeque};
use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_PLAYERS: usize = 16;

pub struct Player {
    pub state: PlayerState,
    out: TcpStream,
    /// Hears voice chat (said so in Hello).
    voice: bool,
}

pub struct State {
    pub name: String,
    pub world_toml: String,
    pub dir: PathBuf,
    /// Latest block per cell: `[overworld, hell]`.
    pub edits: [HashMap<[i32; 4], u16>; 2],
    pub players: HashMap<u32, Player>,
    next_id: u32,
    pub log: VecDeque<String>,
    pub started: Instant,
    pub port: u16,
    /// Print log lines to stdout (the standalone server does, the game does not).
    pub echo: bool,
}

pub type Shared = Arc<Mutex<State>>;

impl State {
    pub fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) % 86400;
        let stamped = format!("[{:02}:{:02}:{:02}] {line}", secs / 3600, secs % 3600 / 60, secs % 60);
        if self.echo {
            println!("{stamped}");
        }
        self.log.push_back(stamped);
        while self.log.len() > 300 {
            self.log.pop_front();
        }
    }

    /// Sends to everyone (except `skip`); players whose socket is dead are dropped.
    /// Sends a voice frame to every other player that hears voice chat.
    pub fn voice_relay(&mut self, from: u32, m: &S2C) {
        let mut dead = Vec::new();
        for (id, p) in self.players.iter_mut() {
            if *id != from && p.voice && write_msg(&mut p.out, m).is_err() {
                dead.push(*id);
            }
        }
        for id in dead {
            self.drop_player(id, "связь потеряна");
        }
    }

    pub fn broadcast(&mut self, m: &S2C, skip: Option<u32>) {
        let mut dead = Vec::new();
        for (id, p) in self.players.iter_mut() {
            if Some(*id) != skip && write_msg(&mut p.out, m).is_err() {
                dead.push(*id);
            }
        }
        for id in dead {
            self.drop_player(id, "связь потеряна");
        }
    }

    pub fn drop_player(&mut self, id: u32, why: &str) {
        if let Some(p) = self.players.remove(&id) {
            let _ = p.out.shutdown(std::net::Shutdown::Both);
            self.log(format!("{} вышел ({why})", p.state.nick));
            let text = format!("{} вышел из игры", p.state.nick);
            self.broadcast(&S2C::Chat { from: String::new(), text }, None);
        }
    }

    /// Tells one player why, then removes them.
    pub fn kick(&mut self, id: u32, reason: &str) {
        if let Some(p) = self.players.get_mut(&id) {
            let _ = write_msg(&mut p.out, &S2C::Kick(reason.to_string()));
        }
        self.drop_player(id, "выгнан");
    }

    fn append_edit(&self, hell: bool, cell: [i32; 4], block: u16) {
        let line = format!("{} {} {} {} {} {}\n", hell as u8, cell[0], cell[1], cell[2], cell[3], block);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join("edits.log")) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

/// Parses `edits.log` (later lines win).
pub fn load_edits(dir: &Path) -> [HashMap<[i32; 4], u16>; 2] {
    let mut out = [HashMap::new(), HashMap::new()];
    if let Ok(text) = std::fs::read_to_string(dir.join("edits.log")) {
        for line in text.lines() {
            let n: Vec<i64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if n.len() == 6 {
                out[(n[0] != 0) as usize].insert([n[1] as i32, n[2] as i32, n[3] as i32, n[4] as i32], n[5] as u16);
            }
        }
    }
    out
}

pub fn clean_nick(nick: &str) -> String {
    let n: String = nick.chars().filter(|c| !c.is_control()).take(16).collect();
    if n.trim().is_empty() { "Игрок".to_string() } else { n.trim().to_string() }
}

fn handle_client(shared: Shared, stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    let Ok(mut out) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    let (nick, skin, voice) = match read_msg::<C2S>(&mut reader) {
        Ok(Some(C2S::Hello { protocol, nick, skin, voice })) if protocol == PROTOCOL => (clean_nick(&nick), skin, voice),
        Ok(Some(C2S::Hello { .. })) => {
            let _ = write_msg(&mut out, &S2C::Kick("другая версия игры".into()));
            return;
        }
        _ => return,
    };
    // Register and send the world, then every earlier edit.
    let id = {
        let mut s = shared.lock().unwrap();
        if s.players.len() >= MAX_PLAYERS {
            let _ = write_msg(&mut out, &S2C::Kick("сервер заполнен".into()));
            return;
        }
        let id = s.next_id;
        s.next_id += 1;
        let welcome = S2C::Welcome { id, world_toml: s.world_toml.clone(), motd: format!("Добро пожаловать в «{}»", s.name), voice: true };
        if write_msg(&mut out, &welcome).is_err() {
            return;
        }
        for hell in [false, true] {
            let list: Vec<Edit> = s.edits[hell as usize].iter().map(|(c, b)| (*c, *b)).collect();
            for part in list.chunks(400) {
                if write_msg(&mut out, &S2C::Edits { hell, list: part.to_vec() }).is_err() {
                    return;
                }
            }
        }
        if write_msg(&mut out, &S2C::Ready).is_err() {
            return;
        }
        let Ok(out2) = out.try_clone() else { return };
        let state = PlayerState { id, nick: nick.clone(), skin, pos: [0.0; 3], heading: [1.0, 0.0, 0.0], pitch: 0.0, phase: 0.0, amount: 0.0, sneak: false, swim: false, hell: false };
        s.players.insert(id, Player { state, out: out2, voice });
        s.log(format!("{nick} вошёл (id {id})"));
        s.broadcast(&S2C::Chat { from: String::new(), text: format!("{nick} вошёл в игру") }, None);
        id
    };
    loop {
        match read_msg::<C2S>(&mut reader) {
            Ok(Some(C2S::State(mut st))) => {
                let mut s = shared.lock().unwrap();
                let Some(p) = s.players.get_mut(&id) else { break };
                // Identity comes from the handshake, never from the state packet.
                st.id = id;
                st.nick = p.state.nick.clone();
                st.skin = p.state.skin;
                p.state = st;
            }
            Ok(Some(C2S::Edit { cell, block, hell })) => {
                let mut s = shared.lock().unwrap();
                if !s.players.contains_key(&id) {
                    break;
                }
                s.edits[hell as usize].insert(cell, block);
                s.append_edit(hell, cell, block);
                s.broadcast(&S2C::Edit { cell, block, hell }, Some(id));
            }
            Ok(Some(C2S::Chat(text))) => {
                let text: String = text.chars().filter(|c| !c.is_control()).take(200).collect();
                if text.trim().is_empty() {
                    continue;
                }
                let mut s = shared.lock().unwrap();
                if !s.players.contains_key(&id) {
                    break;
                }
                s.log(format!("<{nick}> {text}"));
                s.broadcast(&S2C::Chat { from: nick.clone(), text }, None);
            }
            Ok(Some(C2S::Voice(data))) => {
                // Relay to everyone else who hears voice; a frame is small (40 ms of speech).
                if data.len() > 8192 {
                    continue;
                }
                let mut s = shared.lock().unwrap();
                if !s.players.contains_key(&id) {
                    break;
                }
                s.voice_relay(id, &S2C::Voice { id, data });
            }
            Ok(Some(C2S::Hello { .. })) => {}
            _ => break,
        }
    }
    shared.lock().unwrap().drop_player(id, "отключился");
}

/// Addresses other devices can use: the local IPv4 address of the active network.
pub fn lan_addresses(port: u16) -> Vec<String> {
    let mut v = Vec::new();
    // The socket never sends anything: connecting only picks the outgoing interface.
    if let Ok(s) = UdpSocket::bind("0.0.0.0:0") {
        if s.connect("192.168.255.255:9").is_ok() || s.connect("10.255.255.255:9").is_ok() {
            if let Ok(a) = s.local_addr() {
                if !a.ip().is_unspecified() && !a.ip().is_loopback() {
                    v.push(format!("{}:{port}", a.ip()));
                }
            }
        }
    }
    if v.is_empty() {
        v.push(format!("127.0.0.1:{port}"));
    }
    v
}

/// A running server. Dropping the handle stops it.
pub struct ServerHandle {
    pub shared: Shared,
    pub running: Arc<AtomicBool>,
    pub port: u16,
}

impl ServerHandle {
    pub fn stop(&self) {
        if self.running.swap(false, Ordering::SeqCst) {
            let mut s = self.shared.lock().unwrap();
            s.log("Сервер остановлен");
            s.broadcast(&S2C::Kick("сервер остановлен".into()), None);
            // Close every connection so clients notice at once.
            for (_, p) in s.players.drain() {
                let _ = p.out.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts the server on `port`: accept thread + a 20 Hz broadcast of player states.
/// `world_toml` is sent to every client; `dir` holds `edits.log`.
pub fn start(name: &str, world_toml: String, dir: &Path, port: u16, echo: bool) -> Result<ServerHandle, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let listener = TcpListener::bind(("0.0.0.0", port)).map_err(|e| format!("порт {port} занят или недоступен: {e}"))?;
    let _ = listener.set_nonblocking(true);
    let edits = load_edits(dir);
    let shared: Shared = Arc::new(Mutex::new(State {
        name: name.to_string(),
        world_toml,
        dir: dir.to_path_buf(),
        edits,
        players: HashMap::new(),
        next_id: 1,
        log: VecDeque::new(),
        started: Instant::now(),
        port,
        echo,
    }));
    let running = Arc::new(AtomicBool::new(true));
    {
        let (shared, running) = (shared.clone(), running.clone());
        std::thread::Builder::new()
            .name("server-tick".into())
            .spawn(move || {
                while running.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    let mut s = shared.lock().unwrap();
                    if s.players.len() > 1 {
                        let list: Vec<PlayerState> = s.players.values().map(|p| p.state.clone()).collect();
                        s.broadcast(&S2C::Players(list), None);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }
    {
        let (shared, running) = (shared.clone(), running.clone());
        std::thread::Builder::new()
            .name("server-accept".into())
            .spawn(move || {
                while running.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let shared = shared.clone();
                            let _ = std::thread::Builder::new().name("server-client".into()).spawn(move || handle_client(shared, stream));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(40)),
                        Err(_) => std::thread::sleep(Duration::from_millis(200)),
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }
    // Tell the local networks (Wi-Fi, hotspot) that a game is open here.
    {
        let s2 = shared.clone();
        crate::lan::start_beacon(name.to_string(), port, Arc::new(move || (s2.lock().map(|s| s.players.len()).unwrap_or(0), MAX_PLAYERS)), running.clone());
    }
    Ok(ServerHandle { shared, running, port })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;

    #[test]
    fn edit_log_keeps_the_latest_block_per_cell() {
        let dir = std::env::temp_dir().join("wolfnet_edits_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("edits.log"), "0 1 2 3 1030 5\n0 1 2 3 1030 0\n1 0 0 0 1000 86\nмусор\n").unwrap();
        let e = load_edits(&dir);
        assert_eq!(e[0].get(&[1, 2, 3, 1030]), Some(&0));
        assert_eq!(e[1].get(&[0, 0, 0, 1000]), Some(&86));
        assert_eq!(e[0].len() + e[1].len(), 2);
    }

    #[test]
    fn nicknames_are_trimmed_and_limited() {
        assert_eq!(clean_nick("  Волк\n "), "Волк");
        assert_eq!(clean_nick(""), "Игрок");
        assert_eq!(clean_nick(&"a".repeat(40)).chars().count(), 16);
    }

    fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
        let t0 = Instant::now();
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn two_clients_share_edits_chat_and_positions_and_late_joiners_get_the_edits() {
        let dir = std::env::temp_dir().join("wolfnet_server_test");
        let _ = std::fs::remove_dir_all(&dir);
        let port = 25791;
        let server = start("Тест", "seed = 1\n".into(), &dir, port, false).unwrap();
        let addr = format!("127.0.0.1:{port}");
        let mut a = Client::connect(&addr, "Аня", 1).unwrap();
        let mut b = Client::connect(&addr, "Боря", 2).unwrap();
        assert_eq!(a.world_toml, "seed = 1\n");
        assert_ne!(a.id, b.id);

        // A's edit reaches B but is not echoed back to A.
        a.send(&C2S::Edit { cell: [2, 10, 11, 1030], block: 7, hell: false });
        wait_for(|| b.poll().into_iter().find(|m| matches!(m, S2C::Edit { cell: [2, 10, 11, 1030], block: 7, hell: false })));
        assert!(!a.poll().iter().any(|m| matches!(m, S2C::Edit { .. })));

        // Chat goes to everyone, with the name from the handshake.
        b.send(&C2S::Chat("привет".into()));
        wait_for(|| a.poll().into_iter().find(|m| matches!(m, S2C::Chat { from, text } if from == "Боря" && text == "привет")));

        // Positions: A sees B where B says it is, under B's real id and nick.
        let st = PlayerState { id: 999, nick: "подделка".into(), skin: 0, pos: [1.0, 2.0, 3.0], heading: [0.0, 0.0, 1.0], pitch: 0.1, phase: 0.0, amount: 0.0, sneak: false, swim: false, hell: false };
        let b_id = b.id;
        wait_for(|| {
            b.send(&C2S::State(st.clone()));
            a.poll().into_iter().find_map(|m| match m {
                S2C::Players(list) => list.into_iter().find(|p| p.id == b_id && p.pos == [1.0, 2.0, 3.0] && p.nick == "Боря"),
                _ => None,
            })
        });

        // A late joiner receives the earlier edit before `Ready`.
        let c = Client::connect(&addr, "Вика", 0).unwrap();
        let mut got = Vec::new();
        wait_for(|| {
            got.extend(c.poll());
            got.iter().any(|m| *m == S2C::Ready).then_some(())
        });
        assert!(got.iter().any(|m| matches!(m, S2C::Edits { hell: false, list } if list.contains(&([2, 10, 11, 1030], 7)))));

        // Stopping kicks everyone and the edit survives on disk.
        server.stop();
        wait_for(|| (!a.is_alive()).then_some(()));
        assert_eq!(load_edits(&dir)[0].get(&[2, 10, 11, 1030]), Some(&7));
    }
}

