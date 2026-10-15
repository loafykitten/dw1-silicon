//! No wobble (`backend::pgxp`) doesn't record the corners `GsSortSprite` projects for a scaled
//! sprite: the sprite is 2D, so the HD surfaces keep its integer corners and it stays aligned
//! with the unscaled sprites drawn as `SPRT`s next to it.

use psyq::libgs::{GsGetWorkBase, GsOt, GsSetWorkBase, GsSortSprite, GsSprite};
use psyq::memory::{LINK_END, ps1_ram_ptr};

#[test]
fn a_scaled_sprites_corners_are_not_recorded() {
    backend::pgxp::set_enabled(true);
    backend::pgxp::end_frame();
    // the projection distance the game sets, and a 3D point projected as a control
    psyq::gte::with(|g| g.ctc2(26, 512));
    backend::pgxp::record(200, 100, 200.25, 100.25, 512.0, 0);
    let s = GsSprite {
        attribute: 0,
        x: 40,
        y: 80,
        w: 8,
        h: 8,
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
        scalex: 8192,
        scaley: 8192,
        rotate: 0,
    };
    let corners;
    unsafe {
        let org = ps1_ram_ptr(0x801E_0300).cast::<u32>();
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
        GsSetWorkBase(ps1_ram_ptr(0x801E_0400));
        let before = GsGetWorkBase();
        GsSortSprite(&raw const s, &mut ot, 9);
        assert_ne!(GsGetWorkBase(), before, "the sprite was drawn");
        // the corners as the packet holds them (POLY_FT4 after the tag: words 2, 4, 6, 8)
        let pk = before.cast::<u32>();
        let xy = |k: usize| {
            let w = pk.add(k).read();
            (i32::from(w as i16), i32::from((w >> 16) as i16))
        };
        corners = [xy(2), xy(4), xy(6), xy(8)];
    }
    assert_eq!(corners[3].0 - corners[0].0, 16, "scaled 2x");
    assert_eq!(backend::pgxp::lookup_poly(&corners), [None; 4]);
    assert!(backend::pgxp::lookup_poly(&[(200, 100), corners[0], corners[3]])[0].is_some());
    backend::pgxp::set_enabled(false);
}
