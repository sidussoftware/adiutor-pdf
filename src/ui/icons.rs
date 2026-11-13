//! Small stroke icons drawn with the painter. They are decorative: every button also has a text label.

use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, pos2};

const WIDTH: f32 = 1.75;

/// Draws an open polyline given in a 24x24 design grid fitted into `rect`, with round caps and joins.
fn path(p: &Painter, rect: Rect, color: Color32, pts: &[(f32, f32)]) {
    let k = rect.width().min(rect.height()) / 24.0;
    let at = |&(x, y): &(f32, f32)| -> Pos2 { pos2(rect.min.x + x * k, rect.min.y + y * k) };
    let pts: Vec<Pos2> = pts.iter().map(at).collect();
    p.line(pts.clone(), Stroke::new(WIDTH, color));
    // egui strokes have butt caps; a disc on every vertex rounds the caps and the joins.
    for q in pts {
        p.circle_filled(q, WIDTH / 2.0, color);
    }
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
#[expect(dead_code, reason = "used by the empty state")]
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

/// A button with a leading icon and a text label; returns the response of the real egui button.
pub fn button(
    ui: &mut eframe::egui::Ui,
    enabled: bool,
    icon: fn(&Painter, Rect, Color32),
    text: &str,
) -> eframe::egui::Response {
    use eframe::egui::{Button, Vec2, vec2};
    // Three en spaces (about 21 px at 14 pt) leave room for the 16 px icon.
    let r = ui.add_enabled(
        enabled,
        Button::new(format!("\u{2002}\u{2002}\u{2002}{text}")).min_size(vec2(0.0, 40.0)),
    );
    let size = Vec2::splat(16.0);
    let pad = ui.spacing().button_padding.x;
    let rect = Rect::from_center_size(
        pos2(r.rect.min.x + pad + size.x / 2.0, r.rect.center().y),
        size,
    );
    icon(ui.painter(), rect, ui.style().interact(&r).fg_stroke.color);
    r
}
