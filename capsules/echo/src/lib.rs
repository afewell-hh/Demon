use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn echo(s: String) -> String {
    s
}
