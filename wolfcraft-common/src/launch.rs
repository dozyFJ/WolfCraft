//! Command-line protocol between WolfCraftLauncher.exe and WolfCraft.exe.
//!
//! The launcher starts the game as:
//! `WolfCraft.exe --data-dir <dir> --game-version <id> --launched-by-launcher`

use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LaunchArgs {
    /// Root folder for saves, settings and logs. Defaults to `%APPDATA%\WolfCraft`.
    pub data_dir: Option<PathBuf>,
    /// Version id the launcher selected.
    pub game_version: Option<String>,
    pub from_launcher: bool,
}

impl LaunchArgs {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut out = LaunchArgs::default();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--data-dir" => out.data_dir = it.next().map(PathBuf::from),
                "--game-version" => out.game_version = it.next(),
                "--launched-by-launcher" => out.from_launcher = true,
                _ => {}
            }
        }
        out
    }

    pub fn to_args(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(d) = &self.data_dir {
            v.push("--data-dir".into());
            v.push(d.to_string_lossy().into_owned());
        }
        if let Some(g) = &self.game_version {
            v.push("--game-version".into());
            v.push(g.clone());
        }
        if self.from_launcher {
            v.push("--launched-by-launcher".into());
        }
        v
    }

    pub fn resolved_data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(crate::platform::default_data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let a = LaunchArgs { data_dir: Some("C:/x y".into()), game_version: Some("0.1.0".into()), from_launcher: true };
        let parsed = LaunchArgs::parse(a.to_args());
        assert_eq!(a, parsed);
    }

    #[test]
    fn ignores_unknown() {
        let a = LaunchArgs::parse(["--foo".to_string(), "--launched-by-launcher".to_string()]);
        assert!(a.from_launcher);
        assert!(a.data_dir.is_none());
    }
}
