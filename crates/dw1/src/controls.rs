//! Keyboard preferences are a host-side addition: never read by headless/replay hosts.
//! Persist physical key codes, not layout-dependent characters or SDL enum numbers.
use sdl3::keyboard::Scancode;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub mod macos;

pub const BUTTONS: [(&str, &str, u32); 14] = [
    ("up", "Up", 0x1000),
    ("down", "Down", 0x4000),
    ("left", "Left", 0x8000),
    ("right", "Right", 0x2000),
    ("cross", "Cross ×", 0x40),
    ("circle", "Circle ○", 0x20),
    ("square", "Square □", 0x80),
    ("triangle", "Triangle △", 0x10),
    ("select", "Select", 0x100),
    ("start", "Start", 0x800),
    ("l1", "L1", 4),
    ("r1", "R1", 8),
    ("l2", "L2", 1),
    ("r2", "R2", 2),
];

macro_rules! keys {
    ($($code:literal => $scan:ident, $label:literal);* $(;)?) => {
        pub const KEYS: &[(&str, Scancode, &str)] = &[$(($code, Scancode::$scan, $label)),*];
    }
}
// Escape cancels capture; Command/Control/Option and the existing picture/debug hotkeys
// stay with macOS/the host. Unknown DOM codes fail closed, both here and on disk.
keys! {
    "KeyA"=>A,"A"; "KeyB"=>B,"B"; "KeyC"=>C,"C"; "KeyD"=>D,"D";
    "KeyE"=>E,"E"; "KeyF"=>F,"F"; "KeyG"=>G,"G"; "KeyH"=>H,"H";
    "KeyI"=>I,"I"; "KeyJ"=>J,"J"; "KeyK"=>K,"K"; "KeyL"=>L,"L";
    "KeyM"=>M,"M"; "KeyN"=>N,"N"; "KeyO"=>O,"O"; "KeyP"=>P,"P";
    "KeyQ"=>Q,"Q"; "KeyR"=>R,"R"; "KeyS"=>S,"S"; "KeyT"=>T,"T";
    "KeyU"=>U,"U"; "KeyV"=>V,"V"; "KeyW"=>W,"W"; "KeyX"=>X,"X";
    "KeyY"=>Y,"Y"; "KeyZ"=>Z,"Z";
    "Digit0"=>_0,"0"; "Digit1"=>_1,"1"; "Digit2"=>_2,"2"; "Digit3"=>_3,"3";
    "Digit4"=>_4,"4"; "Digit5"=>_5,"5"; "Digit6"=>_6,"6"; "Digit7"=>_7,"7";
    "Digit8"=>_8,"8"; "Digit9"=>_9,"9";
    "ArrowUp"=>Up,"↑"; "ArrowDown"=>Down,"↓"; "ArrowLeft"=>Left,"←"; "ArrowRight"=>Right,"→";
    "Enter"=>Return,"Return"; "NumpadEnter"=>KpEnter,"Num Enter";
    "ShiftLeft"=>LShift,"Left Shift"; "ShiftRight"=>RShift,"Right Shift";
    "Space"=>Space,"Space"; "Tab"=>Tab,"Tab"; "Backspace"=>Backspace,"Delete ⌫";
    "Delete"=>Delete,"Forward Del"; "Home"=>Home,"Home"; "End"=>End,"End";
    "PageUp"=>PageUp,"Page Up"; "PageDown"=>PageDown,"Page Down";
    "Slash"=>Slash,"/"; "Period"=>Period,"."; "Comma"=>Comma,",";
    "Semicolon"=>Semicolon,";"; "Quote"=>Apostrophe,"'"; "Backquote"=>Grave,"`";
    "BracketLeft"=>LeftBracket,"["; "BracketRight"=>RightBracket,"]";
    "Backslash"=>Backslash,"\\"; "Minus"=>Minus,"−"; "Equal"=>Equals,"=";
    "F1"=>F1,"F1"; "F6"=>F6,"F6"; "F7"=>F7,"F7"; "F8"=>F8,"F8";
    "F9"=>F9,"F9"; "F10"=>F10,"F10";
    "Numpad0"=>Kp0,"Num 0"; "Numpad1"=>Kp1,"Num 1"; "Numpad2"=>Kp2,"Num 2";
    "Numpad3"=>Kp3,"Num 3"; "Numpad4"=>Kp4,"Num 4"; "Numpad5"=>Kp5,"Num 5";
    "Numpad6"=>Kp6,"Num 6"; "Numpad7"=>Kp7,"Num 7"; "Numpad8"=>Kp8,"Num 8";
    "Numpad9"=>Kp9,"Num 9"; "NumpadDecimal"=>KpPeriod,"Num .";
    "NumpadAdd"=>KpPlus,"Num +"; "NumpadSubtract"=>KpMinus,"Num −";
    "NumpadMultiply"=>KpMultiply,"Num ×"; "NumpadDivide"=>KpDivide,"Num /";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    codes: [&'static str; 14],
}
impl Default for Bindings {
    fn default() -> Self {
        Self {
            codes: [
                "ArrowUp",
                "ArrowDown",
                "ArrowLeft",
                "ArrowRight",
                "KeyZ",
                "KeyX",
                "KeyA",
                "KeyS",
                "ShiftRight",
                "Enter",
                "KeyQ",
                "KeyW",
                "Digit1",
                "Digit2",
            ],
        }
    }
}
impl Bindings {
    pub fn bit(&self, key: Scancode) -> u32 {
        self.codes
            .iter()
            .zip(BUTTONS)
            .find_map(|(c, (_, _, bit))| {
                KEYS.iter()
                    .any(|(code, scan, _)| code == c && *scan == key)
                    .then_some(bit)
            })
            .unwrap_or(0)
    }
    /// A duplicate assignment swaps the two buttons, keeping every button reachable.
    pub fn assign(&mut self, button: &str, code: &str) -> Result<Option<&'static str>, String> {
        let index = BUTTONS
            .iter()
            .position(|b| b.0 == button)
            .ok_or("Unknown button.")?;
        let code = KEYS
            .iter()
            .find(|k| k.0 == code)
            .ok_or("That key is reserved or unsupported. Choose another key.")?
            .0;
        let other = self
            .codes
            .iter()
            .position(|c| *c == code)
            .filter(|i| *i != index);
        if let Some(i) = other {
            self.codes[i] = self.codes[index];
        }
        self.codes[index] = code;
        Ok(other.map(|i| BUTTONS[i].1))
    }
    fn json(&self) -> serde_json::Value {
        let bindings: serde_json::Map<String, serde_json::Value> = BUTTONS
            .iter()
            .zip(self.codes)
            .map(|(b, c)| (b.0.into(), c.into()))
            .collect();
        serde_json::json!({"version":1, "bindings":bindings})
    }
    fn parse(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v["version"] != 1 {
            return Err("unsupported controls file version".into());
        }
        let map = v["bindings"].as_object().ok_or("missing bindings")?;
        if map.len() != BUTTONS.len() {
            return Err("expected all 14 buttons".into());
        }
        let mut b = Self::default();
        let mut seen = HashSet::new();
        for (i, button) in BUTTONS.iter().enumerate() {
            let code = map
                .get(button.0)
                .and_then(|v| v.as_str())
                .ok_or("missing button")?;
            let code = KEYS
                .iter()
                .find(|k| k.0 == code)
                .ok_or("reserved or unsupported key")?
                .0;
            if !seen.insert(code) {
                return Err("duplicate key".into());
            }
            b.codes[i] = code;
        }
        Ok(b)
    }
    fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Keep the old valid file if writing fails. This app has a single preferences writer.
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        let result = (|| {
            std::fs::write(
                &tmp,
                format!("{}\n", serde_json::to_string_pretty(&self.json()).unwrap()),
            )
            .map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(tmp);
        }
        result
    }
}

