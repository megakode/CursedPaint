//! Pixel-coordinate generators for drawing tools.

pub type Pt = (i32, i32);

/// Bresenham line, endpoints inclusive.
pub fn line(a: Pt, b: Pt) -> Vec<Pt> {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::new();
    loop {
        out.push((x0, y0));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    out
}

fn bounds(a: Pt, b: Pt) -> (i32, i32, i32, i32) {
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

pub fn rect(a: Pt, b: Pt, filled: bool) -> Vec<Pt> {
    let (x0, y0, x1, y1) = bounds(a, b);
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if filled || x == x0 || x == x1 || y == y0 || y == y1 {
                out.push((x, y));
            }
        }
    }
    out
}

/// Ellipse inscribed in the bounding box of `a`,`b`.
pub fn ellipse(a: Pt, b: Pt, filled: bool) -> Vec<Pt> {
    let (x0, y0, x1, y1) = bounds(a, b);
    let (w, h) = ((x1 - x0) as f64, (y1 - y0) as f64);
    if w == 0.0 || h == 0.0 {
        return line((x0, y0), (x1, y1));
    }
    let (cx, cy) = (x0 as f64 + w / 2.0, y0 as f64 + h / 2.0);
    let (rx, ry) = (w / 2.0 + 0.5, h / 2.0 + 0.5);
    let inside = |x: i32, y: i32| {
        let nx = (x as f64 - cx) / rx;
        let ny = (y as f64 - cy) / ry;
        nx * nx + ny * ny <= 1.0
    };
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !inside(x, y) {
                continue;
            }
            let edge = !inside(x - 1, y) || !inside(x + 1, y) || !inside(x, y - 1) || !inside(x, y + 1);
            if filled || edge {
                out.push((x, y));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_includes_endpoints() {
        let l = line((0, 0), (5, 2));
        assert_eq!(l.first(), Some(&(0, 0)));
        assert_eq!(l.last(), Some(&(5, 2)));
        assert_eq!(l.len(), 6);
    }

    #[test]
    fn rect_outline_count() {
        assert_eq!(rect((0, 0), (3, 3), false).len(), 12);
        assert_eq!(rect((3, 3), (0, 0), true).len(), 16);
    }

    #[test]
    fn ellipse_filled_contains_center() {
        assert!(ellipse((0, 0), (10, 6), true).contains(&(5, 3)));
        assert!(!ellipse((0, 0), (10, 6), false).contains(&(5, 3)));
    }
}
