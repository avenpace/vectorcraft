//! Fingers on a touch screen, run as the hosts run them (`raw_input_hook`, then the UI pass), in
//! the order a browser sends them through eframe: a frame for a finger's `pointerdown` (its press
//! alone), then its `touchstart` (the press again with the touch), moves, a frame for `pointerup`
//! (the release alone), then `touchend`. A second finger's `pointerdown` and `pointerup` press
//! and release the primary button too; its touch events carry none.
//!
//! On the canvas one finger is the tool and the Hand tool pans; two fingers pan and pinch-zoom the
//! view whatever the tool and leave nothing the first began. Off the canvas a finger drag scrolls
//! a panel's list ([`crate::touch::FingerFilter`]). A mouse is left as it was.

use egui::{Event, Pos2, TouchPhase, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;
use crate::canvas::Xf;

/// One headless frame of the whole window at `t` seconds, through the raw input hook.
fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, t: f64, events: Vec<Event>) {
    let screen = egui::Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
    app.raw_input_hook(ctx, &mut raw);
    let mut out = ctx.run_ui(raw, |ui| app.ui(ui));
    out.textures_delta.clear();
}

fn touch(n: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(n), phase, pos, force: None }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// The frames eframe's web runner makes of finger `n` (0 is the first, the primary touch).
fn finger(n: u64, phase: TouchPhase, pos: Pos2) -> Vec<Vec<Event>> {
    let primary = n == 0;
    match phase {
        TouchPhase::Start if primary => vec![vec![button(pos, true)], vec![button(pos, true), touch(n, phase, pos)]],
        TouchPhase::Start => vec![vec![button(pos, true)], vec![touch(n, phase, pos)]],
        TouchPhase::Move if primary => vec![vec![Event::PointerMoved(pos), touch(n, phase, pos)]],
        TouchPhase::Move => vec![vec![touch(n, phase, pos)]],
        TouchPhase::End | TouchPhase::Cancel if primary => {
            vec![vec![button(pos, false)], vec![button(pos, false), Event::PointerGone, touch(n, phase, pos)]]
        }
        TouchPhase::End | TouchPhase::Cancel => vec![vec![button(pos, false)], vec![touch(n, phase, pos)]],
    }
}

/// Runs `frames` one after another, 1/60 s apart, from `t`.
fn play(app: &mut VectorcraftApp, ctx: &egui::Context, t: &mut f64, frames: Vec<Vec<Event>>) {
    for events in frames {
        *t += 1.0 / 60.0;
        frame(app, ctx, *t, events);
    }
}

/// A document with one rectangle and `tool` selected, drawn twice; the clock after it. `coarse`:
/// the device said its main pointer is a finger (`(pointer: coarse)`).
fn app_with(tool: &str, ctx: &egui::Context, coarse: bool) -> (VectorcraftApp, f64) {
    let mut app = VectorcraftApp::new(Session::new(), crate::Services::default());
    if coarse {
        app.expect_touch();
    }
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 40, "height": 40})).unwrap();
    app.run("select.none", json!({})).unwrap();
    app.select_tool(tool);
    frame(&mut app, ctx, 0.0, vec![]);
    frame(&mut app, ctx, 0.1, vec![]);
    (app, 0.1)
}

fn objects(app: &VectorcraftApp) -> usize {
    app.session.active().unwrap().doc.layers[0].children().unwrap().len()
}

