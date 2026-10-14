//! The canonical game-state record (schema v1, `docs/canonical-state.md`), collected from the
//! native game's globals and encoded byte for byte like `oracle/canonical-state.lua`, so the
//! port's state at a checkpoint can be compared with the emulator's.
//!
//! The record is a fixed list of fields ([`FIELDS`], in the Lua's order). Values are read by
//! name from the game's C globals (never by PS1 offsets: the native structs are laid out
//! differently), pointers are followed (an entity's `posData` gives its position) or
//! normalised (script cursors become a buffer kind plus an offset from the buffer's base), and
//! the same exclusions apply. Binary: `"DW1CANON"`, u32 version, u32 body length, then every
//! field little-endian with no padding. Text: one `name = value` line per field.

use std::fmt::Write as _;

use decomp_c::bindings as c;

pub const VERSION: u32 = 1;
pub const MAGIC: &[u8; 8] = b"DW1CANON";
pub const HEADER_LEN: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    U8,
    S8,
    U16,
    S16,
    U32,
    S32,
}

impl Ty {
    pub const fn size(self) -> usize {
        match self {
            Ty::U8 | Ty::S8 => 1,
            Ty::U16 | Ty::S16 => 2,
            Ty::U32 | Ty::S32 => 4,
        }
    }
}
use Ty::*;

pub struct Field {
    pub name: &'static str,
    pub ty: Ty,
    pub count: usize,
    /// Rendered as `0x%0Nx` in the text form.
    pub hex: bool,
}

const fn f(name: &'static str, ty: Ty, count: usize, hex: bool) -> Field {
    Field {
        name,
        ty,
        count,
        hex,
    }
}

