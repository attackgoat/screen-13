use serde::Deserialize;

/// Holds a description of top-level content files which simply group other asset files for ease of
/// use.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Content {
    compression: Option<CompressionType>,

    // Brotli-specific compression parameters
    buffer_size: Option<usize>,
    quality: Option<u32>,
    window_size: Option<u32>,

    #[serde(rename = "group")]
    groups: Vec<Group>,
}

impl Content {
    /// An iterator of grouped content file descriptions.
    #[allow(unused)]
    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter()
    }

    // pub(crate) fn compression(&self) -> Option<Compression> {
    //     self.compression.map(|compression| match compression {
    //         CompressionType::Brotli => Compression::Brotli(BrotliParams {
    //             buffer_size: self
    //                 .buf_size
    //                 .unwrap_or_else(|| BrotliParams::default().buf_size),
    //             quality: self
    //                 .quality
    //                 .unwrap_or_else(|| BrotliParams::default().quality),
    //             window_size: self
    //                 .window_size
    //                 .unwrap_or_else(|| BrotliParams::default().window_size),
    //         }),
    //         CompressionType::Snap => Compression::Snap,
    //     })
    // }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum CompressionType {
    /// Higher compression ratio but slower to decode and encode.
    #[serde(rename = "brotli")]
    Brotli,
    /// Lower compression ratio but faster to decode and encode.
    #[serde(rename = "snap")]
    Snap,
}

/// Holds a description of asset files.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Group {
    assets: Vec<String>,
    enabled: Option<bool>,
}

impl Group {
    /// Individual asset file specification globs.
    ///
    /// May be a filename, might be folder/**/other.jpeg
    #[allow(unused)]
    pub fn asset_globs(&self) -> impl Iterator<Item = &String> {
        self.assets.iter()
    }

    /// Allows a group to be selectively removed with a single flag, as opposed to physically
    /// removing a group from the content file.
    ///
    /// This is useful for debugging.
    #[allow(unused)]
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}
