use {
    crate::pak::{BrotliCompression, Compression as PakCompression},
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
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

#[derive(Clone, Copy, Deserialize)]
pub enum Compression {
    #[serde(rename = "brotli")]
    Brotli,
    #[serde(rename = "snap")]
    Snap,
}

#[derive(Clone, Deserialize)]
pub struct Group {
    assets: Vec<PathBuf>,
    enabled: Option<bool>,
}

impl Group {
    pub fn assets(&self) -> impl Iterator<Item = &Path> {
        self.assets.iter().map(|asset| asset.as_path())
    }

    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}
