use {
    super::{
        asset::{Bitmap as BitmapAsset, FontBitmap as FontBitmapAsset},
        get_filename_key, get_path,
    },
    crate::pak::{Bitmap, BitmapFormat, BitmapId, FontBitmap, FontBitmapId, PakBuf},
    bmfont::{BMFont, OrdinateOrientation},
    image::{buffer::ConvertBuffer, open as image_open, DynamicImage, RgbImage, RgbaImage},
    std::{fs::read, path::Path},
};

pub fn bake_bitmap<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    bitmap_asset: &BitmapAsset,
    pak: &mut PakBuf,
) -> BitmapId {
    let key = get_filename_key(&project_dir, &asset_filename);
    if let Some(id) = pak.id(&key) {
        return id.as_bitmap().unwrap();
    }

    info!("Processing asset: {}", key);

    // Get the fs objects for this asset
    let dir = asset_filename.as_ref().parent().unwrap();
    let bitmap_filename = get_path(&dir, bitmap_asset.src(), project_dir);

    // Bake the pixels
    let (fmt, width, pixels) = pixels(&bitmap_filename, bitmap_asset.force_opaque());
    let bitmap = Bitmap::new(fmt, width as u16, pixels);

    // Pak this asset
    pak.push_bitmap(key, bitmap)
}

pub fn bake_font_bitmap<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    font_bitmap_asset: &FontBitmapAsset,
    pak: &mut PakBuf,
) -> FontBitmapId {
    let key = get_filename_key(&project_dir, &asset_filename);
    if let Some(id) = pak.id(&key) {
        return id.as_font_bitmap().unwrap();
    }

    info!("Processing asset: {}", key);

    // Get the fs objects for this asset
    let dir = asset_filename.as_ref().parent().unwrap();
    let def_filename = get_path(&dir, font_bitmap_asset.src(), project_dir);
    let def_file = read(&def_filename).unwrap();
    let def_parent = def_filename.parent().unwrap();
    let def = BMFont::new(def_file.as_slice(), OrdinateOrientation::TopToBottom).unwrap();
    let pages = def
        .pages()
        .iter()
        .map(|page| {
            let page_filename = def_parent.join(page);

            // Bake the pixels
            let (_, width, pixels) = pixels(&page_filename, true);
            let mut better_pixels = Vec::with_capacity(pixels.len());
            for y in 0..width as usize {
                for x in 0..width as usize {
                    let g = pixels[y * width as usize * 3 + x * 3 + 1];
                    let r = pixels[y * width as usize * 3 + x * 3 + 2];
                    if 0xff == r {
                        better_pixels.push(0xff);
                        better_pixels.push(0x00);
                        better_pixels.push(0x00);
                    } else if 0xff == g {
                        better_pixels.push(0x00);
                        better_pixels.push(0xff);
                        better_pixels.push(0x00);
                    } else {
                        better_pixels.push(0x00);
                        better_pixels.push(0x00);
                        better_pixels.push(0x00);
                    }
                }
            }

            Bitmap::new(BitmapFormat::Rgb, width as u16, better_pixels)
        })
        .collect();

    // Pak this asset
    pak.push_font_bitmap(key, FontBitmap::new(def_file, pages))
}

fn pixels<P: AsRef<Path>>(filename: P, force_opaque: bool) -> (BitmapFormat, u32, Vec<u8>) {
    match image_open(&filename).unwrap() {
        DynamicImage::ImageRgb8(image) => (BitmapFormat::Rgb, image.width(), pixels_bgr(&image)),
        DynamicImage::ImageRgba8(image) => {
            if force_opaque {
                (
                    BitmapFormat::Rgb,
                    image.width(),
                    pixels_bgr(&image.convert()),
                )
            } else {
                (BitmapFormat::Rgba, image.width(), pixels_bgra(&image))
            }
        }
        _ => unimplemented!(),
    }
}

fn pixels_bgr(image: &RgbImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[2]);
            buf.push(pixel[1]);
            buf.push(pixel[0]);
        }
    }

    buf
}

fn pixels_bgra(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, image.height() - y - 1);
            buf.push(pixel[2]);
            buf.push(pixel[1]);
            buf.push(pixel[0]);
            buf.push(pixel[3]);
        }
    }

    buf
}
