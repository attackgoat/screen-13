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

    fn tessellate(&self) -> [u8; 96] {
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

        let mut res: [u8; 96] = [0; 96];

        // Top left (first triangle)
        res[0..4].copy_from_slice(&x1);
        res[4..8].copy_from_slice(&y1);
        res[8..12].copy_from_slice(&u1);
        res[12..16].copy_from_slice(&v1);

        // Bottom right
        res[16..20].copy_from_slice(&x2);
        res[20..24].copy_from_slice(&y2);
        res[24..28].copy_from_slice(&u2);
        res[28..32].copy_from_slice(&v2);

        // Top right
        res[32..36].copy_from_slice(&x2);
        res[36..40].copy_from_slice(&y1);
        res[40..44].copy_from_slice(&u2);
        res[44..48].copy_from_slice(&v1);

        // Top left (second triangle)
        res[48..52].copy_from_slice(&x1);
        res[52..56].copy_from_slice(&y1);
        res[56..60].copy_from_slice(&u1);
        res[60..64].copy_from_slice(&v1);

        // Bottom left
        res[64..68].copy_from_slice(&x1);
        res[68..72].copy_from_slice(&y2);
        res[72..76].copy_from_slice(&u1);
        res[76..80].copy_from_slice(&v2);

        // Bottom right
        res[80..84].copy_from_slice(&x2);
        res[84..88].copy_from_slice(&y2);
        res[88..92].copy_from_slice(&u2);
        res[92..96].copy_from_slice(&v2);

        res
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