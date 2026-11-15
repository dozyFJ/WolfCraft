//! Durable outbox for one machine exchange at a time. A reconnect resolves the same
//! operation against the server's receipt instead of duplicating or guessing item transfers.
use crate::session::GameSession;
use serde::{Deserialize, Serialize};
use wolfcraft_engine::{items::ItemStack, storage};
use wolfcraft_net::technology::{Change, Machine, Reply};
#[derive(Serialize, Deserialize)]
struct Identity {
    client: String,
    serial: u64,
}
impl GameSession {
    pub fn load_pending_tech(&mut self) {
        let p = self.dir.join("tech-pending.json");
        if p.exists() {
            self.pending_tech = storage::read_with_backup(&p, |b| {
                serde_json::from_slice::<Change>(b)
                    .ok()
                    .filter(|c| c.inventory_before.len() == 36 && c.inventory_after.len() == 36)
            });
            if self.pending_tech.is_none() {
                self.tech_pending_error=Some("Не удалось прочитать незавершённую операцию машины. Сохранение оставлено для восстановления.".into());
            }
        }
    }
    pub fn begin_tech_change(
        &mut self,
        pos: wolfcraft_engine::cubesphere::Cell,
        before: Vec<Option<ItemStack>>,
        after: Vec<Option<ItemStack>>,
        inventory_before: Vec<Option<ItemStack>>,
        inventory_after: Vec<Option<ItemStack>>,
    ) -> Result<Change, String> {
        if self.pending_tech.is_some() || self.tech_pending_error.is_some() {
            return Err("Дождитесь синхронизации машины".into());
        }
        let p = self.dir.join("tech-client.json");
        let mut identity = if p.exists() {
            storage::read_with_backup(&p, |b| serde_json::from_slice::<Identity>(b).ok())
                .ok_or("Повреждён номер операций машины")?
        } else {
            let mut bytes = [0u8; 16];
            getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
            Identity {
                client: bytes.iter().map(|b| format!("{b:02x}")).collect(),
                serial: 0,
            }
        };
        identity.serial = identity
            .serial
            .checked_add(1)
            .ok_or("Переполнен номер операции")?;
        storage::write_safe(
            &p,
            &serde_json::to_vec(&identity).map_err(|e| e.to_string())?,
        )?;
        let change = Change {
            world: self.network_world.clone(),
            id: format!("{}:{}", identity.client, identity.serial),
            hell: self.hell,
            pos: wolfcraft_net::technology::pos(pos),
            revision: *self
                .tech_revisions
                .get(&pos)
                .ok_or("Дождитесь данных сервера")?,
            before,
            after,
            inventory_before,
            inventory_after,
        };
        let p = self.dir.join("tech-pending.json");
        let _ = std::fs::remove_file(self.dir.join("tech-pending.json.bak"));
        storage::write_safe(&p, &serde_json::to_vec(&change).map_err(|e| e.to_string())?)?;
        self.pending_tech = Some(change.clone());
        self.tech_pending_sent = false;
        // If the process ends here, the outbox recovers the correct before/after inventory.
        storage::write_safe(&self.dir.join("player.dat"), &self.player.to_bytes())?;
        Ok(change)
    }
    pub fn apply_tech_reply(&mut self, reply: Reply) -> Result<Option<String>, String> {
        let Some(change) = self.pending_tech.as_ref().filter(|c| c.id == reply.id) else {
            return Ok(None);
        };
        if !reply.settled {
            self.tech_pending_sent = false;
            self.tech_retry = 2.0;
            return Err(reply.error);
        }
        let expected = if reply.accepted {
            &change.inventory_after
        } else {
            &change.inventory_before
        };
        if reply.inventory != *expected {
            return Err("Сервер вернул несогласованный инвентарь".into());
        }
        self.player.inventory.slots = reply
            .inventory
            .try_into()
            .map_err(|_| "Неверный размер инвентаря")?;
        storage::write_safe(&self.dir.join("player.dat"), &self.player.to_bytes())?;
        let p = self.dir.join("tech-pending.json");
        match std::fs::remove_file(p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let _ = std::fs::remove_file(self.dir.join("tech-pending.json.bak"));
        self.pending_tech = None;
        self.tech_pending_sent = false;
        if !reply.accepted {
            self.item_name = Some((reply.error, 3.0));
        }
        Ok(Some(reply.id))
    }
    pub fn apply_tech_snapshot(
        &mut self,
        hell: bool,
        reset: bool,
        time: f64,
        weather: (wolfcraft_engine::weather::Sky, f32, f32),
        machines: Vec<Machine>,
    ) {
        if hell != self.hell {
            return;
        }
        if time.is_finite() && time >= 0.0 {
            self.player.world_time = time;
        }
        self.weather.sky = weather.0;
        self.weather.rain = weather.1.clamp(0.0, 1.0);
        self.weather.timer = weather.2.max(0.0);
        if reset {
            let cells: Vec<_> = self.tech.keys().copied().collect();
            for c in cells {
                self.containers.remove(&c);
            }
            self.tech.clear();
            self.tech_revisions.clear();
        }
        for m in machines {
            let c = wolfcraft_net::technology::cell(m.pos);
            self.tech.insert(c, m.state);
            self.containers.insert(c, m.slots);
            self.tech_revisions.insert(c, m.revision);
        }
    }
}
