//! Clawku: the host page bridge. Opt-in: only a page that loads the app framed with
//! `?host=parent&open=host` (Clawku) is hosted ([`hosted`]); every other page keeps the browser's
//! own pickers and the app's own sharing.
//!
//! - File › Open asks the host for a file from its own storage instead of opening the browser's
//!   file picker: `<app>:pick` (`{ type, purpose }`) on the same-origin parent. The host answers
//!   with the usual `<app>:open` message, or not at all.
//! - Share asks the host to share the document: `<app>:share` (`{ type }`).

use wasm_bindgen::JsValue;

/// Whether this page is framed by a host that asked for it (`?host=parent&open=host`).
pub fn hosted() -> bool {
    host_parent().is_some()
}

/// The host page when this page is hosted (see [`hosted`]), with its origin (the page's own).
fn host_parent() -> Option<(web_sys::Window, String)> {
    let win = web_sys::window()?;
    let search = win.location().search().unwrap_or_default();
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    if params.get("host").as_deref() != Some("parent") || params.get("open").as_deref() != Some("host") {
        return None;
    }
    let parent = win.parent().ok()??;
    if js_sys::Object::is(&parent, &win) {
        return None; // not framed
    }
    let origin = win.location().origin().ok()?;
    Some((parent, origin))
}

/// Post `{ type: "<app>:<kind>", ...fields }` to the host → whether it was posted.
fn post(app: &str, kind: &str, fields: &[(&str, &str)]) -> bool {
    let Some((parent, origin)) = host_parent() else { return false };
    let msg = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&msg, &JsValue::from_str("type"), &JsValue::from_str(&format!("{app}:{kind}")));
    for (k, v) in fields {
        let _ = js_sys::Reflect::set(&msg, &JsValue::from_str(k), &JsValue::from_str(v));
    }
    parent.post_message(&msg, &origin).is_ok()
}

/// Ask the host for a file to open; `false` when this page isn't hosted that way (use the picker).
pub fn host_pick(app: &str, purpose: &str) -> bool {
    post(app, "pick", &[("purpose", purpose)])
}

/// Ask the host to share the document; `false` when this page isn't hosted.
pub fn host_share(app: &str) -> bool {
    post(app, "share", &[])
}
