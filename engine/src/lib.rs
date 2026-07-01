use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn say_hello_from_rust() {
    let message = "Hello from Rust Wasm!";
    web_sys::console::log_1(&message.into());
}
