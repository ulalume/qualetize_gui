//! The wasm entry point eframe's `WebRunner` is started from.

use crate::types::app_state::AppStateRequest;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Mount the app on `canvas`. An `image=<url>` query parameter on the page
/// is fetched and opened first. Returns once the app has been created; the
/// runner keeps itself alive through the browser's event loop after that.
#[wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    let initial = match image_url_from_query() {
        Some(url) => match fetch_bytes(&url).await {
            Ok(bytes) => Some(AppStateRequest::LoadImageBytes { name: url, bytes }),
            Err(e) => {
                log::error!("could not fetch {url}: {e:?}");
                None
            }
        },
        None => None,
    };

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |cc| Ok(Box::new(crate::app::QualetizeApp::new(cc, initial)))),
        )
        .await
}

/// The `image` query parameter of the page, if any.
fn image_url_from_query() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get("image").filter(|url| !url.is_empty())
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await?
        .dyn_into()?;
    if !response.ok() {
        return Err(JsValue::from_str(&format!("HTTP {}", response.status())));
    }
    let buffer = JsFuture::from(response.array_buffer()?).await?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
