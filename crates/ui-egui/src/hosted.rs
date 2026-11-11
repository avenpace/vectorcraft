//! Clawku hosting: the web app framed by Clawku (`?host=parent&open=host`, [`Services::hosted`]).
//!
//! Clawku owns the file list, New, Open, Close, Rename, sharing, versions, sign-in and help, and
//! keeps versions of every file itself. So while hosted the app shows the one document Clawku sent
//! and leaves out (hides, not dims) every way to make, list, switch or close documents, the Home
//! screen, Data Recovery, its standalone links and its local-folder settings; Share goes to the
//! host. Nothing here changes the desktop app or the plain web build.
//!
//! [`Services::hosted`]: crate::Services::hosted

use crate::VectorcraftApp;

/// Commands left out of the menus, the command palette and their shortcuts while hosted.
pub const HIDDEN_COMMANDS: &[&str] = &[
    // Other documents: Clawku makes, lists and closes them.
    "file.newDialog",
    "file.newWithoutDialog",
    "file.newFromTemplate",
    "file.clearRecent",
    "file.reveal",
    "file.close",
    "file.closeAll",
    // Clawku keeps the versions: there is no file of the app's own to go back to.
    "file.revert",
    "file.saveAsTemplate",
    "app.quit",
    "app.home",
    "window.newWindow",
    "window.nextDocument",
    "window.previousDocument",
    "window.arrange.cascade",
    "window.arrange.tile",
    "window.arrange.floatInWindow",
    "window.arrange.floatAllInWindows",
    "window.arrange.consolidateAllWindows",
    // The standalone app's community, source and updates.
    "help.discord",
    "help.github",
    "help.checkForUpdates",
    // Libraries and shortcut sets saved as loose files, and plug-ins read from a folder.
    "ui.saveSwatchLibrary",
    "ui.saveGraphicStyleLibrary",
    "ui.saveSymbolLibrary",
    "ui.saveBrushLibrary",
    "shortcuts.export",
    "plugin.reload",
];

/// Commands shown only while hosted (they need the host).
pub const HOSTED_ONLY_COMMANDS: &[&str] = &[SHARE];

/// File › Share…: hands sharing to the host page.
pub const SHARE: &str = "file.share";

/// Submenus left out while hosted, by their tree label: the recent files list and the arrangement
/// of several document windows.
pub const HIDDEN_SUBMENUS: &[&str] = &["Open Recent Files", "Arrange"];

/// Items not implemented yet left out while hosted, by their tree label: Help points at Clawku.
pub const HIDDEN_TODOS: &[&str] = &["ClawVector Help…"];

/// Preferences left out while hosted: Data Recovery (Clawku keeps versions), the Home screen and
/// the document tabs (one document, no tabs).
pub const HOSTED_HIDDEN_PREFS: &[&str] =
    &["autosaveRecovery", "autosaveInterval", "recoveryOffForComplex", "showHomeScreen", "openDocumentsAsTabs", "largeTabs"];

/// Preferences left out on the web (hosted or not): folders on this machine, which the browser
/// has none of, and the recent files list, which the browser never fills.
pub const WEB_HIDDEN_PREFS: &[&str] =
    &["fontsFolder", "pluginsFolder", "scratchPrimary", "scratchSecondary", "recoveryFolder", "templatesFolder", "recentFilesCount"];

/// Whether the app runs framed by Clawku.
pub fn hosted(app: &VectorcraftApp) -> bool {
    app.services.hosted
}

/// Whether command `id` is left out now: one of [`HIDDEN_COMMANDS`] (or a recent file slot) while
/// hosted, or a [`HOSTED_ONLY_COMMANDS`] one while not.
pub fn hides(app: &VectorcraftApp, id: &str) -> bool {
    if hosted(app) { HIDDEN_COMMANDS.contains(&id) || id.starts_with("file.openRecent") } else { HOSTED_ONLY_COMMANDS.contains(&id) }
}

/// Whether the submenu labelled `label` (the tree's English label) is left out now.
pub fn hides_submenu(app: &VectorcraftApp, label: &str) -> bool {
    hosted(app) && HIDDEN_SUBMENUS.contains(&label)
}

/// Whether the not-yet-implemented item labelled `label` is left out now.
pub fn hides_todo(app: &VectorcraftApp, label: &str) -> bool {
    hosted(app) && HIDDEN_TODOS.contains(&label)
}

/// Whether Preferences leaves out the preference `key` now.
pub fn hides_pref(app: &VectorcraftApp, key: &str) -> bool {
    (hosted(app) && HOSTED_HIDDEN_PREFS.contains(&key)) || (crate::io::is_web(app) && WEB_HIDDEN_PREFS.contains(&key))
}

/// Share… while hosted: the host page shows its own sharing → whether it took it.
pub fn share(app: &mut VectorcraftApp) -> Result<serde_json::Value, String> {
    if app.services.host_share.as_mut().is_some_and(|f| f()) {
        Ok(serde_json::json!({"shared": true}))
    } else {
        Err("Share is not available here".into())
    }
}

/// The name Save a Copy writes while hosted: `name`'s stem with ` (copy)` (once) and its
/// extension, so Clawku keeps it as a copy next to the document instead of taking it for the
/// document's new file, which a native file under another name is (Save As).
pub fn copy_name(name: &str) -> String {
    let path = std::path::Path::new(name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = stem.strip_suffix(" copy").unwrap_or(&stem);
    let stem = if stem.ends_with(" (copy)") { stem.to_string() } else { format!("{stem} (copy)") };
    let file = match path.extension() {
        Some(ext) => format!("{stem}.{}", ext.to_string_lossy()),
        None => stem,
    };
    match path.parent().map(|p| p.to_string_lossy().into_owned()).filter(|p| !p.is_empty()) {
        Some(folder) => format!("{folder}/{file}"),
        None => file,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_names_carry_one_copy_marker() {
        assert_eq!(copy_name("Poster copy.vectorcraft"), "Poster (copy).vectorcraft");
        assert_eq!(copy_name("Poster.vectorcraft"), "Poster (copy).vectorcraft");
        assert_eq!(copy_name("Poster (copy).vectorcraft"), "Poster (copy).vectorcraft");
        assert_eq!(copy_name("logo.svg"), "logo (copy).svg");
        assert_eq!(copy_name("noext"), "noext (copy)");
    }
}
