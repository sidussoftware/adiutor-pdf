//! Persistent user settings, stored as TOML in the per-user config directory.

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub language: LanguagePref,
    pub theme: ThemePref,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePref {
    #[default]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "it")]
    Italian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

impl LanguagePref {
    pub const ALL: [LanguagePref; 3] = [
        LanguagePref::System,
        LanguagePref::English,
        LanguagePref::Italian,
    ];

    /// The language to show; `System` follows the OS locale.
    pub fn resolve(self) -> Lang {
        match self {
            LanguagePref::System => Lang::detect(),
            LanguagePref::English => Lang::En,
            LanguagePref::Italian => Lang::It,
        }
    }
}

#[derive(Default)]
pub struct Loaded {
    pub settings: Settings,
    /// The file exists but could not be parsed; defaults are used and the file is left untouched.
    pub was_corrupt: bool,
}

pub fn load_from(path: &Path) -> Loaded {
    let parsed = match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|_| ()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(_) => Err(()),
    };
    Loaded {
        was_corrupt: parsed.is_err(),
        settings: parsed.unwrap_or_default(),
    }
}

/// Writes via a temp file and rename. With `backup_corrupt`, the old file is first kept as `.bak`.
pub fn save_to(path: &Path, s: &Settings, backup_corrupt: bool) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    if backup_corrupt && path.exists() {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        std::fs::copy(path, bak)?;
    }
    let text = toml::to_string_pretty(s).map_err(std::io::Error::other)?;
    let tmp = dir.join(".settings.toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

pub fn config_path() -> Option<PathBuf> {
    config_path_from(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

fn config_path_from(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(get("APPDATA")?);
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(get("HOME")?).join("Library/Application Support");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = match get("XDG_CONFIG_HOME") {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(get("HOME")?).join(".config"),
    };
    Some(base.join("adiutor-pdf").join("settings.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn italian() -> Settings {
        Settings {
            language: LanguagePref::Italian,
            theme: ThemePref::Dark,
        }
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/settings.toml");
        save_to(&p, &italian(), false).unwrap();
        let l = load_from(&p);
        assert_eq!(l.settings, italian());
        assert!(!l.was_corrupt);
    }

    #[test]
    fn missing_fields_default() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.toml");
        std::fs::write(&p, "").unwrap();
        assert_eq!(load_from(&p).settings, Settings::default());
    }

    #[test]
    fn unknown_fields_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.toml");
        std::fs::write(&p, "language = \"it\"\nfuture = 3").unwrap();
        let l = load_from(&p);
        assert_eq!(l.settings.language, LanguagePref::Italian);
        assert!(!l.was_corrupt);
    }

    #[test]
    fn corrupt_file_defaults_and_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.toml");
        std::fs::write(&p, "language = [").unwrap();
        let l = load_from(&p);
        assert_eq!(l.settings, Settings::default());
        assert!(l.was_corrupt);
        assert_eq!(std::fs::read(&p).unwrap(), b"language = [");
    }

    #[test]
    fn missing_file_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let l = load_from(&dir.path().join("settings.toml"));
        assert_eq!(l.settings, Settings::default());
        assert!(!l.was_corrupt);
    }

    #[test]
    fn save_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        save_to(&dir.path().join("settings.toml"), &italian(), false).unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings.toml"]);
    }

    #[test]
    fn corrupt_file_backed_up_on_first_save() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.toml");
        std::fs::write(&p, "language = [").unwrap();
        save_to(&p, &italian(), true).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("settings.toml.bak")).unwrap(),
            b"language = ["
        );
        assert_eq!(load_from(&p).settings, italian());
    }

    #[test]
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn config_path_per_os() {
        let env = |pairs: &'static [(&str, &str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            config_path_from(env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")])),
            Some(PathBuf::from("/x/adiutor-pdf/settings.toml"))
        );
        assert_eq!(
            config_path_from(env(&[("HOME", "/h")])),
            Some(PathBuf::from("/h/.config/adiutor-pdf/settings.toml"))
        );
        assert_eq!(config_path_from(env(&[])), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn config_path_per_os() {
        let p = config_path_from(|k| (k == "HOME").then(|| "/h".to_string()));
        assert_eq!(
            p,
            Some(PathBuf::from(
                "/h/Library/Application Support/adiutor-pdf/settings.toml"
            ))
        );
        assert_eq!(config_path_from(|_| None), None);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn config_path_per_os() {
        let p = config_path_from(|k| (k == "APPDATA").then(|| r"C:\a".to_string()));
        assert_eq!(
            p,
            Some(
                PathBuf::from(r"C:\a")
                    .join("adiutor-pdf")
                    .join("settings.toml")
            )
        );
        assert_eq!(config_path_from(|_| None), None);
    }
}