/// Every field, in the binary's order (the Lua's `C.fields`; a test checks it against the
/// layout table in `docs/canonical-state.md`).
pub const FIELDS: &[Field] = &[
    f("mode.main_state", S8, 1, false),
    f("mode.game_state", S8, 1, false),
    f("mode.is_in_menu", S32, 1, false),
    f("mode.map_layer_enabled", U8, 1, false),
    f("mode.pause_state", U8, 1, false),
    f("mode.tamer_state", S8, 1, false),
    f("mode.tamer_sub_state", S8, 1, false),
    f("mode.partner_state", S8, 1, false),
    f("mode.partner_sub_state", S8, 1, false),
    f("screen.current_screen", U8, 1, false),
    f("screen.previous_screen", U8, 1, false),
    f("screen.current_exit", U8, 1, false),
    f("screen.previous_exit", U8, 1, false),
    f("script.current_screen_id", U16, 1, false),
    f("script.current_script_id", U16, 1, false),
    f("script.active_map_script", U16, 1, false),
    f("script.active_buffer", U8, 1, false),
    f("script.cursor_buffer", U8, 1, false),
    f("script.cursor_offset", U32, 1, false),
    f("script.section", U8, 1, false),
    f("script.section_is_event", S32, 1, false),
    f("script.active_instruction", U8, 1, false),
    f("script.is_paused", S32, 1, false),
    f("script.param_1", U16, 1, false),
    f("script.state_2", U16, 1, false),
    f("script.delay_frames", U16, 1, false),
    f("script.stack_depth", U16, 1, false),
    f("script.stack_buffer", U8, 8, false),
    f("script.stack_offset", U32, 8, false),
    f("script.stack_script_id", U16, 8, false),
    f("script.stack_smth0", U8, 8, false),
    f("script.stack_smth1", U8, 8, false),
    f("rng.custom_val1", U32, 1, true),
    f("rng.custom_val2", U32, 1, true),
    f("rng.bios_rand_state", U32, 1, true),
    f("clock.year", U8, 1, false),
    f("clock.day", S16, 1, false),
    f("clock.hour", S16, 1, false),
    f("clock.minute", S16, 1, false),
    f("clock.current_frame", U16, 1, false),
    f("clock.last_handled_frame", U16, 1, false),
    f("clock.subframe_count", U16, 1, false),
    f("clock.is_gametime_running", S8, 1, false),
    f("clock.time_speed", U8, 1, false),
    f("clock.playtime_frames", U16, 1, false),
    f("clock.playtime_minutes", U16, 1, false),
    f("clock.playtime_hours", U16, 1, false),
    f("tamer.money", S32, 1, false),
    f("tamer.level", S8, 1, false),
    f("tamer.raised_count", U8, 1, false),
    f("tamer.entity_type", S32, 1, false),
    f("tamer.pos_valid", U8, 1, false),
    f("tamer.pos", S32, 3, false),
    f("tamer.yaw", S16, 1, false),
    f("partner.species", S32, 1, false),
    f("partner.pos_valid", U8, 1, false),
    f("partner.pos", S32, 3, false),
    f("partner.yaw", S16, 1, false),
    f("partner.base_off", S16, 1, false),
    f("partner.base_def", S16, 1, false),
    f("partner.base_speed", S16, 1, false),
    f("partner.base_brain", S16, 1, false),
    f("partner.moves_prio", U8, 4, false),
    f("partner.moves", U8, 4, false),
    f("partner.max_hp", S16, 1, false),
    f("partner.max_mp", S16, 1, false),
    f("partner.cur_hp", S16, 1, false),
    f("partner.cur_mp", S16, 1, false),
    f("partner.learned_moves", U32, 2, true),
    f("partner.name", U8, 20, false),
    f("partner.lives", S8, 1, false),
    f("partner.para.condition", U32, 1, true),
    f("partner.para.sleepy_hour", S16, 1, false),
    f("partner.para.sleepy_minute", S16, 1, false),
    f("partner.para.wakeup_hour", S16, 1, false),
    f("partner.para.wakeup_minute", S16, 1, false),
    f("partner.para.hours_awake_default", S16, 1, false),
    f("partner.para.hours_asleep_default", S16, 1, false),
    f("partner.para.time_awake_today", S16, 1, false),
    f("partner.para.sickness_counter", S16, 1, false),
    f("partner.para.missed_sleep_hours", S16, 1, false),
    f("partner.para.tiredness_sleep_timer", S16, 1, false),
    f("partner.para.poop_level", S16, 1, false),
    f("partner.para.virus_bar", S16, 1, false),
    f("partner.para.pooping_timer", S16, 1, false),
    f("partner.para.tiredness", S16, 1, false),
    f("partner.para.sub_tiredness", S16, 1, false),
    f("partner.para.tiredness_hunger_timer", S16, 1, false),
    f("partner.para.discipline", S16, 1, false),
    f("partner.para.happiness", S16, 1, false),
    f("partner.para.times_being_sick", S16, 1, false),
    f("partner.para.sickness_tries", S16, 1, false),
    f("partner.para.area_effect_timer", S16, 1, false),
    f("partner.para.sickness_timer", S16, 1, false),
    f("partner.para.injury_timer", S16, 1, false),
    f("partner.para.next_hunger_hour", S16, 1, false),
    f("partner.para.energy_level", S16, 1, false),
    f("partner.para.food_level", S16, 1, false),
    f("partner.para.starvation_timer", S16, 1, false),
    f("partner.para.weight", S16, 1, false),
    f("partner.para.refused_fav_food", S16, 1, false),
    f("partner.para.empty_stomach_timer", S16, 1, false),
    f("partner.para.remaining_lifetime", S16, 1, false),
    f("partner.para.age", S16, 1, false),
    f("partner.para.train_boost_flag", S16, 1, false),
    f("partner.para.train_boost_value", S16, 1, false),
    f("partner.para.train_boost_timer", S16, 1, false),
    f("partner.para.care_mistakes", S16, 1, false),
    f("partner.para.battles", S16, 1, false),
    f("partner.para.evo_timer", S16, 1, false),
    f("partner.para.fish_caught", S16, 1, false),
    f("partner.para.upgrade_hp_counter", S16, 1, false),
    f("partner.para.upgrade_mp_counter", S16, 1, false),
    f("partner.para.upgrade_off_counter", S16, 1, false),
    f("partner.para.bugged_upgrade_brains_counter", S16, 1, false),
    f("partner.para.upgrade_defense_counter", S16, 1, false),
    f("partner.para.upgrade_speed_counter", S16, 1, false),
    f("partner.para.upgrade_brain_counter", S16, 1, false),
    f("partner.para.suka_backup_hp", S16, 1, false),
    f("partner.para.suka_backup_mp", S16, 1, false),
    f("partner.para.suka_backup_off", S16, 1, false),
    f("partner.para.suka_backup_def", S16, 1, false),
    f("partner.para.suka_backup_speed", S16, 1, false),
    f("partner.para.suka_backup_brain", S16, 1, false),
    f("inventory.size", U8, 1, false),
    f("inventory.item_ids", U8, 30, false),
    f("inventory.counts", U8, 30, false),
    f("progress.cards", U8, 33, false),
    f("progress.triggers", U8, 100, false),
    f("progress.pstats", U8, 256, false),
    f("progress.merit", S16, 1, false),
    f("progress.battles_started", S16, 1, false),
    f("progress.battles_fled", S16, 1, false),
    f("progress.tournaments_won", S16, 1, false),
    f("progress.tournament_wins", S16, 1, false),
    f("progress.tournaments_lost", S16, 1, false),
];

