//! Controller settings the manager passes to a game: a sparse set of the input section's keys (the same names as
//! nx_app.ini), written to a manager-owned overlay that the game reads with NXRT_CONTROLLER.
//!
//! The manager's defaults apply to every game; a per-game set overrides them key by key. A key neither sets is left
//! to the game's own settings. Nothing here writes the game's nx_app.ini.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The Switch inputs a controller button can drive (input/bindings.c's "bind_pad_" names, in its order).
pub const PAD_INPUTS: &[&str] = &[
    "a", "b", "x", "y", "lstick", "rstick", "l", "r", "zl", "zr", "plus", "minus", "left", "up",
    "right", "down",
];
/// The host buttons those inputs can be bound to (bindings.c's PADB_HOST names).
pub const PAD_BUTTONS: &[&str] = &[
    "south",
    "east",
    "west",
    "north",
    "back",
    "guide",
    "start",
    "lstick",
    "rstick",
    "lshoulder",
    "rshoulder",
    "dpad_up",
    "dpad_down",
    "dpad_left",
    "dpad_right",
    "misc1",
    "rpaddle1",
    "lpaddle1",
    "rpaddle2",
    "lpaddle2",
    "touchpad",
    "misc2",
    "misc3",
    "misc4",
    "misc5",
    "misc6",
    "ltrigger",
    "rtrigger",
];
pub const PROMPTS: &[&str] = &["auto", "nintendo", "xbox", "playstation", "keyboard"];

/// A sparse input-section configuration: only what is set is passed; the rest is the game's own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpad_move: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rumble: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rumble_strength: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led: Option<bool>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bind_pad: BTreeMap<String, String>,
}

impl Controls {
    pub fn is_empty(&self) -> bool {
        self.prompts.is_none()
            && self.dpad_move.is_none()
            && self.rumble.is_none()
            && self.rumble_strength.is_none()
            && self.led.is_none()
            && self.bind_pad.is_empty()
    }

    /// This set with `over` layered on top (used for defaults + a game's own overrides).
    pub fn merged(&self, over: &Controls) -> Controls {
        let mut bind_pad = self.bind_pad.clone();
        bind_pad.extend(over.bind_pad.iter().map(|(k, v)| (k.clone(), v.clone())));
        Controls {
            prompts: over.prompts.clone().or_else(|| self.prompts.clone()),
            dpad_move: over.dpad_move.or(self.dpad_move),
            rumble: over.rumble.or(self.rumble),
            rumble_strength: over.rumble_strength.or(self.rumble_strength),
            led: over.led.or(self.led),
            bind_pad,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(p) = &self.prompts {
            if !PROMPTS.contains(&p.as_str()) {
                return Err(format!("Unknown button prompts: {p}"));
            }
        }
        if let Some(s) = self.rumble_strength {
            if s > 100 {
                return Err("Rumble strength is 0-100".into());
            }
        }
        for (input, button) in &self.bind_pad {
            if !PAD_INPUTS.contains(&input.as_str()) {
                return Err(format!("Unknown controller input: {input}"));
            }
            if !PAD_BUTTONS.contains(&button.as_str()) {
                return Err(format!("Unknown controller button: {button}"));
            }
        }
        Ok(())
    }

    /// The lines the game's input section understands (input/input.c input_key).
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(p) = &self.prompts {
            out.push(format!("prompts = {p}"));
        }
        if let Some(v) = self.dpad_move {
            out.push(format!("dpad_move = {}", v as u8));
        }
        if let Some(v) = self.rumble {
            out.push(format!("rumble = {}", v as u8));
        }
        if let Some(v) = self.rumble_strength {
            out.push(format!("rumble_strength = {v}"));
        }
        if let Some(v) = self.led {
            out.push(format!("led = {}", v as u8));
        }
        for input in PAD_INPUTS {
            if let Some(button) = self.bind_pad.get(*input) {
                out.push(format!("bind_pad_{input} = {button}"));
            }
        }
        out
    }

    /// Writes the overlay the game reads. An empty set removes any old one.
    pub fn write_overlay(&self, path: &Path) -> Result<(), String> {
        if self.is_empty() {
            let _ = std::fs::remove_file(path);
            return Ok(());
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let text = format!(
            "# nxrt-frontend controller settings (manager-owned; the game's own nx_app.ini is not changed)\n{}\n",
            self.lines().join("\n")
        );
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Where a game's effective controls are written before it starts.
pub fn overlay_path(key: &str) -> PathBuf {
    let mut safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while safe.contains("--") {
        safe = safe.replace("--", "-");
    }
    let safe = safe.trim_matches('-');
    let safe = if safe.is_empty() { "game" } else { safe };
    let short: String = safe.chars().take(60).collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    crate::library::dir()
        .join("controls")
        .join(format!("{short}-{:08x}.conf", h.finish() as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn global() -> Controls {
        Controls {
            prompts: Some("nintendo".into()),
            rumble: Some(false),
            bind_pad: [("a".to_string(), "south".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn per_game_layers_over_the_defaults() {
        let game = Controls {
            dpad_move: Some(true),
            bind_pad: [
                ("a".to_string(), "north".to_string()),
                ("x".to_string(), "west".to_string()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let c = global().merged(&game);
        assert_eq!(c.prompts.as_deref(), Some("nintendo"));
        assert_eq!(c.rumble, Some(false));
        assert_eq!(c.dpad_move, Some(true));
        assert_eq!(c.bind_pad["a"], "north");
        assert_eq!(c.bind_pad["x"], "west");
        assert!(!c.is_empty());
        assert!(Controls::default().is_empty());
    }

    #[test]
    fn only_set_keys_are_written() {
        let c = Controls {
            prompts: Some("xbox".into()),
            rumble_strength: Some(70),
            bind_pad: [("zl".to_string(), "ltrigger".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let lines = c.lines();
        assert!(lines.contains(&"prompts = xbox".to_string()));
        assert!(lines.contains(&"rumble_strength = 70".to_string()));
        assert!(lines.contains(&"bind_pad_zl = ltrigger".to_string()));
        assert!(!lines.iter().any(|l| l.starts_with("rumble =")));
        assert!(!lines.iter().any(|l| l.starts_with("bind_pad_a =")));
    }

    #[test]
    fn overlay_round_trips_through_a_file() {
        let c = global();
        let path = std::env::temp_dir().join(format!("nxrt-controls-{}.conf", std::process::id()));
        c.write_overlay(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("bind_pad_a = south"));
        c.write_overlay(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        // (an empty set removes the overlay instead of passing an empty file)
        Controls::default().write_overlay(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn validation_rejects_unknown_names() {
        assert!(Controls {
            prompts: Some("gamecube".into()),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Controls {
            rumble_strength: Some(101),
            ..Default::default()
        }
        .validate()
        .is_err());
        let mut c = Controls::default();
        c.bind_pad.insert("bogus".into(), "south".into());
        assert!(c.validate().is_err());
        let mut c = Controls::default();
        c.bind_pad.insert("a".into(), "bogus".into());
        assert!(c.validate().is_err());
        assert!(global().validate().is_ok());
    }
}