/// Two fingers come down `gap` apart about `c`: the first presses and starts the tool's drag with
/// a move of (10, 10), then the second lands, also (10, 10) off, so their centre is `c + (10, 10)`.
/// Each of `steps` moves them to (centre, gap); then the second lifts, then the first.
fn two_fingers(app: &mut VectorcraftApp, ctx: &egui::Context, t: &mut f64, c: Pos2, gap: f32, steps: &[(Pos2, f32)]) {
    let off = vec2(10.0, 10.0);
    let (a, b) = (c - vec2(gap / 2.0, 0.0), c + vec2(gap / 2.0, 0.0) + off);
    play(app, ctx, t, finger(0, TouchPhase::Start, a));
    play(app, ctx, t, finger(0, TouchPhase::Move, a + off));
    play(app, ctx, t, finger(1, TouchPhase::Start, b));
    let (mut a, mut b) = (a + off, b);
    for &(at, gap) in steps {
        a = at - vec2(gap / 2.0, 0.0);
        b = at + vec2(gap / 2.0, 0.0);
        let mut events = finger(0, TouchPhase::Move, a).remove(0);
        events.push(touch(1, TouchPhase::Move, b));
        play(app, ctx, t, vec![events]);
    }
    play(app, ctx, t, finger(1, TouchPhase::End, b));
    play(app, ctx, t, finger(0, TouchPhase::End, a));
    play(app, ctx, t, vec![vec![]]);
}

/// Two fingers dragging together pan the view with them, and the rectangle the first finger began
/// is dropped: nothing is drawn and nothing is left to undo. Also on a device that didn't say it
/// has a touch screen, whose first press reaches egui as a mouse's.
#[test]
fn a_two_finger_drag_pans_and_leaves_no_tool_action() {
    for coarse in [true, false] {
        let ctx = egui::Context::default();
        let (mut app, mut t) = app_with("rectangle", &ctx, coarse);
        let c = app.canvas_rect.unwrap().center();
        let (before, undo) = (*app.view().unwrap(), app.session.active().unwrap().history.undo.len());
        let steps: Vec<_> = (1..=6).map(|i| (c + vec2(10.0, 10.0) + vec2(8.0, 5.0) * i as f32, 100.0)).collect();
        two_fingers(&mut app, &ctx, &mut t, c, 100.0, &steps);
        let after = *app.view().unwrap();
        assert_eq!(objects(&app), 1, "no rectangle from the first finger (coarse {coarse})");
        assert_eq!(app.session.active().unwrap().history.undo.len(), undo, "nothing to undo");
        assert!(app.session.active().unwrap().interaction.is_none());
        assert!((after.zoom - before.zoom).abs() < 1e-6, "no zoom: {} -> {}", before.zoom, after.zoom);
        // The content followed the fingers' centre: 48 right and 30 down on screen.
        let moved = ((before.center.x - after.center.x) * before.zoom, (before.center.y - after.center.y) * before.zoom);
        assert!((moved.0 - 48.0).abs() < 2.0 && (moved.1 - 30.0).abs() < 2.0, "panned {moved:?} (coarse {coarse})");
    }
}

/// Fingers spreading apart zoom about the point between them (not about the first finger, which
/// is the pointer): the document point under that centre stays under it.
#[test]
fn a_pinch_zooms_about_the_point_between_the_fingers() {
    let ctx = egui::Context::default();
    let (mut app, mut t) = app_with("selection", &ctx, true);
    let rect = app.canvas_rect.unwrap();
    let c = rect.center() + vec2(-40.0, 20.0);
    let z0 = app.view().unwrap().zoom;
    let mid = c + vec2(10.0, 10.0);
    let under = Xf::new(rect, app.view().unwrap()).to_doc(mid);
    let steps: Vec<_> = (1..=5).map(|i| (mid, 100.0 + 20.0 * i as f32)).collect();
    two_fingers(&mut app, &ctx, &mut t, c, 100.0, &steps);
    let v = app.view().unwrap();
    assert!((v.zoom / z0 - 2.0).abs() < 0.05, "the fingers twice as far apart zoom in twice: {z0} -> {}", v.zoom);
    let now = Xf::new(rect, v).to_screen(under);
    assert!((now - mid).length() < 2.0, "the point between the fingers stays put: {now:?} vs {mid:?}");
    assert_eq!(objects(&app), 1);
    assert!(app.session.active().unwrap().selection.objects.is_empty(), "the first finger's marquee selected nothing");
}

