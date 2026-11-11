//! Hosted by Clawku (`Services::hosted`): one document, no Home screen, no New / Close / Recent,
//! Share goes to the host, Save keeps the document's name, Save a Copy is marked as a copy and
//! there is no Help at all (menu, About, palette entries).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::fileio::SaveMode;

use crate::{Services, VectorcraftApp};

type Saved = Rc<RefCell<Vec<String>>>;

/// A web app (saves download into `saved`), hosted or not; `shares` counts Share requests.
fn web_app(hosted: bool, saved: &Saved, shares: &Rc<Cell<u32>>) -> VectorcraftApp {
    let s = saved.clone();
    let n = shares.clone();
    let services = Services {
        download: Some(Box::new(move |name: &str, _: &[u8]| {
            s.borrow_mut().push(name.to_string());
            Ok(())
        })),
        hosted,
        host_share: hosted.then(|| {
            Box::new(move || {
                n.set(n.get() + 1);
                true
            }) as Box<dyn FnMut() -> bool>
        }),
        ..Default::default()
    };
    VectorcraftApp::new(Session::new(), services)
}

fn menu_commands(app: &VectorcraftApp) -> Vec<String> {
    crate::menus::menu_entries(app).into_iter().filter_map(|e| e.command).collect()
}

fn menu_labels(app: &VectorcraftApp) -> Vec<String> {
    crate::menus::menu_entries(app).into_iter().map(|e| e.label).collect()
}

#[test]
fn hosted_menus_leave_out_other_documents_and_standalone_extras() {
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let plain = web_app(false, &saved, &shares);
    let hosted = web_app(true, &saved, &shares);
    let (plain_cmds, hosted_cmds) = (menu_commands(&plain), menu_commands(&hosted));
    for id in crate::hosted::HIDDEN_COMMANDS.iter().filter(|id| plain_cmds.iter().any(|c| c == *id)) {
        assert!(!hosted_cmds.iter().any(|c| c == id), "{id} is left out while hosted");
    }
    for id in ["file.newDialog", "file.close", "file.closeAll", "file.revert", "app.quit", "help.discord", "help.github", "help.checkForUpdates"] {
        assert!(plain_cmds.iter().any(|c| c == id), "{id} stays in the plain web app");
        assert!(!crate::menus::enabled(&hosted, id), "{id} is off on its keys too");
    }
    assert!(!hosted_cmds.iter().any(|c| c.starts_with("file.openRecent")), "no recent files");
    assert!(!hosted_cmds.iter().any(|c| c.starts_with("window.arrange")), "no window arrangement");
    assert!(!menu_labels(&hosted).iter().any(|l| l == "ClawVector Help…"));
    // Open, Save and Print stay; Share is there only while hosted.
    for id in ["file.open", "file.save", "file.saveAs", "file.saveCopy", "file.print"] {
        assert!(hosted_cmds.iter().any(|c| c == id), "{id} stays while hosted");
    }
    assert!(hosted_cmds.iter().any(|c| c == crate::hosted::SHARE));
    assert!(!plain_cmds.iter().any(|c| c == crate::hosted::SHARE), "no Share without the host");
}

#[test]
fn hosted_never_shows_the_home_screen_and_home_or_new_do_nothing() {
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let mut plain = web_app(false, &saved, &shares);
    assert!(crate::menus::home_showing(&plain), "no document: the plain app starts on Home");
    plain.run("file.new", json!({})).unwrap();
    crate::menus::invoke(&mut plain, "app.home", json!({}));
    assert!(crate::menus::home_showing(&plain));

    let mut app = web_app(true, &saved, &shares);
    assert!(!crate::menus::home_showing(&app), "hosted: no Home screen before Clawku's file arrives");
    crate::menus::invoke(&mut app, "file.newDialog", json!({}));
    assert!(app.ui.dialog.is_none(), "no New Document dialog");
    app.run("file.new", json!({})).unwrap();
    crate::menus::invoke(&mut app, "app.home", json!({}));
    crate::menus::invoke(&mut app, "file.close", json!({}));
    assert!(!crate::menus::home_showing(&app));
    assert_eq!(app.session.documents().len(), 1, "the document stays open");
}

#[test]
fn hosted_share_goes_to_the_host() {
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let mut app = web_app(true, &saved, &shares);
    app.run("file.new", json!({})).unwrap();
    crate::menus::invoke(&mut app, crate::hosted::SHARE, json!({}));
    assert_eq!(shares.get(), 1);
    assert_eq!(app.run(crate::hosted::SHARE, json!({})).unwrap(), json!({"shared": true}));
    assert_eq!(shares.get(), 2);
    let mut plain = web_app(false, &saved, &shares);
    assert!(plain.run(crate::hosted::SHARE, json!({})).is_err());
}

