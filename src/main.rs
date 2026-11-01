#[cfg(not(target_arch = "wasm32"))]
fn main() {
    sheath::launch();
}

/// The browser's entry point: read the saved KDL files, then run the window. `wasm-bindgen`
/// exports it to the page, which awaits it after the module is ready.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn boot() {
    sheath::hydrate_web().await;
    sheath::launch();
}

/// A binary needs a `main` on every target; the page calls `boot` instead.
#[cfg(target_arch = "wasm32")]
fn main() {}
