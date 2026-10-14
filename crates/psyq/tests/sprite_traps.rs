//! `GsSortSprite` and `GsSortFastSprite` stop the program on what the oracle runs never showed
//! (attribute bits outside the known ones, sides over 300) instead of guessing, and draw nothing
//! for the cases that were observed to draw nothing (display off, a side of 0), unknown bits
//! included only where they'd trap. Each case runs in a child process (this test binary again,
//! with `DW1_TRAP_CASE` set) because the stop is an abort.

use psyq::libgs::{GsGetWorkBase, GsOt, GsSetWorkBase, GsSortFastSprite, GsSortSprite, GsSprite};
use psyq::memory::{LINK_END, ps1_ram_ptr};

const OT_ENTRIES: u32 = 0x801E_0300;
const PACKETS: u32 = 0x801E_0400;

fn sprite(attribute: u32, w: u16, h: u16) -> GsSprite {
    GsSprite {
        attribute,
        x: 10,
        y: 20,
        w,
        h,
        tpage: 0x1E,
        u: 0,
        v: 0,
        cx: 256,
        cy: 499,
        r: 128,
        g: 128,
        b: 128,
        mx: 0,
        my: 0,
        scalex: 4096,
        scaley: 4096,
        rotate: 0,
    }
}

fn run_case(name: &str) {
    let (fast, s) = match name {
        "sprite_unknown_bit" => (false, sprite(0x0000_0001, 8, 8)),
        "sprite_unknown_bit_hidden" => (false, sprite(0x8800_0000, 8, 8)),
        "sprite_too_wide" => (false, sprite(0, 301, 8)),
        "sprite_too_tall_hidden" => (false, sprite(0x8000_0000, 8, 400)),
        "fast_unknown_bit" => (true, sprite(0x0400_0000, 8, 8)),
        "fast_zero_width_too_tall" => (true, sprite(0, 0, 301)),
        "sprite_hidden" => (false, sprite(0x8000_0000, 8, 8)),
        "sprite_zero_height" => (false, sprite(0x5000_0000, 8, 0)),
        "fast_zero_width" => (true, sprite(0, 0, 3)),
        "fast_hidden_big" => (true, sprite(0xC000_0000, 300, 300)),
        other => panic!("unknown case {other}"),
    };
    unsafe {
        let org = ps1_ram_ptr(OT_ENTRIES).cast::<u32>();
        for i in 0..16 {
            org.add(i).write(LINK_END);
        }
        let mut ot = GsOt {
            length: 4,
            org,
            offset: 0,
            point: 0,
            tag: org.add(15),
        };
        GsSetWorkBase(ps1_ram_ptr(PACKETS));
        let before = GsGetWorkBase();
        let sp = &raw const s;
        if fast {
            GsSortFastSprite(sp, &mut ot, 9);
        } else {
            GsSortSprite(sp, &mut ot, 9);
        }
        // Reached only by the cases that draw nothing: no packet, no link.
        assert_eq!(GsGetWorkBase(), before, "{name}: wrote a packet");
        assert!((0..16).all(|i| org.add(i).read() == LINK_END), "{name}: linked something");
    }
}

#[test]
fn unobserved_sprites_stop_and_hidden_ones_draw_nothing() {
    if let Ok(case) = std::env::var("DW1_TRAP_CASE") {
        run_case(&case);
        return;
    }
    let child = |case: &str| {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "unobserved_sprites_stop_and_hidden_ones_draw_nothing",
                "--nocapture",
            ])
            .env("DW1_TRAP_CASE", case)
            .output()
            .unwrap()
    };
    for (case, function) in [
        ("sprite_unknown_bit", "GsSortSprite"),
        ("sprite_unknown_bit_hidden", "GsSortSprite"),
        ("sprite_too_wide", "GsSortSprite"),
        ("sprite_too_tall_hidden", "GsSortSprite"),
        ("fast_unknown_bit", "GsSortFastSprite"),
        ("fast_zero_width_too_tall", "GsSortFastSprite"),
    ] {
        let out = child(case);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains(&format!("unimplemented SDK call `{function} with")),
            "{case} did not stop the program: {err}"
        );
    }
    for case in [
        "sprite_hidden",
        "sprite_zero_height",
        "fast_zero_width",
        "fast_hidden_big",
    ] {
        let out = child(case);
        assert!(
            out.status.success(),
            "{case} must draw nothing without stopping: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
