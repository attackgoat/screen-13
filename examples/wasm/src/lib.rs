use wasm_bindgen::prelude::*;

#[cfg(debug_assertions)]
use {
    console_error_panic_hook::hook, console_log::init_with_level, log::Level, std::panic::set_hook,
};

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn glue() {
    alert("Hello, world!");
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    {
        // Without a panic hook you will never know about panics!
        set_hook(Box::new(hook));

        init_with_level(Level::Trace);
    }

    panic!("BOOM");

    Ok(())
}