/// State owned by the menu, independent of the game host (a menu callback may run while
/// SDL polls native events). Revisions also invalidate keys held across the modal window.
pub struct EditorState {
    pub bindings: Bindings,
    path: Option<PathBuf>,
    pub revision: u64,
    /// SDL monotonic nanoseconds when the editor closed; reject older capture input only.
    pub closed_at_ns: u64,
    pub status: String,
    pub failed: bool,
}
impl EditorState {
    pub fn load() -> Self {
        let path = std::env::var_os("DW1_CONTROLS_FILE")
            .map(PathBuf::from)
            .or_else(|| crate::default_save_dir().map(|p| p.join("controls.json")));
        Self::at(path)
    }
    fn at(path: Option<PathBuf>) -> Self {
        let mut state = Self {
            bindings: Bindings::default(),
            path,
            revision: 0,
            closed_at_ns: 0,
            status: "One key per button. Duplicate keys swap assignments.".into(),
            failed: false,
        };
        if let Some(path) = &state.path {
            match std::fs::read_to_string(path) {
                Ok(text) => match Bindings::parse(&text) {
                    Ok(b) => state.bindings = b,
                    Err(e) => {
                        state.failed = true;
                        state.status = format!(
                            "Could not load controls: {e}. Defaults are in use; the file is unchanged."
                        );
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    state.failed = true;
                    state.status = format!("Could not read controls: {e}. Defaults are in use.");
                }
            }
        }
        state
    }
    pub fn change(&mut self, button: &str, code: &str) {
        let mut next = self.bindings.clone();
        match next.assign(button, code) {
            Ok(swap) => {
                let status = swap
                    .map(|b| format!("Saved · swapped with {b}."))
                    .unwrap_or("Saved · ready to play.".into());
                self.commit(next, status);
            }
            Err(e) => {
                self.status = e;
                self.failed = true;
            }
        }
    }
    pub fn reset(&mut self) {
        self.commit(Bindings::default(), "Default controls restored.".into());
    }
    fn commit(&mut self, next: Bindings, status: String) {
        let result = self
            .path
            .as_deref()
            .ok_or("No preferences folder is available.".into())
            .and_then(|p| next.save(p));
        match result {
            Ok(()) => {
                self.bindings = next;
                self.revision += 1;
                self.status = status;
                self.failed = false;
            }
            Err(e) => {
                self.status = format!("Not applied — could not save: {e}");
                self.failed = true;
            }
        }
    }
    pub fn view(&self) -> String {
        let buttons: Vec<_> = BUTTONS
            .iter()
            .zip(self.bindings.codes)
            .map(|(b, code)| {
                let label = KEYS.iter().find(|k| k.0 == code).unwrap().2;
                serde_json::json!({"id":b.0,"code":code,"label":label})
            })
            .collect();
        serde_json::json!({"buttons":buttons,"status":self.status,"failed":self.failed}).to_string()
    }
}

#[derive(Default)]
pub struct KeyboardState {
    held: HashSet<Scancode>,
}
impl KeyboardState {
    pub fn key(&mut self, key: Scancode, down: bool) {
        if down {
            self.held.insert(key);
        } else {
            self.held.remove(&key);
        }
    }
    pub fn clear(&mut self) {
        self.held.clear();
    }
    pub fn bits(&self, bindings: &Bindings) -> u32 {
        self.held
            .iter()
            .fold(0, |bits, key| bits | bindings.bit(*key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_no_aliases() {
        let b = Bindings::default();
        assert_eq!(b.bit(Scancode::Z), 0x40);
        assert_eq!(b.bit(Scancode::Slash), 0);
        assert_eq!(b.bit(Scancode::KpEnter), 0);
    }
    #[test]
    fn live_rebind_and_release() {
        let mut b = Bindings::default();
        let mut k = KeyboardState::default();
        k.key(Scancode::Slash, true);
        assert_eq!(k.bits(&b), 0);
        b.assign("cross", "Slash").unwrap();
        assert_eq!(k.bits(&b), 0x40);
        k.key(Scancode::Slash, false);
        assert_eq!(k.bits(&b), 0);
        k.key(Scancode::Slash, true);
        k.clear();
        assert_eq!(k.bits(&b), 0);
    }
    #[test]
    fn duplicate_swaps_and_round_trips() {
        let mut b = Bindings::default();
        assert_eq!(b.assign("cross", "KeyX").unwrap(), Some("Circle ○"));
        assert_eq!(b.bit(Scancode::X), 0x40);
        assert_eq!(b.bit(Scancode::Z), 0x20);
        assert_eq!(Bindings::parse(&b.json().to_string()).unwrap(), b);
    }
    #[test]
    fn invalid_inputs_do_not_change_mapping() {
        let mut b = Bindings::default();
        let old = b.clone();
        for key in [
            "F2",
            "F3",
            "F4",
            "F5",
            "F11",
            "F12",
            "Escape",
            "MetaLeft",
            "ControlLeft",
            "AltRight",
            "garbage",
        ] {
            assert!(b.assign("cross", key).is_err());
        }
        assert!(b.assign("garbage", "KeyX").is_err());
        assert_eq!(b, old);
    }
    #[test]
    fn malformed_files_rejected() {
        let b = Bindings::default();
        for text in ["", "{", "{}", "{\"version\":2}"] {
            assert!(Bindings::parse(text).is_err());
        }
        let mut v = b.json();
        v["bindings"]["cross"] = "KeyX".into();
        assert!(Bindings::parse(&v.to_string()).is_err());
        v["bindings"]["cross"] = "F12".into();
        assert!(Bindings::parse(&v.to_string()).is_err());
    }
    #[test]
    fn persistence_and_failure_are_transactional() {
        let dir = std::env::temp_dir().join(format!("dw1-controls-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("controls.json");
        let mut s = EditorState::at(Some(path.clone()));
        s.change("cross", "Slash");
        assert!(!s.failed);
        assert_eq!(EditorState::at(Some(path)).bindings, s.bindings);
        let old = s.bindings.clone();
        s.path = Some(dir.clone());
        s.change("circle", "Period");
        assert!(s.failed);
        assert_eq!(s.bindings, old);
        s.reset();
        assert!(s.failed);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn key_table_is_unambiguous() {
        let mut codes = HashSet::new();
        let mut scans = HashSet::new();
        for (code, scan, _) in KEYS {
            assert!(codes.insert(code));
            assert!(scans.insert(scan));
        }
    }
    #[test]
    fn independent_keys_and_clear_do_not_stick() {
        let b = Bindings::default();
        let mut k = KeyboardState::default();
        k.key(Scancode::Z, true);
        k.key(Scancode::Up, true);
        assert_eq!(k.bits(&b), 0x1040);
        k.key(Scancode::Z, false);
        assert_eq!(k.bits(&b), 0x1000);
        k.clear();
        assert_eq!(k.bits(&b), 0);
    }
    #[test]
    fn malformed_file_is_left_untouched_and_reset_can_repair_it() {
        let dir =
            std::env::temp_dir().join(format!("dw1-controls-malformed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("controls.json");
        std::fs::write(&path, "{truncated").unwrap();
        let mut s = EditorState::at(Some(path.clone()));
        assert!(s.failed);
        assert_eq!(s.bindings, Bindings::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{truncated");
        s.reset();
        assert!(!s.failed);
        assert_eq!(
            Bindings::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            Bindings::default()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn editor_has_an_unbranded_vector_target_for_every_button() {
        let html = include_str!("controls/editor.html");
        for (id, _, _) in BUTTONS {
            assert!(html.contains(&format!("id=\"shape-{id}\"")), "{id}");
        }
        assert!(!html.contains("https://"));
        assert!(html.contains("connect-src 'none'"));
    }
}
