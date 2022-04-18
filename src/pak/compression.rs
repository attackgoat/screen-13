use {
    brotli::{CompressorReader, CompressorWriter},
    serde::{Deserialize, Serialize},
    snap::{read::FrameDecoder, write::FrameEncoder},
    std::io::{Read, Write},
};

/// Describes Brotli-based compression.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct BrotliParams {
    /// Buffer size.
    pub buffer_size: usize,
    /// Compression quality.
    pub quality: u32,
    /// Window size.
    pub window_size: u32,
}

impl Default for BrotliParams {
    fn default() -> Self {
        Self {
            buffer_size: 4096,
            quality: 8,
            window_size: 22,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum Compression {
    Brotli(BrotliParams),
    Snap,
}

impl Compression {
    pub fn new_reader<'a>(self, reader: impl Read + 'a) -> Box<dyn Read + 'a> {
        match self {
            Compression::Brotli(b) => Box::new(CompressorReader::new(
                reader,
                b.buffer_size,
                b.quality,
                b.window_size,
            )),
            Compression::Snap => Box::new(FrameDecoder::new(reader)),
        }
    }

    pub fn new_writer<'a>(self, writer: impl Write + 'a) -> Box<dyn Write + 'a> {
        match self {
            Compression::Brotli(b) => Box::new(CompressorWriter::new(
                writer,
                b.buffer_size,
                b.quality,
                b.window_size,
            )),
            Compression::Snap => Box::new(FrameEncoder::new(writer)),
        }
    }
}

impl Default for Compression {
    fn default() -> Self {
        Self::Brotli(Default::default())
    }
}
