//! Vector icons drawn with the painter (no font or image assets needed).

use eframe::egui::{
    epaint::StrokeKind, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Pencil,
    Line,
    Rect,
    RectFill,
    Ellipse,
    EllipseFill,
    Fill,
    Picker,
    Select,
    New,
    Open,
    Save,
    Undo,
    Redo,
    CellGrid,
    PixelGrid,
    Fit,
}

pub const ACCENT: Color32 = Color32::from_rgb(0x6C, 0x5E, 0xB5); // C64 light blue

pub fn paint(p: &Painter, area: Rect, icon: Icon, c: Color32) {
    let s = area.width().min(area.height());
    let r = Rect::from_center_size(area.center(), Vec2::splat(s));
    let pt = |x: f32, y: f32| r.min + Vec2::new(x, y) * s;
    let w = (s * 0.085).max(1.5);
    let st = Stroke::new(w, c);
    let poly = |pts: Vec<Pos2>, fill: Color32, stroke: Stroke| {
        p.add(Shape::convex_polygon(pts, fill, stroke));
    };
    let path = |pts: Vec<Pos2>| {
        p.add(Shape::line(pts, st));
    };
    match icon {
        Icon::Pencil => {
            poly(vec![pt(0.70, 0.10), pt(0.90, 0.30), pt(0.42, 0.78), pt(0.22, 0.58)], c, Stroke::NONE);
            poly(vec![pt(0.17, 0.64), pt(0.36, 0.83), pt(0.08, 0.92)], c, Stroke::NONE);
        }
        Icon::Line => {
            p.line_segment([pt(0.16, 0.84), pt(0.84, 0.16)], Stroke::new(w * 1.4, c));
            p.circle_filled(pt(0.16, 0.84), w * 1.4, c);
            p.circle_filled(pt(0.84, 0.16), w * 1.4, c);
        }
        Icon::Rect => {
            p.rect_stroke(Rect::from_min_max(pt(0.14, 0.24), pt(0.86, 0.76)), 1.0, st, StrokeKind::Inside);
        }
        Icon::RectFill => {
            p.rect_filled(Rect::from_min_max(pt(0.14, 0.24), pt(0.86, 0.76)), 1.0, c);
        }
        Icon::Ellipse => {
            p.add(Shape::ellipse_stroke(pt(0.5, 0.5), Vec2::new(0.37, 0.28) * s, st));
        }
        Icon::EllipseFill => {
            p.add(Shape::ellipse_filled(pt(0.5, 0.5), Vec2::new(0.37, 0.28) * s, c));
        }
        Icon::Fill => {
            // tilted bucket + drop
            poly(vec![pt(0.12, 0.52), pt(0.44, 0.18), pt(0.76, 0.50), pt(0.44, 0.84)], c, Stroke::NONE);
            path(vec![pt(0.44, 0.18), pt(0.44, 0.08), pt(0.34, 0.08)]);
            p.circle_filled(pt(0.86, 0.72), s * 0.075, c);
            poly(vec![pt(0.86, 0.50), pt(0.92, 0.66), pt(0.80, 0.66)], c, Stroke::NONE);
        }
        Icon::Picker => {
            p.line_segment([pt(0.16, 0.84), pt(0.52, 0.48)], Stroke::new(w * 1.5, c));
            poly(vec![pt(0.46, 0.34), pt(0.66, 0.54), pt(0.76, 0.44), pt(0.56, 0.24)], c, Stroke::NONE);
            poly(vec![pt(0.60, 0.18), pt(0.82, 0.40), pt(0.90, 0.32), pt(0.68, 0.10)], c, Stroke::NONE);
        }
        Icon::Select => {
            let c4 = [pt(0.14, 0.18), pt(0.86, 0.18), pt(0.86, 0.82), pt(0.14, 0.82), pt(0.14, 0.18)];
            p.extend(Shape::dashed_line(&c4, st, s * 0.12, s * 0.08));
        }
        Icon::New => {
            path(vec![pt(0.24, 0.10), pt(0.58, 0.10), pt(0.80, 0.32), pt(0.80, 0.90), pt(0.24, 0.90), pt(0.24, 0.10)]);
            path(vec![pt(0.58, 0.10), pt(0.58, 0.32), pt(0.80, 0.32)]);
        }
        Icon::Open => {
            path(vec![pt(0.10, 0.80), pt(0.10, 0.22), pt(0.40, 0.22), pt(0.48, 0.32), pt(0.84, 0.32), pt(0.84, 0.80), pt(0.10, 0.80)]);
            path(vec![pt(0.10, 0.80), pt(0.24, 0.52), pt(0.96, 0.52), pt(0.84, 0.80)]);
        }
        Icon::Save => {
            p.rect_stroke(Rect::from_min_max(pt(0.14, 0.14), pt(0.86, 0.86)), 2.0, st, StrokeKind::Inside);
            p.rect_filled(Rect::from_min_max(pt(0.30, 0.14), pt(0.66, 0.40)), 0.0, c);
            p.rect_stroke(Rect::from_min_max(pt(0.28, 0.58), pt(0.72, 0.86)), 0.0, st, StrokeKind::Inside);
        }
        Icon::Undo | Icon::Redo => {
            let m = |x: f32| if icon == Icon::Redo { 1.0 - x } else { x };
            let mut pts = Vec::new();
            for i in 0..=24 {
                let th = (200.0 + 215.0 * i as f32 / 24.0).to_radians();
                pts.push(pt(m(0.5 + 0.30 * th.cos()), 0.56 + 0.30 * th.sin()));
            }
            path(pts);
            poly(vec![pt(m(0.184), 0.541), pt(m(0.303), 0.478), pt(m(0.133), 0.416)], c, Stroke::NONE);
        }
        Icon::CellGrid => {
            p.rect_stroke(Rect::from_min_max(pt(0.14, 0.14), pt(0.86, 0.86)), 0.0, st, StrokeKind::Inside);
            p.line_segment([pt(0.5, 0.14), pt(0.5, 0.86)], st);
            p.line_segment([pt(0.14, 0.5), pt(0.86, 0.5)], st);
        }
        Icon::PixelGrid => {
            let thin = Stroke::new((w * 0.7).max(1.0), c);
            p.rect_stroke(Rect::from_min_max(pt(0.14, 0.14), pt(0.86, 0.86)), 0.0, thin, StrokeKind::Inside);
            for k in [0.14 + 0.72 / 3.0, 0.14 + 1.44 / 3.0] {
                p.line_segment([pt(k, 0.14), pt(k, 0.86)], thin);
                p.line_segment([pt(0.14, k), pt(0.86, k)], thin);
            }
        }
        Icon::Fit => {
            for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let (cx, cy) = (0.14 + 0.72 * sx, 0.14 + 0.72 * sy);
                let (dx, dy) = (if sx == 0.0 { 0.22 } else { -0.22 }, if sy == 0.0 { 0.22 } else { -0.22 });
                path(vec![pt(cx + dx, cy), pt(cx, cy), pt(cx, cy + dy)]);
            }
        }
    }
}

/// A square icon button. Returns the response (check `.clicked()`).
pub fn button(ui: &mut Ui, icon: Icon, selected: bool, enabled: bool, size: f32, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hovered = resp.hovered() && enabled;
    let bg = if selected {
        ACCENT
    } else if hovered {
        Color32::from_white_alpha(22)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 7.0, bg);
    if selected {
        ui.painter().rect_stroke(rect, 7.0, Stroke::new(1.0_f32, Color32::from_white_alpha(70)), StrokeKind::Inside);
    }
    let col = if !enabled {
        Color32::from_gray(80)
    } else if selected || hovered {
        Color32::WHITE
    } else {
        Color32::from_gray(205)
    };
    paint(ui.painter(), rect.shrink(size * 0.2), icon, col);
    resp.on_hover_text(tip)
}
