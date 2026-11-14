//! WolfCraft Сервер — the dedicated server of WolfCraft.
//!
//! It keeps the world settings and the list of block edits, relays player states and chat
//! between clients, and serves a local control panel in the browser.
//!
//! Usage: `WolfServer [--name <мир>] [--seed <число>] [--port <порт>] [--creative]
//!                    [--background] [--no-browser] [--data <папка>] [--token <ключ>]`
//!
//! The same server runs on a PC and, inside the WolfServer Pocket app, on an Android phone.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use wolfcraft_engine::config::{GameMode, WorldConfig};
mod worlds;
mod profile;
mod runtime;
use wolfcraft_net::server::{ServerHandle, lan_addresses};
use wolfcraft_net::{DEFAULT_PORT, S2C};

const PANEL_PORT: u16 = 25580;


fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex.ok_or(()) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| url_decode(v)))
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) {
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

/// The control panel: local only, and every API call needs the token embedded in the page, so
/// another web page open in the browser cannot stop the server or kick players.
fn handle_panel(server: &ServerHandle, cfg: &WorldConfig, token: &str, mut stream: TcpStream) {
    let shared = &server.shared;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    // Drain the headers.
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
        line.clear();
    }
    let mut parts = first.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == "/" {
        let html = include_str!("panel.html").replace("__TOKEN__", token);
        return respond(&mut stream, "200 OK", "text/html; charset=utf-8", html.as_bytes());
    }
    if !path.starts_with("/api/") {
        return respond(&mut stream, "404 Not Found", "text/plain", b"not found");
    }
    if query_param(query, "t").as_deref() != Some(token) {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"forbidden");
    }
    match (method, path) {
        ("GET", "/api/status") => {
            let s = shared.lock().unwrap();
            let players: Vec<serde_json::Value> = s.players.values().map(|p| serde_json::json!({ "id": p.state.id, "nick": p.state.nick, "hell": p.state.hell, "role": p.role.name() })).collect();
            let body = serde_json::json!({
                "version": wolfcraft_common::VERSION,
                "name": cfg.name,
                "seed": (cfg.seed as i64).to_string(),
                "world_id": cfg.id,
                "kind": if cfg.studio_project { "place" } else { "world" },
                "mode": if cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" },
                "edits": s.edits[0].len() + s.edits[1].len(),
                "uptime": s.started.elapsed().as_secs(),
                "addresses": lan_addresses(s.port),
                "codes": wolfcraft_net::lan::codes_for(&lan_addresses(s.port)),
                "players": players,
                "log": s.log.iter().collect::<Vec<_>>(),
            });
            respond(&mut stream, "200 OK", "application/json; charset=utf-8", body.to_string().as_bytes());
        }
        ("POST", "/api/say") => {
            if let Some(text) = query_param(query, "text").filter(|t| !t.trim().is_empty()) {
                let text: String = text.chars().filter(|c| !c.is_control()).take(200).collect();
                let mut s = shared.lock().unwrap();
                s.log(format!("[Сервер] {text}"));
                s.broadcast(&S2C::Chat { from: "Сервер".into(), text }, None);
            }
            respond(&mut stream, "200 OK", "text/plain", b"ok");
        }
        ("POST", "/api/kick") => {
            if let Some(id) = query_param(query, "id").and_then(|v| v.parse::<u32>().ok()) {
                shared.lock().unwrap().kick(id, "вас выгнал администратор");
            }
            respond(&mut stream, "200 OK", "text/plain", b"ok");
        }
        ("POST", "/api/stop") => {
            server.stop();
            respond(&mut stream, "200 OK", "text/plain", b"ok");
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
    let _ = stream.flush();
    let mut sink = [0u8; 1];
    let _ = stream.read(&mut sink);
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned()
}

fn data_dir() -> PathBuf {
    let base = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| PathBuf::from("."));
    base.join("wolfserver_data")
}

#[cfg(windows)]
fn respawn_hidden(args: &[String]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let exe = std::env::current_exe()?;
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--background").collect();
    std::process::Command::new(exe).args(rest).creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS).spawn().map(|_| ())
}

