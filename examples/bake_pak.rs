use screen_13::prelude_all::*;

fn main() -> anyhow::Result<()> {
    // Set RUST_LOG=trace in your environment variables to see log output
    pretty_env_logger::init();

    PakBuf::bake("examples/shader-toy/res/pak.toml", "target/dont_care.pak")?;

    Ok(())

    /*
    Expected console output:

    cargo:rerun-if-changed=examples/shader-toy/res/pak.toml
    cargo:rerun-if-changed=examples/shader-toy/res/image/flowers.jpg
    INFO  screen_13::pak::buf::bitmap > Baking bitmap: image/flowers.jpg
    cargo:rerun-if-changed=examples/shader-toy/res/image/rgba_noise.png
    INFO  screen_13::pak::buf::bitmap > Baking bitmap: image/rgba_noise.png
    TRACE screen_13::pak::buf::writer > Writing animations
    TRACE screen_13::pak::buf::writer > Writing bitmaps
    TRACE screen_13::pak::buf::writer > Index 0 = 2359316 bytes
    TRACE screen_13::pak::buf::writer > Index 1 = 262164 bytes
    TRACE screen_13::pak::buf::writer > Writing blobs
    TRACE screen_13::pak::buf::writer > Writing bitmap fonts
    TRACE screen_13::pak::buf::writer > Writing models
    TRACE screen_13::pak::buf::writer > Writing scenes
    */
}
