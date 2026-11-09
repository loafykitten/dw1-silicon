//! Documented box blend modes, including the Techsheet's 0x60000000 subtractive boxes.
//! No retail data or emulator captures: packet bits follow libgs.h and psx-spx GP0(E1h).

use backend::gpu::{Gpu, VRAM_W};
use psyq::libgs::{GsBoxf, GsGetWorkBase, GsOt, GsSetWorkBase, GsSortBoxFill};
use psyq::memory::{LINK_END, link_of};

// Packet links must point into the executable's data segment, not the stack or host heap.
static mut PACKET: [u32; 6] = [0; 6];
static mut TABLE: [u32; 2] = [LINK_END; 2];

fn grey(level: u16) -> u16 {
    level | level << 5 | level << 10
}

#[test]
fn box_blend_modes_write_packets_and_blend_pixels() {
    // One test owns the shared libgs work base. Start with the exact crashing attribute.
    for rate in [2u32, 0, 1, 3] {
        for alpha in [true, false] {
            let attribute = rate << 28 | if alpha { 1 << 30 } else { 0 };
            let b = GsBoxf {
                attribute,
                x: 30,
                y: 8,
                w: 12,
                h: 12,
                r: 104,
                g: 104,
                b: 104,
            };
            let words = unsafe {
                PACKET = [0xDEAD_BEEF; 6];
                TABLE = [LINK_END, 0xA000_0000 | LINK_END];
                let packet = (&raw mut PACKET).cast::<u32>();
                let table = (&raw mut TABLE).cast::<u32>();
                let mut ot = GsOt {
                    length: 1,
                    org: table,
                    offset: 0,
                    point: 0,
                    tag: table.add(1),
                };
                GsSetWorkBase(packet.cast());
                GsSortBoxFill(&b, &mut ot, 1);
                let words = std::ptr::read(&raw const PACKET);
                assert_eq!(words[0], 0x0400_0000 | LINK_END);
                assert_eq!(words[1], 0xE100_0200 | rate << 5);
                assert_eq!(words[2], if alpha { 0x6268_6868 } else { 0x6068_6868 });
                assert_eq!(words[3], 8 << 16 | 30);
                assert_eq!(words[4], 12 << 16 | 12);
                assert_eq!(words[5], 0xDEAD_BEEF, "packet overrun");
                assert_eq!(table.read(), LINK_END, "other OT entries stay unchanged");
                assert_eq!(table.add(1).read(), 0xA000_0000 | link_of(packet.cast()));
                assert_eq!(GsGetWorkBase(), packet.add(5).cast());
                words
            };

            let mut gpu = Gpu::new();
            gpu.vram_mut().fill(grey(20));
            assert!(gpu.gp0(&words[1..5]));
            // B=20, F=104/8=13: half, add (clamped), subtract, quarter-add.
            let expected = if alpha {
                [16, 31, 7, 23][rate as usize]
            } else {
                13
            };
            for y in 8..20 {
                for x in 30..42 {
                    assert_eq!(gpu.vram()[y * VRAM_W + x], grey(expected), "{attribute:#x}");
                }
            }
            assert_eq!(gpu.vram()[8 * VRAM_W + 29], grey(20));
            assert_eq!(gpu.vram()[20 * VRAM_W + 30], grey(20));
        }
    }
}
