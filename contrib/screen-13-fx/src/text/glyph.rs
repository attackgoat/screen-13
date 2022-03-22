pub use bmfont::CharPosition as BitmapGlyph;

use screen_13::prelude_all::*;

pub trait Glyph {
    fn page_height(&self) -> u32;
    fn page_width(&self) -> u32;
    fn page_x(&self) -> u32;
    fn page_y(&self) -> u32;
    fn screen_height(&self) -> f32;
    fn screen_width(&self) -> f32;
    fn screen_x(&self) -> f32;
    fn screen_y(&self) -> f32;

    fn tessellate(&self) -> [[u8; 16]; 6] {
        let x1 = self.screen_x();
        let y1 = self.screen_y();
        let x2 = self.screen_x() + self.screen_width();
        let y2 = self.screen_y() + self.screen_height();

        let u1 = self.page_x() as f32;
        let u2 = (self.page_x() + self.page_width()) as f32;
        let v1 = self.page_y() as f32;
        let v2 = (self.page_y() + self.page_height()) as f32;

        let x1 = x1.to_ne_bytes();
        let x2 = x2.to_ne_bytes();
        let y1 = y1.to_ne_bytes();
        let y2 = y2.to_ne_bytes();
        let u1 = u1.to_ne_bytes();
        let u2 = u2.to_ne_bytes();
        let v1 = v1.to_ne_bytes();
        let v2 = v2.to_ne_bytes();

        let mut top_left = [0u8; 16];
        top_left[0..4].copy_from_slice(&x1);
        top_left[4..8].copy_from_slice(&y1);
        top_left[8..12].copy_from_slice(&u1);
        top_left[12..16].copy_from_slice(&v1);

        let mut bottom_right = [0u8; 16];
        bottom_right[0..4].copy_from_slice(&x2);
        bottom_right[4..8].copy_from_slice(&y2);
        bottom_right[8..12].copy_from_slice(&u2);
        bottom_right[12..16].copy_from_slice(&v2);

        let mut top_right = [0u8; 16];
        top_right[0..4].copy_from_slice(&x2);
        top_right[4..8].copy_from_slice(&y1);
        top_right[8..12].copy_from_slice(&u2);
        top_right[12..16].copy_from_slice(&v1);

        let mut bottom_left = [0u8; 16];
        bottom_left[0..4].copy_from_slice(&x1);
        bottom_left[4..8].copy_from_slice(&y2);
        bottom_left[8..12].copy_from_slice(&u1);
        bottom_left[12..16].copy_from_slice(&v2);

        [
            // First triangle
            top_left,
            bottom_right,
            top_right,

            // Second triangle
            top_left,
            bottom_left,
            bottom_right,
        ]
    }
}

impl Glyph for BitmapGlyph {
    #[inline(always)]
    fn page_height(&self) -> u32 {
        self.page_rect.height
    }

    #[inline(always)]
    fn page_width(&self) -> u32 {
        self.page_rect.width
    }

    #[inline(always)]
    fn page_x(&self) -> u32 {
        debug_assert!(self.page_rect.x >= 0);

        self.page_rect.x as _
    }

    #[inline(always)]
    fn page_y(&self) -> u32 {
        debug_assert!(self.page_rect.y >= 0);

        self.page_rect.y as _
    }

    #[inline(always)]
    fn screen_height(&self) -> f32 {
        self.screen_rect.height as _
    }

    #[inline(always)]
    fn screen_width(&self) -> f32 {
        self.screen_rect.width as _
    }

    #[inline(always)]
    fn screen_x(&self) -> f32 {
        self.screen_rect.x as _
    }

    #[inline(always)]
    fn screen_y(&self) -> f32 {
        self.screen_rect.y as _
    }
}

#[derive(Clone, Copy)]
pub struct VectorGlyph {
    pub page_idx: usize,
    pub page_rect: (IVec2, UVec2),
    pub screen_rect: (Vec2, Vec2),
}

impl Glyph for VectorGlyph {
    #[inline(always)]
    fn page_height(&self) -> u32 {
        self.page_rect.1.y
    }

    #[inline(always)]
    fn page_width(&self) -> u32 {
        self.page_rect.1.x
    }

    #[inline(always)]
    fn page_x(&self) -> u32 {
        debug_assert!(self.page_rect.0.x >= 0);

        self.page_rect.0.x as _
    }

    #[inline(always)]
    fn page_y(&self) -> u32 {
        debug_assert!(self.page_rect.0.y >= 0);

        self.page_rect.0.y as _
    }

    #[inline(always)]
    fn screen_height(&self) -> f32 {
        self.screen_rect.1.y
    }

    #[inline(always)]
    fn screen_width(&self) -> f32 {
        self.screen_rect.1.x
    }

    #[inline(always)]
    fn screen_x(&self) -> f32 {
        self.screen_rect.0.x
    }

    #[inline(always)]
    fn screen_y(&self) -> f32 {
        self.screen_rect.0.y
    }
}
