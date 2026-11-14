//! Small stroke icons drawn with the painter. Next to a text label they are decorative; icon-only
//! buttons (`icon_button`) carry an accessible name and a tooltip instead.

use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, pos2};

const WIDTH: f32 = 1.75;

/// Draws an open polyline given in a 24x24 design grid fitted into `rect`, with round caps and joins.
fn path(p: &Painter, rect: Rect, color: Color32, pts: &[(f32, f32)]) {
    let k = rect.width().min(rect.height()) / 24.0;
    let at = |&(x, y): &(f32, f32)| -> Pos2 { pos2(rect.min.x + x * k, rect.min.y + y * k) };
    let pts: Vec<Pos2> = pts.iter().map(at).collect();
    // egui strokes have butt caps; a disc on every vertex rounds the caps and the joins.
    for &q in &pts {
        p.circle_filled(q, WIDTH / 2.0, color);
    }
    p.line(pts, Stroke::new(WIDTH, color));
}

fn ring(p: &Painter, rect: Rect, color: Color32, (x, y): (f32, f32), r: f32) {
    let k = rect.width().min(rect.height()) / 24.0;
    let c = pos2(rect.min.x + x * k, rect.min.y + y * k);
    p.circle_stroke(c, r * k, Stroke::new(WIDTH, color));
}

pub fn plus(p: &Painter, rect: Rect, color: Color32) {
    path(p, rect, color, &[(12.0, 5.0), (12.0, 19.0)]);
    path(p, rect, color, &[(5.0, 12.0), (19.0, 12.0)]);
}

pub fn close(p: &Painter, rect: Rect, color: Color32) {
    path(p, rect, color, &[(6.0, 6.0), (18.0, 18.0)]);
    path(p, rect, color, &[(18.0, 6.0), (6.0, 18.0)]);
}

/// Curved arrow pointing back (left).
pub fn undo(p: &Painter, rect: Rect, color: Color32) {
    path(p, rect, color, &[(8.0, 5.0), (4.0, 9.0), (8.0, 13.0)]);
    path(
        p,
        rect,
        color,
        &[
            (4.0, 9.0),
            (14.0, 9.0),
            (19.0, 11.5),
            (19.0, 16.0),
            (15.0, 19.0),
            (9.0, 19.0),
        ],
    );
}

/// Curved arrow pointing forward (right).
pub fn redo(p: &Painter, rect: Rect, color: Color32) {
    path(p, rect, color, &[(16.0, 5.0), (20.0, 9.0), (16.0, 13.0)]);
    path(
        p,
        rect,
        color,
        &[
            (20.0, 9.0),
            (10.0, 9.0),
            (5.0, 11.5),
            (5.0, 16.0),
            (9.0, 19.0),
            (15.0, 19.0),
        ],
    );
}

pub fn check(p: &Painter, rect: Rect, color: Color32) {
    path(p, rect, color, &[(5.0, 12.5), (10.0, 17.5), (19.0, 7.0)]);
}

/// Scissors.
pub fn split(p: &Painter, rect: Rect, color: Color32) {
    ring(p, rect, color, (7.0, 18.0), 2.5);
    ring(p, rect, color, (17.0, 18.0), 2.5);
    path(p, rect, color, &[(8.6, 15.9), (19.0, 4.5)]);
    path(p, rect, color, &[(15.4, 15.9), (5.0, 4.5)]);
}

/// Arrow down into a tray.
pub fn save(p: &Painter, rect: Rect, color: Color32) {
    path(p, rect, color, &[(12.0, 4.0), (12.0, 15.0)]);
    path(p, rect, color, &[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
    path(
        p,
        rect,
        color,
        &[(4.0, 15.0), (4.0, 20.0), (20.0, 20.0), (20.0, 15.0)],
    );
}

/// A page with a folded corner and an up arrow.
pub fn file_add(p: &Painter, rect: Rect, color: Color32) {
    path(
        p,
        rect,
        color,
        &[
            (6.0, 3.0),
            (14.0, 3.0),
            (19.0, 8.0),
            (19.0, 21.0),
            (6.0, 21.0),
            (6.0, 3.0),
        ],
    );
    path(p, rect, color, &[(14.0, 3.0), (14.0, 8.0), (19.0, 8.0)]);
    path(p, rect, color, &[(12.5, 17.5), (12.5, 11.5)]);
    path(p, rect, color, &[(10.0, 14.0), (12.5, 11.5), (15.0, 14.0)]);
}

pub type IconFn = fn(&Painter, Rect, Color32);

pub fn glyph<'a>(icon: IconFn) -> eframe::egui::Atom<'a> {
    // Painted in the button's text colour, so disabled buttons fade it too.
    eframe::egui::Atom::paint(eframe::egui::Vec2::splat(16.0), move |ui, a| {
        icon(ui.painter(), a.rect, a.fallback_text_color)
    })
}

/// A button with a leading icon and a text label; returns the response of the real egui button.
pub fn button(
    ui: &mut eframe::egui::Ui,
    enabled: bool,
    icon: IconFn,
    text: impl Into<eframe::egui::WidgetText>,
) -> eframe::egui::Response {
    use eframe::egui::{Button, vec2};
    ui.add_enabled(
        enabled,
        Button::new((glyph(icon), text)).min_size(vec2(0.0, 40.0)),
    )
}

/// A frameless 40 px icon-only button; `name` is its accessible name and tooltip.
pub fn icon_button(ui: &mut eframe::egui::Ui, icon: IconFn, name: &str) -> eframe::egui::Response {
    use eframe::egui::{Button, Vec2};
    ui.add(
        Button::new(glyph(icon))
            .frame_when_inactive(false)
            .min_size(Vec2::splat(40.0)),
    )
    .accessible_name(name)
    .on_hover_text(name)
    .on_disabled_hover_text(name)
}
