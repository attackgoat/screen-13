#[rustfmt::skip]
mod point_light {
    include!(concat!(env!("OUT_DIR"), "/point_light.rs"));
}

#[rustfmt::skip]
mod spotlight {
    include!(concat!(env!("OUT_DIR"), "/spotlight.rs"));
}

pub use self::{
    point_light::{POINT_LIGHT, POINT_LIGHT_DRAW_COUNT},
    spotlight::{gen_spotlight, SPOTLIGHT_STRIDE},
};

use {
    super::command::LineVertex,
    crate::math::{Coord8, CoordF},
    std::ops::Range,
};

// TODO: use genmesh::{LruIndexer, Triangulate} (both here and in the bake::mesh code!! switch to a more efficient drawing mode!!)
// TODO: Use https://doc.rust-lang.org/std/primitive.slice.html#method.get_unchecked_mut

pub const LINE_STRIDE: usize = 64;
pub const RECT_LIGHT_STRIDE: usize = 144;

/// Produces the vertices of a given line definition.
pub(super) fn gen_line(vertices: &[LineVertex; 2]) -> [u8; LINE_STRIDE] {
    let mut res = [0; LINE_STRIDE];

    res[0..4].copy_from_slice(&vertices[0].pos.x.to_ne_bytes());
    res[4..8].copy_from_slice(&vertices[0].pos.y.to_ne_bytes());
    res[8..12].copy_from_slice(&vertices[0].pos.z.to_ne_bytes());

    let color = vertices[0].color.to_rgba();
    res[12..16].copy_from_slice(&color.x.to_ne_bytes());
    res[16..20].copy_from_slice(&color.y.to_ne_bytes());
    res[20..24].copy_from_slice(&color.z.to_ne_bytes());
    res[24..28].copy_from_slice(&color.w.to_ne_bytes());

    res[32..36].copy_from_slice(&vertices[1].pos.x.to_ne_bytes());
    res[36..40].copy_from_slice(&vertices[1].pos.y.to_ne_bytes());
    res[40..44].copy_from_slice(&vertices[1].pos.z.to_ne_bytes());

    let color = vertices[1].color.to_rgba();
    res[44..48].copy_from_slice(&color.x.to_ne_bytes());
    res[48..52].copy_from_slice(&color.y.to_ne_bytes());
    res[52..56].copy_from_slice(&color.z.to_ne_bytes());
    res[56..60].copy_from_slice(&color.w.to_ne_bytes());

    res
}