/// A one-finger drag from the canvas centre by (60, 40).
fn one_finger_drag(app: &mut VectorcraftApp, ctx: &egui::Context, t: &mut f64) {
    let c = app.canvas_rect.unwrap().center();
    play(app, ctx, t, finger(0, TouchPhase::Start, c));
    play(app, ctx, t, finger(0, TouchPhase::Move, c + vec2(30.0, 20.0)));
    play(app, ctx, t, finger(0, TouchPhase::Move, c + vec2(60.0, 40.0)));
    play(app, ctx, t, finger(0, TouchPhase::End, c + vec2(60.0, 40.0)));
    play(app, ctx, t, vec![vec![]]);
}

/// One finger with a drawing tool still draws (the canvas isn't a scroll area): one rectangle,
/// from where the finger came down to where it lifted, one undo step.
#[test]
fn one_finger_draws_with_the_rectangle_tool() {
    for coarse in [true, false] {
        let ctx = egui::Context::default();
        let (mut app, mut t) = app_with("rectangle", &ctx, coarse);
        let undo = app.session.active().unwrap().history.undo.len();
        one_finger_drag(&mut app, &ctx, &mut t);
        assert_eq!(objects(&app), 2, "a rectangle drawn (coarse {coarse})");
        assert_eq!(app.session.active().unwrap().history.undo.len(), undo + 1);
        let zoom = app.view().unwrap().zoom;
        let doc = &app.session.active().unwrap().doc;
        let id = doc.layers[0].children().unwrap().last().unwrap().id;
        let b = doc.bounds_of(&[id], false).unwrap();
        assert!((b.width() * zoom - 60.0).abs() < 1.5 && (b.height() * zoom - 40.0).abs() < 1.5, "{b:?} at {zoom}");
    }
}

/// With the Hand tool one finger pans the view.
#[test]
fn the_hand_tool_pans_with_one_finger() {
    let ctx = egui::Context::default();
    let (mut app, mut t) = app_with("hand", &ctx, true);
    let before = *app.view().unwrap();
    one_finger_drag(&mut app, &ctx, &mut t);
    let after = *app.view().unwrap();
    assert_eq!(objects(&app), 1);
    let moved = ((before.center.x - after.center.x) * before.zoom, (before.center.y - after.center.y) * before.zoom);
    assert!((moved.0 - 60.0).abs() < 1.0 && (moved.1 - 40.0).abs() < 1.0, "the content followed the finger: {moved:?}");
}

/// A mouse drag still draws, and on a device with no touch screen its events pass the finger
/// filter exactly as they came.
#[test]
fn a_mouse_drag_is_unchanged() {
    let ctx = egui::Context::default();
    let (mut app, mut t) = app_with("rectangle", &ctx, false);
    let c = app.canvas_rect.unwrap().center();
    let steps = [
        vec![Event::PointerMoved(c), button(c, true)],
        vec![Event::PointerMoved(c + vec2(30.0, 20.0))],
        vec![Event::PointerMoved(c + vec2(60.0, 40.0))],
        vec![button(c + vec2(60.0, 40.0), false)],
    ];
    for events in steps {
        let mut probe = crate::touch::FingerFilter::default();
        let mut raw = egui::RawInput { events: events.clone(), time: Some(t), ..Default::default() };
        probe.filter(&mut raw, |_| false, || crate::touch::Under::List);
        assert_eq!(raw.events, events, "the finger filter leaves a mouse alone");
        t += 1.0 / 60.0;
        frame(&mut app, &ctx, t, events);
    }
    assert_eq!(objects(&app), 2, "a rectangle drawn");
}

