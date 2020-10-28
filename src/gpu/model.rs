// TODO: Not all of these should come from super, remove from parent mod!
use {
    super::{BitmapId, BitmapRef, Data, Lease, Sphere, Texture2d},
    std::fmt::{Debug, Error, Formatter},
};

/// A textured and renderable model.
pub struct Model {
    bitmaps: Vec<(BitmapId, BitmapRef)>,
    bounds: Sphere,
    has_alpha: bool,
    vertex_buf: Lease<Data>,
    vertex_count: u32,
}

// TODO: Not sure about *anything* in this impl block. Maybe `textures`, that one is pretty cool.
impl Model {
    pub fn bounds(&self) -> Sphere {
        self.bounds
    }

    pub(crate) fn is_animated(&self) -> bool {
        // TODO: This needs to be implemented in some fashion - skys the limit here what should we do? hmmmm
        false
    }

    pub(crate) fn is_single_texture(&self) -> bool {
        self.bitmaps.len() == 1
    }

    pub(crate) fn textures(&self) -> impl Iterator<Item = &Texture2d> {
        Textures {
            bitmaps: &self.bitmaps,
            idx: 0,
        }
    }
}

impl Debug for Model {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Model")
    }
}

struct Textures<'a> {
    bitmaps: &'a Vec<(BitmapId, BitmapRef)>,
    idx: usize,
}

impl<'a> Iterator for Textures<'a> {
    type Item = &'a Texture2d;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.bitmaps.len() {
            Some(&self.bitmaps[self.idx].1)
        } else {
            None
        }
    }
}