/// The values of every field, in [`FIELDS`] order. Each value is widened to i64 (a scalar is a
/// one-element vector).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    pub values: Vec<Vec<i64>>,
}

impl Record {
    fn check(&self) {
        assert_eq!(
            self.values.len(),
            FIELDS.len(),
            "record has the wrong field count"
        );
        for (field, v) in FIELDS.iter().zip(&self.values) {
            assert_eq!(v.len(), field.count, "{}: wrong element count", field.name);
        }
    }

    /// The binary form: header, then the body.
    pub fn encode(&self) -> Vec<u8> {
        self.check();
        let mut body = Vec::new();
        for (field, v) in FIELDS.iter().zip(&self.values) {
            for &x in v {
                // Two's complement truncated to the field's width.
                body.extend_from_slice(&x.to_le_bytes()[..field.ty.size()]);
            }
        }
        let mut out = Vec::with_capacity(HEADER_LEN + body.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Reads a record from its binary form (tests); Err if the header, length or version is wrong.
    #[cfg(test)]
    pub fn decode(bytes: &[u8]) -> Result<Record, String> {
        if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
            return Err("not a DW1CANON record".into());
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != VERSION {
            return Err(format!("schema version {version}, expected {VERSION}"));
        }
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if bytes.len() != HEADER_LEN + len || len != body_len() {
            return Err(format!(
                "body length {len} (file {} bytes), expected {}",
                bytes.len(),
                body_len()
            ));
        }
        let mut at = HEADER_LEN;
        let mut values = Vec::new();
        for field in FIELDS {
            let mut v = Vec::new();
            for _ in 0..field.count {
                let n = field.ty.size();
                let mut raw = [0u8; 8];
                raw[..n].copy_from_slice(&bytes[at..at + n]);
                at += n;
                let u = u64::from_le_bytes(raw);
                v.push(match field.ty {
                    U8 | U16 | U32 => u as i64,
                    S8 => i64::from(u as u8 as i8),
                    S16 => i64::from(u as u16 as i16),
                    S32 => i64::from(u as u32 as i32),
                });
            }
            values.push(v);
        }
        Ok(Record { values })
    }

    /// The text form, like the Lua's `C.render`.
    pub fn render(&self) -> String {
        self.check();
        let mut out = format!("# dw1 canonical state schema v{VERSION}\n");
        for (field, v) in FIELDS.iter().zip(&self.values) {
            let scalar = |x: i64| -> String {
                if field.hex {
                    let mask = (1u64 << (8 * field.ty.size())) - 1;
                    format!("{:#0w$x}", x as u64 & mask, w = 2 + 2 * field.ty.size())
                } else {
                    x.to_string()
                }
            };
            let parts: Vec<String> = if field.count == 1 {
                vec![scalar(v[0])]
            } else if field.ty == U8 && field.count > 4 {
                v.iter().map(|x| format!("{x:02x}")).collect()
            } else {
                v.iter().map(|&x| scalar(x)).collect()
            };
            let _ = writeln!(out, "{} = {}", field.name, parts.join(" "));
        }
        out
    }
}

/// Body length in bytes (818 for schema v1).
#[cfg(test)]
pub fn body_len() -> usize {
    FIELDS.iter().map(|f| f.ty.size() * f.count).sum()
}

// ---- collection from the native game ----

unsafe extern "C" {
    static mut MAIN_STATE: i8;
    static mut GAME_STATE: i8;
    static mut IS_IN_MENU: i32;
    static mut MAP_LAYER_ENABLED: u8;
    static mut PAUSE_STATE: u8;
    static mut TAMER_STATE: i8;
    static mut TAMER_SUB_STATE: i8;
    static mut PARTNER_STATE: i8;
    static mut PARTNER_SUB_STATE: i8;
    static mut CURRENT_SCREEN: u8;
    static mut PREVIOUS_SCREEN: u8;
    static mut CURRENT_EXIT: u8;
    static mut PREVIOUS_EXIT: u8;
    static mut CURRENT_SCREEN_ID: u16;
    static mut CURRENT_SCRIPT_ID: u16;
    static mut ACTIVE_MAP_SCRIPT: u16;
    static mut ACTIVE_SCRIPT: *mut u8;
    static mut SCRIPT_POINTER: *mut u8;
    static mut SCRIPT_SECTION: u8;
    static mut SCRIPT_SECTION_IS_EVENT: i32;
    static mut ACTIVE_INSTRUCTION: u8;
    static mut IS_SCRIPT_PAUSED: i32;
    static mut SCRIPT_PARAM_1: u16;
    static mut SCRIPT_STATE_2: u16;
    static mut DELAY_FRAMES: u16;
    static mut SCRIPT_STACK_OFFSET: u16;
    static mut MAPHEAD_SCRIPT_PTR: *mut u8;
    static mut MAP_SCRIPT_PTR: *mut u8;
    static mut SCRIPT_STATE: c::ScriptState;
    static mut YEAR: u8;
    static mut DAY: i16;
    static mut HOUR: i16;
    static mut MINUTE: i16;
    static mut CURRENT_FRAME: u16;
    static mut LAST_HANDLED_FRAME: u16;
    static mut SUBFRAME_COUNT: u16;
    static mut IS_GAMETIME_RUNNING: i8;
    static mut PLAYTIME_FRAMES: u16;
    static mut PLAYTIME_MINUTES: u16;
    static mut PLAYTIME_HOURS: u16;
    static mut MONEY: i32;
    static mut TAMER_ENTITY: c::TamerEntity;
    static mut PARTNER_ENTITY: c::PartnerEntity;
    static mut PARTNER_PARA: c::PartnerPara;
    static mut INVENTORY: c::Inventory;
    static mut MERIT: i16;
    static mut BATTLES_STARTED: i16;
    static mut BATTLES_FLED: i16;
    static mut TOURNAMENTS_WON: i16;
    static mut TOURNAMENT_WINS: i16;
    static mut TOURNAMENTS_LOST: i16;
}

/// Reads a game global (the game is single-threaded and paused while we read).
macro_rules! g {
    ($name:expr) => {
        unsafe { (&raw const $name).read_volatile() }
    };
}

/// The script buffers' sizes (`MAPHEAD_SCRIPT_BUFFER`, `MAP_SCRIPT_BUFFER`).
const MAPHEAD_SCRIPT_SIZE: usize = 25000;
const MAP_SCRIPT_SIZE: usize = 16384;

/// Which resident buffer a script pointer lies in (0 null, 1 maphead SCN buffer, 2 cached map
/// SCN buffer, 3 anywhere else) and its byte offset from that buffer's base (0 for kinds 0
/// and 3). Same rule as the Lua's `normScript`.
fn norm_script(p: usize, maphead: usize, map: usize) -> (i64, i64) {
    if p == 0 {
        return (0, 0);
    }
    for (kind, base, size) in [(1, maphead, MAPHEAD_SCRIPT_SIZE), (2, map, MAP_SCRIPT_SIZE)] {
        if base != 0 && p >= base && p - base <= size {
            return (kind, (p - base) as i64);
        }
    }
    (3, 0)
}

fn norm(p: *mut u8) -> (i64, i64) {
    norm_script(
        p as usize,
        g!(MAPHEAD_SCRIPT_PTR) as usize,
        g!(MAP_SCRIPT_PTR) as usize,
    )
}

/// Builds a record, checking each value against the next field of [`FIELDS`] by name.
struct Builder {
    values: Vec<Vec<i64>>,
}

impl Builder {
    fn one(&mut self, name: &str, v: impl Into<i64>) {
        self.many(name, vec![v.into()]);
    }
    fn many(&mut self, name: &str, v: Vec<i64>) {
        let field = &FIELDS[self.values.len()];
        assert_eq!(field.name, name, "collection order differs from FIELDS");
        assert_eq!(field.count, v.len(), "{name}: wrong element count");
        self.values.push(v);
    }
}

fn bytes(a: &[u8]) -> Vec<i64> {
    a.iter().map(|&x| i64::from(x)).collect()
}

/// An entity's position through its `posData`: (valid, [x, y, z], yaw). Null gives zeros.
fn position(pos: *mut c::PositionData) -> (i64, Vec<i64>, i64) {
    if pos.is_null() {
        return (0, vec![0; 3], 0);
    }
    let p = unsafe { pos.read() };
    (
        1,
        [p.location.vx, p.location.vy, p.location.vz]
            .map(i64::from)
            .to_vec(),
        i64::from(p.rotation.vy),
    )
}

/// Reads the native game's state. Call between game steps (the checkpoint hook runs inside
/// `PadRead`, where the game is paused at a defined point).
pub fn collect() -> Record {
    let mut b = Builder { values: Vec::new() };
    b.one("mode.main_state", g!(MAIN_STATE));
    b.one("mode.game_state", g!(GAME_STATE));
    b.one("mode.is_in_menu", g!(IS_IN_MENU));
    b.one("mode.map_layer_enabled", g!(MAP_LAYER_ENABLED));
    b.one("mode.pause_state", g!(PAUSE_STATE));
    b.one("mode.tamer_state", g!(TAMER_STATE));
    b.one("mode.tamer_sub_state", g!(TAMER_SUB_STATE));
    b.one("mode.partner_state", g!(PARTNER_STATE));
    b.one("mode.partner_sub_state", g!(PARTNER_SUB_STATE));

    b.one("screen.current_screen", g!(CURRENT_SCREEN));
    b.one("screen.previous_screen", g!(PREVIOUS_SCREEN));
    b.one("screen.current_exit", g!(CURRENT_EXIT));
    b.one("screen.previous_exit", g!(PREVIOUS_EXIT));
    b.one("script.current_screen_id", g!(CURRENT_SCREEN_ID));
    b.one("script.current_script_id", g!(CURRENT_SCRIPT_ID));
    b.one("script.active_map_script", g!(ACTIVE_MAP_SCRIPT));
    b.one("script.active_buffer", norm(g!(ACTIVE_SCRIPT)).0);
    let cursor = norm(g!(SCRIPT_POINTER));
    b.one("script.cursor_buffer", cursor.0);
    b.one("script.cursor_offset", cursor.1);
    b.one("script.section", g!(SCRIPT_SECTION));
    b.one("script.section_is_event", g!(SCRIPT_SECTION_IS_EVENT));
    b.one("script.active_instruction", g!(ACTIVE_INSTRUCTION));
    b.one("script.is_paused", g!(IS_SCRIPT_PAUSED));
    b.one("script.param_1", g!(SCRIPT_PARAM_1));
    b.one("script.state_2", g!(SCRIPT_STATE_2));
    b.one("script.delay_frames", g!(DELAY_FRAMES));
    let depth = usize::from(g!(SCRIPT_STACK_OFFSET));
    b.one("script.stack_depth", depth as i64);
    // Entries at or beyond the depth hold stale bytes: zero.
    let ss = g!(SCRIPT_STATE);
    let part = |f: &dyn Fn(&c::StackEntry) -> i64| -> Vec<i64> {
        (0..8)
            .map(|i| if i < depth { f(&ss.stack[i]) } else { 0 })
            .collect()
    };
    b.many("script.stack_buffer", part(&|e| norm(e.scriptPtr).0));
    b.many("script.stack_offset", part(&|e| norm(e.scriptPtr).1));
    b.many("script.stack_script_id", part(&|e| i64::from(e.scriptId)));
    b.many("script.stack_smth0", part(&|e| i64::from(e.smth[0])));
    b.many("script.stack_smth1", part(&|e| i64::from(e.smth[1])));

    b.one("rng.custom_val1", g!(decomp_c::CUSTOM_RNG_VAL1));
    b.one("rng.custom_val2", g!(decomp_c::CUSTOM_RNG_VAL2));
    b.one("rng.bios_rand_state", psyq::bios::rand_state());

    b.one("clock.year", g!(YEAR));
    b.one("clock.day", g!(DAY));
    b.one("clock.hour", g!(HOUR));
    b.one("clock.minute", g!(MINUTE));
    b.one("clock.current_frame", g!(CURRENT_FRAME));
    b.one("clock.last_handled_frame", g!(LAST_HANDLED_FRAME));
    b.one("clock.subframe_count", g!(SUBFRAME_COUNT));
    b.one("clock.is_gametime_running", g!(IS_GAMETIME_RUNNING));
    b.one("clock.time_speed", ss.pstats[0]);
    b.one("clock.playtime_frames", g!(PLAYTIME_FRAMES));
    b.one("clock.playtime_minutes", g!(PLAYTIME_MINUTES));
    b.one("clock.playtime_hours", g!(PLAYTIME_HOURS));

    let tamer = g!(TAMER_ENTITY);
    b.one("tamer.money", g!(MONEY));
    b.one("tamer.level", tamer.tamerLevel);
    b.one("tamer.raised_count", tamer.raisedCount);
    b.one("tamer.entity_type", tamer.entity.type_);
    let (valid, xyz, yaw) = position(tamer.entity.posData);
    b.one("tamer.pos_valid", valid);
    b.many("tamer.pos", xyz);
    b.one("tamer.yaw", yaw);

    let partner = g!(PARTNER_ENTITY);
    let digi = &partner.digimonEntity;
    b.one("partner.species", digi.entity.type_);
    let (valid, xyz, yaw) = position(digi.entity.posData);
    b.one("partner.pos_valid", valid);
    b.many("partner.pos", xyz);
    b.one("partner.yaw", yaw);
    let (base, cur) = (&digi.stats.base, &digi.stats.current);
    b.one("partner.base_off", base.off);
    b.one("partner.base_def", base.def);
    b.one("partner.base_speed", base.speed);
    b.one("partner.base_brain", base.brain);
    b.many("partner.moves_prio", bytes(&base.movesPrio));
    b.many("partner.moves", bytes(&base.moves));
    b.one("partner.max_hp", base.hp);
    b.one("partner.max_mp", base.mp);
    b.one("partner.cur_hp", cur.currentHP);
    b.one("partner.cur_mp", cur.currentMP);
    b.many(
        "partner.learned_moves",
        partner.learnedMoves.iter().map(|&x| i64::from(x)).collect(),
    );
    b.many(
        "partner.name",
        partner.name.iter().map(|&x| i64::from(x as u8)).collect(),
    );
    b.one("partner.lives", partner.lives);
    let pp = g!(PARTNER_PARA);
    b.one("partner.para.condition", pp.condition);
    macro_rules! para {
        ($($name:literal => $field:ident),* $(,)?) => {
            $(b.one(concat!("partner.para.", $name), pp.$field);)*
        };
    }
    para! {
        "sleepy_hour" => sleepyHour, "sleepy_minute" => sleepyMinute,
        "wakeup_hour" => wakeupHour, "wakeup_minute" => wakeupMinute,
        "hours_awake_default" => hoursAwakeDefault, "hours_asleep_default" => hoursAsleepDefault,
        "time_awake_today" => timeAwakeToday, "sickness_counter" => sicknessCounter,
        "missed_sleep_hours" => missedSleepHours, "tiredness_sleep_timer" => tirednessSleepTimer,
        "poop_level" => poopLevel, "virus_bar" => virusBar, "pooping_timer" => poopingTimer,
        "tiredness" => tiredness, "sub_tiredness" => subTiredness,
        "tiredness_hunger_timer" => tirednessHungerTimer, "discipline" => discipline,
        "happiness" => happiness, "times_being_sick" => timesBeingSick,
        "sickness_tries" => sicknessTries, "area_effect_timer" => areaEffectTimer,
        "sickness_timer" => sicknessTimer, "injury_timer" => injuryTimer,
        "next_hunger_hour" => nextHungerHour, "energy_level" => energyLevel,
        "food_level" => foodLevel, "starvation_timer" => starvationTimer, "weight" => weight,
        "refused_fav_food" => refusedFavFood, "empty_stomach_timer" => emptyStomachTimer,
        "remaining_lifetime" => remainingLifetime, "age" => age,
        "train_boost_flag" => trainBoostFlag, "train_boost_value" => trainBoostValue,
        "train_boost_timer" => trainBoostTimer, "care_mistakes" => careMistakes,
        "battles" => battles, "evo_timer" => evoTimer, "fish_caught" => fishCaught,
        "upgrade_hp_counter" => upgradeHPcounter, "upgrade_mp_counter" => upgradeMPcounter,
        "upgrade_off_counter" => upgradeOffCounter,
        "bugged_upgrade_brains_counter" => buggedUpgradeBrainsCounter,
        "upgrade_defense_counter" => upgradeDefenseCounter,
        "upgrade_speed_counter" => upgradeSpeedCounter,
        "upgrade_brain_counter" => upgradeBrainCounter,
        "suka_backup_hp" => sukaBackupHP, "suka_backup_mp" => sukaBackupMP,
        "suka_backup_off" => sukaBackupOff, "suka_backup_def" => sukaBackupDef,
        "suka_backup_speed" => sukaBackupSpeed, "suka_backup_brain" => sukaBackupBrain,
    }

    let inv = g!(INVENTORY);
    b.one("inventory.size", inv.size);
    b.many("inventory.item_ids", bytes(&inv.types.array));
    b.many("inventory.counts", bytes(&inv.amounts.array));

    b.many("progress.cards", bytes(&ss.cards));
    b.many("progress.triggers", bytes(&ss.triggers));
    b.many("progress.pstats", bytes(&ss.pstats));
    b.one("progress.merit", g!(MERIT));
    b.one("progress.battles_started", g!(BATTLES_STARTED));
    b.one("progress.battles_fled", g!(BATTLES_FLED));
    b.one("progress.tournaments_won", g!(TOURNAMENTS_WON));
    b.one("progress.tournament_wins", g!(TOURNAMENT_WINS));
    b.one("progress.tournaments_lost", g!(TOURNAMENTS_LOST));

    let rec = Record { values: b.values };
    rec.check();
    rec
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout table of docs/canonical-state.md (generated from the Lua's `C.layout()`):
    /// (offset, size, name, type, count).
    fn doc_layout() -> Vec<(usize, usize, String, String, usize)> {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/canonical-state.md"
        ))
        .unwrap();
        text.lines()
            .filter_map(|l| {
                let c: Vec<&str> = l.split('|').map(str::trim).collect();
                let off = c.get(1)?.strip_prefix("0x")?;
                let ty = c.get(4)?;
                let (ty, count) = match ty.split_once('[') {
                    Some((t, n)) => (t, n.trim_end_matches(']').parse().unwrap()),
                    None => (*ty, 1),
                };
                Some((
                    usize::from_str_radix(off, 16).ok()?,
                    c[2].parse().ok()?,
                    c[3].trim_matches('`').to_string(),
                    ty.to_string(),
                    count,
                ))
            })
            .collect()
    }

    #[test]
    fn fields_match_the_documented_layout() {
        let doc = doc_layout();
        assert_eq!(doc.len(), FIELDS.len());
        let mut off = HEADER_LEN;
        for (field, (doff, size, name, ty, count)) in FIELDS.iter().zip(&doc) {
            let ty_name = format!("{:?}", field.ty).to_lowercase();
            assert_eq!(field.name, name);
            assert_eq!(&ty_name, ty, "{name}");
            assert_eq!(field.count, *count, "{name}");
            assert_eq!(off, *doff, "{name} offset");
            assert_eq!(field.ty.size() * field.count, *size, "{name} size");
            off += size;
        }
        assert_eq!(off, 834);
        assert_eq!(body_len(), 818);
    }

    #[test]
    fn native_collection_follows_the_field_list() {
        // Builder asserts every name and count; the globals are zero or initialised statics.
        let bytes = collect().encode();
        assert_eq!(bytes.len(), 834);
        assert_eq!(&bytes[..8], b"DW1CANON");
        assert_eq!(&bytes[8..16], &[1, 0, 0, 0, 0x32, 3, 0, 0]);
    }

    fn sample() -> Record {
        let values = FIELDS
            .iter()
            .enumerate()
            .map(|(i, f)| {
                (0..f.count)
                    .map(|j| {
                        let n = (i * 7 + j * 3) as i64;
                        match f.ty {
                            U8 => n % 256,
                            S8 => -(n % 128),
                            U16 => 0xF000 + n,
                            S16 => -1000 - n,
                            U32 => 0xDEAD_0000 + n,
                            S32 => -100_000 - n,
                        }
                    })
                    .collect()
            })
            .collect();
        Record { values }
    }

    #[test]
    fn encode_decode_round_trip_and_signedness() {
        let rec = sample();
        let bytes = rec.encode();
        assert_eq!(bytes.len(), 834);
        assert_eq!(Record::decode(&bytes).unwrap(), rec);
        // A signed byte is two's complement.
        let mut r = rec.clone();
        r.values[0] = vec![-3];
        assert_eq!(r.encode()[HEADER_LEN], 0xFD);
        assert!(Record::decode(&bytes[..833]).is_err());
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert!(Record::decode(&bad).is_err());
        let mut bad = bytes;
        bad[8] = 2;
        assert!(Record::decode(&bad).unwrap_err().contains("version"));
    }

    #[test]
    fn render_matches_the_luas_text_form() {
        let mut rec = sample();
        let at = |name: &str| FIELDS.iter().position(|f| f.name == name).unwrap();
        rec.values[at("rng.custom_val2")] = vec![0x0013_CC25];
        rec.values[at("partner.learned_moves")] = vec![4, 0];
        rec.values[at("partner.moves")] = vec![46, 255, 255, 53];
        rec.values[at("partner.name")] = [0x82, 0x60].repeat(10);
        rec.values[at("tamer.pos")] = vec![-1, 2, -2735];
        rec.values[at("mode.main_state")] = vec![-1];
        let text = rec.render();
        assert!(text.starts_with("# dw1 canonical state schema v1\n"));
        assert!(text.contains("\nrng.custom_val2 = 0x0013cc25\n"));
        assert!(text.contains("\npartner.learned_moves = 0x00000004 0x00000000\n"));
        assert!(text.contains("\npartner.moves = 46 255 255 53\n"));
        let name = ["82 60"; 10].join(" ");
        assert!(text.contains(&format!("\npartner.name = {name}\n")));
        assert!(text.contains("\ntamer.pos = -1 2 -2735\n"));
        assert!(text.contains("\nmode.main_state = -1\n"));
        assert_eq!(text.lines().count(), FIELDS.len() + 1);
    }

    #[test]
    fn script_pointers_are_normalised_to_buffers() {
        assert_eq!(norm_script(0, 0x1000, 0x9000), (0, 0));
        assert_eq!(norm_script(0x1010, 0x1000, 0x9000), (1, 0x10));
        assert_eq!(norm_script(0x1000 + 25000, 0x1000, 0x9000), (1, 25000));
        assert_eq!(norm_script(0x9000 + 100, 0x1000, 0x9000), (2, 100));
        assert_eq!(norm_script(0xF000, 0x1000, 0x9000), (3, 0));
        // Unset buffer bases never match.
        assert_eq!(norm_script(0x10, 0, 0), (3, 0));
    }

    /// Round trip through the emulator's own files when they are present (local, gitignored):
    /// decode each `.bin`, re-encode it, and check that rendering gives its `.txt`.
    #[test]
    fn emulator_records_round_trip_when_present() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/oracle-runs/s08-replay/record");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if !(name.starts_with("ckpt_") && name.ends_with(".bin")) {
                continue;
            }
            let bytes = std::fs::read(&p).unwrap();
            let rec = Record::decode(&bytes).unwrap();
            assert_eq!(rec.encode(), bytes, "{}", p.display());
            let txt = std::fs::read_to_string(p.with_extension("txt")).unwrap();
            assert_eq!(rec.render(), txt, "{}", p.display());
        }
    }
}
