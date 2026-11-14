use super::Cell;
use crate::ui::theme::*;
use eframe::egui::*;

pub(super) fn body(painter: &Painter, cell: &Cell, body: Rect, color: Color32) {
    let left = cell.rect.left();
    let right = cell.rect.right();
    if right <= left {
        return;
    }
    let painter = painter.with_clip_rect(body.intersect(painter.clip_rect()));
    let activity = cell.phone.boundary_strength as f32;
    if cell.phone.has_activity {
        let peak = left + (right - left) * cell.phone.boundary_position as f32;
        let height = activity * activity * body.height();
        let first = peak.max(body.left());
        let last = right.min(body.right());
        if last <= first {
            return;
        }
        let y = |x: f32| {
            let tail = 1.0 - (x - peak) / (right - peak);
            body.bottom() - tail * tail * height
        };
        let mut points = vec![pos2(first, body.bottom()), pos2(first, y(first))];

        let step = 1.0 / painter.ctx().pixels_per_point();
        let mut x = (first / step).ceil() * step;
        while x < last {
            points.push(pos2(x, y(x)));
            x += step;
        }
        points.push(pos2(last, y(last)));
        points.push(pos2(last, body.bottom()));
        let mut mesh = Mesh::default();
        let base = points[0];

        for i in 1..points.len() - 1 {
            let at = mesh.vertices.len() as u32;
            for p in [base, points[i], points[i + 1]] {
                mesh.colored_vertex(p, color.gamma_multiply(0.16));
            }
            mesh.indices.extend([at, at + 1, at + 2]);
        }
        painter.add(Shape::mesh(mesh));
        painter.add(Shape::line(points, Stroke::new(1.0, color)));
    } else {
        let y = body.bottom() - (0.5 + 0.5 * cell.phone.strength) * body.height();
        painter.hline(
            left..=right,
            y,
            Stroke::new(
                0.7,
                if cell.phone.strength == 0.0 {
                    MUTED
                } else {
                    color
                },
            ),
        );
    }
}
