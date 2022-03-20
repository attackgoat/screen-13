use screen_13::prelude_all::*;

fn main() -> anyhow::Result<()> {
    // Set RUST_LOG=trace in your environment variables to see log output
    pretty_env_logger::init();

    PakBuf::bake("examples/res/fonts.toml", "target/debug/examples/fonts.pak")?;

    Ok(())

    /*
    Expected console output:

    cargo:rerun-if-changed=examples/res/fonts.toml
    cargo:rerun-if-changed=examples/res/font/cedarville_cursive/cedarville_cursive_regular.ttf
    cargo:rerun-if-changed=examples/res/font/rye/rye_regular.ttf
    INFO  screen_13::pak::buf::blob > Baking blob: font/cedarville_cursive/cedarville_cursive_regular.ttf
    cargo:rerun-if-changed=examples/res/font/small/small_10px.toml
    INFO  screen_13::pak::buf::blob > Baking blob: font/rye/rye_regular.ttf
    INFO  screen_13::pak::buf::blob > Baking bitmap font: font/small/small_10px
    TRACE screen_13::pak::buf::writer > Writing animations
    TRACE screen_13::pak::buf::writer > Writing bitmaps
    TRACE screen_13::pak::buf::writer > Writing blobs
    TRACE screen_13::pak::buf::writer > Index 0 = 63676 bytes
    TRACE screen_13::pak::buf::writer > Index 1 = 179196 bytes
    TRACE screen_13::pak::buf::writer > Writing bitmap fonts
    TRACE screen_13::pak::buf::writer > Index 0 = 23013 bytes
    TRACE screen_13::pak::buf::writer > Writing models
    TRACE screen_13::pak::buf::writer > Writing scenes
    */
}
