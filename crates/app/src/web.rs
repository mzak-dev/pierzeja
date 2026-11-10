//! Port przeglądarkowy (wasm32, GitHub Pages): zamienniki tego, czego w przeglądarce nie ma —
//! zmiennych środowiskowych (parametry adresu `?mode=`), zegara systemowego i zapisu plików
//! (eksport = pobranie przez Blob).

use wasm_bindgen::{JsCast, JsValue};

/// Parametr z adresu strony, np. `?mode=rondo`.
pub fn query(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == key).then(|| v.to_string())
    })
}

/// Milisekundy od epoki (std::time::SystemTime panikuje na wasm32-unknown-unknown).
pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

/// Pobranie pliku w przeglądarce.
pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    let err = |e: JsValue| format!("{e:?}");
    let doc = web_sys::window().and_then(|w| w.document()).ok_or("brak document")?;
    let arr = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&arr);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let a: web_sys::HtmlAnchorElement = doc.create_element("a").map_err(err)?.dyn_into().map_err(|_| "a")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    web_sys::Url::revoke_object_url(&url).map_err(err)?;
    Ok(())
}
