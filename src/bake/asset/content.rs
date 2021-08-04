use {
    crate::pak::{BrotliCompression, Compression as PakCompression},
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of top-level content files which simply group other asset files for ease of
/// use.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Content {
    compression: Option<Compression>,

    // Brotli-specific compression fields
    buf_size: Option<usize>,
    quality: Option<u32>,
    window_size: Option<u32>,

    #[serde(rename = "group")]
    groups: Vec<Group>,
}

impl Content {
    /// An iterator of grouped content file descriptions.
    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter()
    }

    pub(crate) fn compression(&self) -> Option<PakCompression> {
        self.compression.map(|compression| match compression {
            Compression::Brotli => PakCompression::Brotli(BrotliCompression {
                buf_size: self
                    .buf_size
                    .unwrap_or_else(|| BrotliCompression::default().buf_size),
                quality: self
                    .quality
                    .unwrap_or_else(|| BrotliCompression::default().quality),
                window_size: self
                    .window_size
                    .unwrap_or_else(|| BrotliCompression::default().window_size),
            }),
            Compression::Snap => PakCompression::Snap,
        })
    }
}

#[derive(Clone, Copy,  Deserialize, Eq, Hash, PartialEq)]
pub enum Compression {
    /// Higher compression ratio but slower to decode and encode.
    #[serde(rename = "brotli")]
    Brotli,
    /// Lower compression ratio but faster to decode and encode.
    #[serde(rename = "snap")]
    Snap,
}

/// Holds a description of asset files.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Group {
    assets: Vec<PathBuf>,
    enabled: Option<bool>,
}

impl Group {
    /// Individual asset files.
    pub fn assets(&self) -> impl Iterator<Item = &Path> {
        self.assets.iter().map(|asset| asset.as_path())
    }

    /// Allows a group to be selectively removed with a single flag, as opposed to physically
    /// removing a group from the content file.
    ///
    /// This is useful for debugging.
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}
