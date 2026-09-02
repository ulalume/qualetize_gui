//! The wasm entry point eframe's `WebRunner` is started from.

use wasm_bindgen::prelude::*;

/// Mount the app on `canvas`. Returns once the app has been created; the
/// runner keeps itself alive through the browser's event loop after that.
#[wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(Box::new(crate::app::QualetizeApp::new(cc, None)))),
        )
        .await
}