#[cfg(not(windows))]
fn respawn_hidden(_args: &[String]) -> std::io::Result<()> {
    Err(std::io::Error::other("фоновый режим есть только в Windows"))
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("WolfCraft Сервер {}\n  --name <мир>      название мира (по умолчанию «Сервер»)\n  --seed <число>    seed нового мира\n  --port <порт>     порт игры (по умолчанию {DEFAULT_PORT})\n  --creative        творческий режим для нового мира\n  --survival        выживание для нового мира\n  --place           новый плейс: пустая основа для студии\n  --planet          обычный мир-планета (по умолчанию)\n  --world <id>      папка существующего мира\n  --data <папка>    папка данных сервера\n  --background      запустить в фоне без окна\n  --no-browser      не открывать панель в браузере", wolfcraft_common::VERSION);
        return;
    }
    if args.iter().any(|a| a == "--background") {
        match respawn_hidden(&args) {
            Ok(()) => println!("WolfCraft Сервер запущен в фоне. Панель: http://127.0.0.1:{PANEL_PORT}/"),
            Err(e) => eprintln!("Не удалось запустить в фоне: {e}"),
        }
        return;
    }
    let name = arg_value(&args, "--name").unwrap_or_else(|| "Сервер".to_string());
    let port = arg_value(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
    // `--data`: where worlds live (the phone app passes its own files folder).
    let data = arg_value(&args, "--data").map(PathBuf::from).unwrap_or_else(data_dir);
    let (dir,cfg)=match worlds::load_or_create(&data,&name,&args) {
        Ok(world)=>world,
        Err(e)=>{eprintln!("Сервер не запущен: {e}");return;}
    };
    let world_toml = match toml::to_string_pretty(&cfg) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("world.toml: {e}");
            return;
        }
    };
    let token=match arg_value(&args,"--token"){Some(t) if t.len()>=8=>t,_=>match profile::secret(){Ok(t)=>t,Err(e)=>{eprintln!("Ключ: {e}");return;}}};
    let profile=match profile::load(&data,&dir,&cfg){Ok(p)=>p,Err(e)=>{eprintln!("Конфигурация сервера: {e}");return;}};
    let hooks=match runtime::Runtime::load(&profile,&cfg){Ok(h)=>std::sync::Arc::new(h),Err(e)=>{eprintln!("Расширения сервера: {e}");return;}};
    let options=wolfcraft_net::server::ServerOptions{place:profile.place,max_players:profile.config.max_players,owner_keys:vec![profile.config.owner_key.clone(),token.clone()],editor_keys:profile.config.editor_keys.clone(),hooks:Some(hooks)};
    let server = match wolfcraft_net::server::start_with_options(&cfg.name, world_toml, &dir, port, true,options) {
        Ok(s) => std::sync::Arc::new(s),
        Err(e) => {
            eprintln!("Сервер не запущен: {e}. Возможно, WolfCraft Сервер уже работает.");
            return;
        }
    };
    let port=server.port;
    {
        let mut s = server.shared.lock().unwrap();
        let e = s.edits[0].len() + s.edits[1].len();
        s.log(format!("WolfCraft Сервер {} — мир «{}», seed {}, изменённых блоков: {e}", wolfcraft_common::VERSION, cfg.name, cfg.seed as i64));
        for a in lan_addresses(port) {
            s.log(format!("Адрес для игроков: {a}"));
        }
        for c in wolfcraft_net::lan::codes_for(&lan_addresses(port)) {
            s.log(format!("Код для входа: {c}  (WolfCraft → Игра по сети → По коду)"));
        }
    }

    // Control panel (this computer only).
    // `--token`: the WolfServer Pocket app passes its own, so its dashboard can use the API.

    match TcpListener::bind(("127.0.0.1", PANEL_PORT)) {
        Ok(panel) => {
            let (server2, cfg2, token2) = (server.clone(), cfg.clone(), token.clone());
            std::thread::spawn(move || {
                for stream in panel.incoming().flatten() {
                    handle_panel(&server2, &cfg2, &token2, stream);
                }
            });
            server.shared.lock().unwrap().log(format!("Панель управления: http://127.0.0.1:{PANEL_PORT}/"));
            if !args.iter().any(|a| a == "--no-browser") {
                open_browser(&format!("http://127.0.0.1:{PANEL_PORT}/"));
            }
        }
        Err(e) => server.shared.lock().unwrap().log(format!("Панель недоступна (порт {PANEL_PORT}): {e}")),
    }

    while server.is_running() {
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(300));
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_decoding_handles_utf8_and_plus() {
        assert_eq!(url_decode("%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82+%D0%BC%D0%B8%D1%80"), "привет мир");
        assert_eq!(url_decode("100%"), "100%");
        assert_eq!(query_param("id=7&t=abc", "t").as_deref(), Some("abc"));
        assert_eq!(query_param("id=7", "t"), None);
    }
}

/// The engine links winit, whose Android backend expects this entry point even though the
/// server is a plain command-line program there (WolfServer Pocket starts it as a process).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(_app: winit::platform::android::activity::AndroidApp) {}

