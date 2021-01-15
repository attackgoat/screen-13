//! The entrypoint in this file is what runs on the HTML page; it starts the engine.
#![deny(warnings)]

mod browser {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        pub fn alert(s: &str);
    }
}

use wasm_bindgen::prelude::*;

#[cfg(debug_assertions)]
use {
    console_error_panic_hook::hook, console_log::init_with_level, log::Level, std::panic::set_hook,
};

#[wasm_bindgen]
pub fn some_exported_function_name() {
    browser::alert("Hello, world!");
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    {
        // Without a panic hook you will never know about panics!
        set_hook(Box::new(hook));

        // This logs to the developer console of your browser
        init_with_level(Level::Trace).unwrap();
    }

    Ok(())
}
