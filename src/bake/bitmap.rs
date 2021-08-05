use {
    super::{
        asset::{Asset, Bitmap, Blob},
        get_filename_key,
    },
    crate::pak::{
        id::{BitmapFontId, BitmapId, Id},
        BitmapBuf, BitmapFont, BitmapFormat, PakBuf,
    },
    bmfont::{BMFont, OrdinateOrientation},
    image::{buffer::ConvertBuffer, open as image_open, DynamicImage, RgbaImage},
    std::{
        collections::HashMap,
        fs::read_to_string,
        io::Cursor,
        path::{Path, PathBuf},
    },
};

/// Reads and processes image source files into an existing `.pak` file buffer.
pub fn bake_bitmap<P1, P2>(
    context: &mut HashMap<Asset, Id>,
    pak: &mut PakBuf,
    project_dir: P1,
    res_dir: P2,
    bitmap: &Bitmap,
) -> BitmapId
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    // let key = get_filename_key(&project_dir, &asset_filename);
    // if let Some(id) = pak.id(&key) {
    //     return id.as_bitmap().unwrap();
    // }

    // info!("Baking bitmap: {}", key);

    // Get the fs objects for this asset
    // let dir = asset_filename.as_ref().parent().unwrap();
    // let bitmap_filename = get_path(&dir, bitmap_asset.src(), project_dir);

    // Bake the pixels
    let (width, pixels) = pixels(bitmap.src(), bitmap.format());
    let buf = BitmapBuf::new(bitmap.format(), width as u16, pixels);

    // Pak this asset
    pak.push_bitmap(None, buf)
}

/// Reads and processes image source files into an existing `.pak` file buffer.
pub fn bake_bitmap_path<P1: AsRef<Path>, P2: AsRef<Path>>(
    context: &mut HashMap<Asset, Id>,
    pak: &mut PakBuf,
    project_dir: P1,
    path: P2,
    asset: &Bitmap,
) -> BitmapId {
    // let key = get_filename_key(&project_dir, &asset_filename);
    // if let Some(id) = pak.id(&key) {
    //     return id.as_bitmap().unwrap();
    // }

    // info!("Baking bitmap: {}", key);

    // Get the fs objects for this asset
    // let dir = asset_filename.as_ref().parent().unwrap();
    // let bitmap_filename = get_path(&dir, bitmap_asset.src(), project_dir);

    // // Bake the pixels
    // let (width, pixels) = pixels(&bitmap_filename, bitmap_asset.format());
    // let bitmap = BitmapBuf::new(bitmap_asset.format(), width as u16, pixels);

    // Pak this asset
    // pak.push_bitmap(key, bitmap)
    todo!();
}

/// Reads and processes bitmapped font source files into an existing `.pak` file buffer.
pub fn bake_bitmap_font<P1: AsRef<Path>, P2: AsRef<Path>>(
    context: &mut HashMap<Asset, Id>,
    pak: &mut PakBuf,
    project_dir: P1,
    src: P2,
    bitmap_font_asset: &Blob,
) -> BitmapFontId {
    // let key = get_filename_key(&project_dir, &asset_filename);
    // if let Some(id) = pak.id(&key) {
    //     return id.as_bitmap_font().unwrap();
    // }

    // info!("Baking bitmap font: {}", key);

    // Get the fs objects for this asset
    let src_dir = src.as_ref().parent().unwrap();
    let def_filename = bitmap_font_asset.src(); // TODO get_path(&dir, bitmap_font_asset.src(), project_dir);
    let def_file = read_to_string(&def_filename).unwrap();
    let def_parent = def_filename.parent().unwrap();
    let def = BMFont::new(Cursor::new(&def_file), OrdinateOrientation::TopToBottom).unwrap();
    let pages = def
        .pages()
        .map(|page| {
            let page_filename = def_parent.join(page);

            // Bake the pixels
            let (width, pixels) = pixels(&page_filename, BitmapFormat::Rgb);
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
    // let page_bufs = pages
    //     .into_iter()
    //     .map(|(_, pixels)| BitmapBuf::new(BitmapFormat::Rgb, width as u16, pixels))
    //     .collect();

    // Pak this asset
    // pak.push_bitmap_font(key, BitmapFont::new(def_file, page_bufs))
    todo!();
}

/// Reads raw pixel data from an image source file and returns them in the given format.
pub fn pixels<P: AsRef<Path>>(filename: P, fmt: BitmapFormat) -> (u32, Vec<u8>) {
    let image = match image_open(&filename).unwrap() {
        DynamicImage::ImageRgb8(image) => image.convert(),
        DynamicImage::ImageRgba8(image) => image,
        _ => unimplemented!(),
    };
    let width = image.width();
    let data = match fmt {
        BitmapFormat::R => pixels_r(&image),
        BitmapFormat::Rg => pixels_rg(&image),
        BitmapFormat::Rgb => pixels_rgb(&image),
        BitmapFormat::Rgba => pixels_rgba(&image),
    };

    (width, data)
}

fn pixels_r(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[0]);
        }
    }

    buf
}

fn pixels_rg(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 2);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[0]);
            buf.push(pixel[1]);
        }
    }

    buf
}

fn pixels_rgb(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[0]);
            buf.push(pixel[1]);
            buf.push(pixel[2]);
        }
    }

    buf
}

fn pixels_rgba(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[0]);
            buf.push(pixel[1]);
            buf.push(pixel[2]);
            buf.push(pixel[3]);
        }
    }

    buf
}