#[test]
fn hosted_save_keeps_the_name_and_save_a_copy_is_marked_as_a_copy() {
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let mut app = web_app(true, &saved, &shares);
    app.run("file.new", json!({"width": 100, "height": 80})).unwrap();
    app.session.doc_mut().unwrap().path = Some("Poster.vectorcraft".into());
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 30, "height": 20})).unwrap();
    app.run("file.save", json!({})).unwrap();
    crate::io::save(&mut app, SaveMode::Copy, &json!({}), false).unwrap();
    // The Save a Copy dialog's name, even the document's own, is marked too.
    app.run("file.saveCopy", json!({"path": "Poster.vectorcraft"})).unwrap();
    assert_eq!(*saved.borrow(), ["Poster.vectorcraft", "Poster (copy).vectorcraft", "Poster (copy).vectorcraft"]);
    assert_eq!(app.session.doc().unwrap().path.as_deref(), Some("Poster.vectorcraft"), "a copy doesn't switch the document");

    // Not hosted, Save a Copy keeps its own name.
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let mut plain = web_app(false, &saved, &shares);
    plain.run("file.new", json!({"width": 100, "height": 80})).unwrap();
    plain.session.doc_mut().unwrap().path = Some("Poster.vectorcraft".into());
    crate::io::save(&mut plain, SaveMode::Copy, &json!({}), false).unwrap();
    assert_eq!(*saved.borrow(), ["Poster copy.vectorcraft"]);
}

#[test]
fn web_preferences_leave_out_folders_and_hosted_ones_data_recovery() {
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let desktop = VectorcraftApp::new(Session::new(), Services::default());
    let web = web_app(false, &saved, &shares);
    let hosted = web_app(true, &saved, &shares);
    for key in ["fontsFolder", "pluginsFolder", "scratchPrimary", "recoveryFolder", "templatesFolder", "recentFilesCount"] {
        assert!(!crate::hosted::hides_pref(&desktop, key), "{key} on the desktop");
        assert!(crate::hosted::hides_pref(&web, key), "{key} on the web");
    }
    for key in ["autosaveRecovery", "autosaveInterval", "showHomeScreen", "openDocumentsAsTabs"] {
        assert!(!crate::hosted::hides_pref(&web, key), "{key} on the plain web");
        assert!(crate::hosted::hides_pref(&hosted, key), "{key} while hosted");
    }
    let has = |app: &VectorcraftApp, c: &str| crate::prefs_dialog::shown_categories(app).contains(&c);
    assert!(has(&desktop, "Plug-ins & Scratch Disks"));
    assert!(!has(&web, "Plug-ins & Scratch Disks"), "only folders there");
    assert!(has(&web, "File Handling") && has(&hosted, "General"));
}

#[test]
fn hosted_has_no_help_menu_about_or_help_commands() {
    let (saved, shares) = (Saved::default(), Rc::new(Cell::new(0)));
    let plain = web_app(false, &saved, &shares);
    let mut hosted = web_app(true, &saved, &shares);
    let paths = |app: &VectorcraftApp| crate::menus::menu_entries(app).into_iter().map(|e| e.path).collect::<Vec<_>>();
    assert!(paths(&plain).iter().any(|p| p.first().is_some_and(|t| t == "Help")), "the plain web app has Help");
    assert!(!paths(&hosted).iter().any(|p| p.iter().any(|t| t == "Help")), "no Help menu while hosted");
    assert!(crate::hosted::hides_menu(&hosted, "Help") && !crate::hosted::hides_menu(&plain, "Help"));
    let labels = menu_labels(&hosted);
    for l in ["About ClawVector", "ClawVector Help…", "Join Our Discord", "Check for Updates…", "Search Commands…"] {
        assert!(!labels.iter().any(|x| x == l), "{l} is left out while hosted");
    }
    let (plain_cmds, hosted_cmds) = (menu_commands(&plain), menu_commands(&hosted));
    assert!(!hosted_cmds.iter().any(|c| c.starts_with("help.")), "no help.* item while hosted: {hosted_cmds:?}");
    for id in ["help.about", "help.discord", "help.website", "help.appPage", "help.github", "help.checkForUpdates"] {
        assert!(plain_cmds.iter().any(|c| c == id), "{id} stays in the plain web app");
        assert!(!crate::menus::enabled(&hosted, id), "{id} is off on its keys too");
        assert!(crate::hosted::hides_listed(&hosted, id), "{id} is not in the palette");
        assert!(!crate::hosted::hides_listed(&plain, id), "{id} is in the plain palette");
    }
    // About never opens from a hosted app.
    crate::menus::invoke(&mut hosted, "help.about", json!({}));
    assert!(!hosted.ui.about, "no About while hosted");
    // Search Commands still opens the palette (its shortcut, the search icon) but lists no Help.
    assert!(crate::hosted::hides_listed(&hosted, crate::hosted::PALETTE));
    assert!(!crate::hosted::hides(&hosted, crate::hosted::PALETTE));
    crate::menus::invoke(&mut hosted, crate::hosted::PALETTE, json!({}));
    assert!(hosted.ui.palette_open);
    // No "?" mark in the status bar while hosted.
    assert!(!crate::hosted::shows_help_mark(&hosted) && crate::hosted::shows_help_mark(&plain));
}