/// A finger dragging up the Layers panel's list scrolls it, 1:1: no row moves, nothing is
/// selected. A tap on a row then selects it. Also when the device didn't say it has a touch
/// screen, and the finger's first press reached egui as a mouse's.
#[test]
fn a_finger_drag_scrolls_the_layers_panel() {
    for coarse in [true, false] {
        let mut app = VectorcraftApp::new(Session::new(), crate::Services::default());
        if coarse {
            app.expect_touch();
        }
        app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
        for i in 0..40 {
            app.run("layer.new", json!({"name": format!("Layer {i}")})).unwrap();
        }
        let ctx = egui::Context::default();
        let doc = app.session.active().unwrap().doc.clone();
        let rows = app.session.active().unwrap().highlighted_rows();
        let mut t = 0.0;
        // The panel alone on a 300 × 400 window: the filter, the finger's scroll handed to egui
        // (as `VectorcraftApp::ui` does), the panel. The target circles' centres, top first.
        let mut panel = |app: &mut VectorcraftApp, frames: Vec<Vec<Event>>| {
            let mut circles = vec![];
            for events in frames {
                t += 1.0 / 60.0;
                let screen = egui::Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0));
                let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
                app.fingers.filter(&mut raw, |_| false, || crate::touch::under_finger(&ctx));
                let mut out = ctx.run_ui(raw, |ui| {
                    let d = app.fingers.take_scroll();
                    ui.ctx().input_mut(|i| i.smooth_scroll_delta += d);
                    crate::panels::layers::show(app, ui);
                });
                out.textures_delta.clear();
                circles = out
                    .shapes
                    .iter()
                    .filter_map(|c| if let egui::Shape::Circle(cs) = &c.shape { (cs.radius == 5.0).then_some(cs.center) } else { None })
                    .collect();
                circles.sort_by(|a, b| a.y.total_cmp(&b.y));
            }
            circles
        };
        // A touch screen seen before (egui gives a list its drag-to-scroll then).
        panel(&mut app, vec![vec![touch(9, TouchPhase::Cancel, egui::pos2(-10.0, -10.0))]]);
        let before = panel(&mut app, vec![vec![]]);
        let row_h = before[1].y - before[0].y;
        let p = egui::pos2(150.0, 250.0);
        panel(&mut app, finger(0, TouchPhase::Start, p));
        for i in 1..=10 {
            panel(&mut app, finger(0, TouchPhase::Move, p - vec2(0.0, 12.0 * i as f32)));
        }
        panel(&mut app, finger(0, TouchPhase::End, p - vec2(0.0, 120.0)));
        let after = panel(&mut app, vec![vec![]]);
        let st = app.session.active().unwrap();
        assert_eq!(*st.doc, *doc, "no row moved (coarse {coarse})");
        if coarse {
            assert_eq!(st.highlighted_rows(), rows, "nothing selected");
        }
        assert!(!ctx.input(|i| i.pointer.primary_down()), "egui holds no button");
        // The rows went up 120 points with the finger.
        let off = (before[0].y - after[0].y).rem_euclid(row_h);
        assert!((off - 120.0_f32.rem_euclid(row_h)).abs() < 1.0, "the list scrolled 1:1: {:?} -> {:?} (coarse {coarse})", before[0], after[0]);
        // A tap selects the row under it: the third drawn, now 120 points further down the list.
        let q = egui::pos2(150.0, after[2].y);
        panel(&mut app, finger(0, TouchPhase::Start, q));
        panel(&mut app, finger(0, TouchPhase::End, q));
        panel(&mut app, vec![vec![]]);
        let st = app.session.active().unwrap();
        assert_eq!(*st.doc, *doc);
        let picked = st.highlighted_rows();
        assert_eq!(picked.len(), 1, "a tap selects a row");
        let row = st.doc.layers.iter().rev().position(|l| l.id == picked[0]).expect("a layer row");
        let want = ((after[2].y - before[0].y + 120.0) / row_h).round() as usize;
        assert_eq!(row, want, "the row the list scrolled under the finger");
    }
}
