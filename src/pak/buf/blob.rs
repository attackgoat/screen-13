use {
    super::{
        bitmap::Bitmap, file_key, Asset, BitmapBuf, BitmapFontBuf, BitmapFontId, Canonicalize, Id,
    },
    crate::pak::BitmapFormat,
    bmfont::{BMFont, OrdinateOrientation},
    log::info,
    serde::Deserialize,
    std::{
        collections::HashMap,
        fs::read_to_string,
        fs::File,
        io::{Cursor, Error, Read},
        path::{Path, PathBuf},
    },
};

#[cfg(feature = "bake")]
use super::Writer;

/// Holds a description of any generic file.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Blob {
    src: PathBuf,
}

impl Blob {
    /// Reads and processes bitmapped font source files into an existing `.pak` file buffer.
    #[allow(unused)]
    #[cfg(feature = "bake")]
    pub(super) fn bake_bitmap_font(
        context: &mut HashMap<Asset, Id>,
        writer: &mut Writer,
        project_dir: impl AsRef<Path>,
        src: impl AsRef<Path>,
        bitmap_font: Blob,
    ) -> Result<BitmapFontId, Error> {
        use crate::pak::BitmapColor;

        assert!(project_dir.as_ref().is_absolute());
        assert!(src.as_ref().is_absolute());

        let key = file_key(&project_dir, &src);
        let src = bitmap_font.src().to_owned();

        // Early-out if we have this asset in our context
        let context_key = Asset::BitmapFont(bitmap_font);
        if let Some(id) = context.get(&context_key) {
            return Ok(id.as_bitmap_font().unwrap());
        }

        info!("Baking bitmap font: {}", &key);

        // Get the fs objects for this asset
        let def_parent = src.parent().unwrap();
        let def_file = read_to_string(&src).unwrap();
        let def = BMFont::new(Cursor::new(&def_file), OrdinateOrientation::TopToBottom).unwrap();
        let pages = def
            .pages()
            .map(|page| {
                let path = def_parent.join(page);

                // Bake the pixels
                Bitmap::read_pixels(path, BitmapFormat::Rgb, None)
            })
            .filter(|res| res.is_ok()) // TODO: Horrible!
            .map(|res| res.unwrap())
            .map(|(width, pixels)| {
                let mut better_pixels = Vec::with_capacity(pixels.len());
                for y in 0..pixels.len() / 3 / width as usize {
                    for x in 0..width as usize {
                        let g = pixels[y * width as usize * 3 + x * 3 + 1];
                        let r = pixels[y * width as usize * 3 + x * 3];
                        if 0xff == r {
                            better_pixels.push(0xff);
                            better_pixels.push(0x00);
                        } else if 0xff == g {
                            better_pixels.push(0x00);
                            better_pixels.push(0xff);
                        } else {
                            better_pixels.push(0x00);
                            better_pixels.push(0x00);
                        }
                        better_pixels.push(0x00);
                    }
                }

                (width, better_pixels)
            })
            .collect::<Vec<_>>();

        // Panic if any page is a different size (the format says they should all be the same)
        let mut page_size = None;
        for (page_width, page_pixels) in &pages {
            let page_height = page_pixels.len() as u32 / 3 / page_width;
            if page_size.is_none() {
                page_size = Some((*page_width, page_height));
            } else if let Some((width, height)) = page_size {
                if *page_width != width || page_height != height {
                    panic!("Unexpected page size");
                }
            }
        }

        let (width, _) = page_size.unwrap();

        // In order to make drawing text easier, we optionally store the "pages" as one large texture
        // Each page is just appended to the bottom of the previous page making a tall bitmap
        let page_bufs = pages
            .into_iter()
            .map(|(_, pixels)| {
                BitmapBuf::new(BitmapColor::Linear, BitmapFormat::Rgb, width, pixels)
            })
            .collect();

        // Pak this asset and add it to the context
        let handle = writer.push_bitmap_font(BitmapFontBuf::new(def_file, page_bufs), Some(key));
        context.insert(context_key, handle.into());

        Ok(handle)
    }

    /// Reads and processes arbitrary binary source files into an existing `.pak` file buffer.
    #[allow(unused)]
    #[cfg(feature = "bake")]
    pub fn bake(writer: &mut Writer, project_dir: impl AsRef<Path>, path: impl AsRef<Path>) {
        let key = file_key(&project_dir, &path);

        info!("Baking blob: {}", key);

        let mut file = File::open(path).unwrap();
        let mut value = vec![];
        file.read_to_end(&mut value).unwrap();

        writer.push_blob(value, Some(key));
    }

    /// The file source.
    #[allow(unused)]
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

impl Canonicalize for Blob {
    fn canonicalize(&mut self, project_dir: impl AsRef<Path>, src_dir: impl AsRef<Path>) {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}