/// Produces the vertices of a given rectangular light definition, which form a truncated pyramid. The
/// resulting mesh will be normalized and requires an additional scale factor to render as intended.
/// The final location will be (0,0,0) at the center of the top quad, and the orientation will point (0,-1,0).
pub(super) fn gen_rect_light(dims: Coord8, range: u8, radius: u8) -> [u8; RECT_LIGHT_STRIDE] {
    let mut res = [0; RECT_LIGHT_STRIDE];
    let radius = radius as f32;
    let range = range as f32 / -2.0;
    let p = CoordF::from(dims) / 2.0;
    let n = -p;

    // The quads below look like:
    //  a---b
    //  | / |
    //  c---d
    //
    // Triangle a-b-c:
    //  a--b
    //  | /
    //  c
    //
    // Triangle c-b-d:
    //     b
    //   / |
    //  c--d

    // Top quad (y is zero, so not written out because the zero bit pattern is also a 0f32)
    {
        // Triangle a-b-c
        {
            // Index 0 (a)
            res[x_range(0)].copy_from_slice(&n.x.to_ne_bytes());
            res[z_range(0)].copy_from_slice(&p.y.to_ne_bytes());

            // Index 1 (b)
            res[x_range(1)].copy_from_slice(&p.x.to_ne_bytes());
            res.copy_within(z_range(0), z_start(1));

            // Index 2 (c)
            res.copy_within(x_range(0), x_start(2));
            res[z_range(2)].copy_from_slice(&n.y.to_ne_bytes());
        }

        // Triangle c-b-d
        {
            // Index 3 (c)
            res.copy_within(v_range(2), x_start(3));

            // Index 4 (b)
            res.copy_within(v_range(1), x_start(4));

            // Index 5 (d)
            res.copy_within(x_range(1), x_start(5));
            res.copy_within(z_range(2), z_start(5));
        }
    }

    // Front quad (a/b matches c/d on the top)
    {
        // Triangle a-b-c
        {
            // Index 6 (a)
            res.copy_within(v_range(2), x_start(6));

            // Index 7 (b)
            res.copy_within(v_range(5), x_start(7));

            // Index 8 (c)
            res[x_range(8)].copy_from_slice(&(n.x - radius).to_ne_bytes());
            res[y_range(8)].copy_from_slice(&range.to_ne_bytes());
            res[z_range(8)].copy_from_slice(&(n.y - radius).to_ne_bytes());
        }

        // Triangle c-b-d
        {
            // Index 9 (c)
            res.copy_within(v_range(8), x_start(9));

            // Index 10 (b)
            res.copy_within(v_range(5), x_start(10));

            // Index 11 (d)
            res[x_range(11)].copy_from_slice(&(p.x + radius).to_ne_bytes());
            res.copy_within(yz_range(8), y_start(11));
        }
    }

    // Back quad (a/b matches b/a on the top)
    {
        // Triangle a-b-c
        {
            // Index 12 (a)
            res.copy_within(v_range(1), x_start(12));

            // Index 13 (b)
            res.copy_within(v_range(0), x_start(13));

            // Index 14 (c)
            res.copy_within(xy_range(8), x_start(14));
            res[z_range(14)].copy_from_slice(&(p.y + radius).to_ne_bytes());
        }

        // Triangle c-b-d
        {
            // Index 15 (c)
            res.copy_within(v_range(14), x_start(9));

            // Index 16 (b)
            res.copy_within(v_range(13), x_start(16));

            // Index 17 (d)
            res.copy_within(x_range(11), x_start(17));
            res.copy_within(yz_range(14), y_start(17));
        }
    }

    // Left quad (a/b matches a/c on the top)
    {
        // Triangle a-b-c
        {
            // Index 18 (a)
            res.copy_within(v_range(0), x_start(18));

            // Index 19 (b)
            res.copy_within(v_range(2), x_start(19));

            // Index 20 (c)
            res.copy_within(v_range(14), x_start(20));
        }

        // Triangle c-b-d
        {
            // Index 21 (c)
            res.copy_within(v_range(14), x_start(21));

            // Index 22 (b)
            res.copy_within(v_range(2), x_start(22));

            // Index 23 (d)
            res.copy_within(v_range(8), x_start(23));
        }
    }

    // Right quad (a/b matches d/b on the top)
    {
        // Triangle a-b-c
        {
            // Index 24 (a)
            res.copy_within(v_range(5), x_start(24));

            // Index 25 (b)
            res.copy_within(v_range(1), x_start(25));

            // Index 26 (c)
            res.copy_within(v_range(11), x_start(26));
        }

        // Triangle c-b-d
        {
            // Index 27 (c)
            res.copy_within(v_range(11), x_start(27));

            // Index 28 (b)
            res.copy_within(v_range(1), x_start(28));

            // Index 29 (d)
            res.copy_within(v_range(14), x_start(29));
        }
    }

    // Bottom quad (a/b matches c/d on the front)
    {
        // Triangle a-b-c
        {
            // Index 30 (a)
            res.copy_within(v_range(8), x_start(30));

            // Index 31 (b)
            res.copy_within(v_range(11), x_start(31));

            // Index 32 (c)
            res.copy_within(v_range(17), x_start(32));
        }

        // Triangle c-b-d
        {
            // Index 33 (c)
            res.copy_within(v_range(17), x_start(33));

            // Index 34 (b)
            res.copy_within(v_range(11), x_start(34));

            // Index 35 (d)
            res.copy_within(v_range(14), x_start(35));
        }
    }

    res
}

/// Returns the range of a given vertex
const fn v_range(idx: usize) -> Range<usize> {
    let start = x_start(idx);

    start..start + 12
}

/// Returns the range of the "x" field of a given vertex
const fn x_range(idx: usize) -> Range<usize> {
    let start = x_start(idx);

    start..start + 4
}

/// Returns the range of the "x" and "y" fields of a given vertex
const fn xy_range(idx: usize) -> Range<usize> {
    let start = x_start(idx);

    start..start + 8
}

/// Returns the start of the "x" field of a given vertex
const fn x_start(idx: usize) -> usize {
    idx * 12
}

/// Returns the range of the "y" field of a given vertex
const fn y_range(idx: usize) -> Range<usize> {
    let start = y_start(idx);

    start..start + 4
}

/// Returns the start of the "y" field of a given vertex
const fn y_start(idx: usize) -> usize {
    x_start(idx) + 4
}

/// Returns the range of the "y" and "z" fields of a given vertex
const fn yz_range(idx: usize) -> Range<usize> {
    let start = y_start(idx);

    start..start + 8
}

/// Returns the range of the "z" field of a given vertex
const fn z_range(idx: usize) -> Range<usize> {
    let start = z_start(idx);

    start..start + 4
}

/// Returns the start of the "z" field of a given vertex
const fn z_start(idx: usize) -> usize {
    x_start(idx) + 8
}
