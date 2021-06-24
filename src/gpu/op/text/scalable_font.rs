use {
    crate::{gpu::pool::Pool, pak::Pak},
    archery::SharedPointerKind,
    fontdue::Font,
    std::{
        fmt::{Debug, Error, Formatter},
        io::{Read, Seek},
    },
};

/// Holds a decoded font.
pub struct ScalableFont {
    font: Font,
    //page: TODO!
}

impl ScalableFont {
    pub(crate) fn read<K, P, R>(_pool: &mut Pool<P>, pak: &mut Pak<R>, key: K) -> Self
    where
        K: AsRef<str>,
        P: SharedPointerKind,
        R: Read + Seek,
    {
        let id = pak.font_id(key).unwrap();
        let font = pak.read_font(id);

        ScalableFont { font }
    }
}

impl Debug for ScalableFont {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("ScalableFont")
    }
}

impl From<Font> for ScalableFont {
    fn from(font: Font) -> Self {
        ScalableFont { font }
    }
}
