use crate::ui::theme;
use eframe::egui::*;

#[derive(Clone, Copy)]
struct Point {
    x: f32,
    top: f32,
    bottom: f32,
    intensity: f32,
}

pub(super) struct Flash {
    mesh: Mesh,
    previous: Option<Point>,
    clip: Rect,
    pixel: f32,
}

impl Flash {
    pub fn new(clip: Rect, pixel: f32) -> Self {
        Self {
            mesh: Mesh::default(),
            previous: None,
            clip,
            pixel,
        }
    }

    pub fn push(&mut self, x: f32, top: f32, bottom: f32, intensity: f32) {
        let next = Point {
            x,
            top,
            bottom,
            intensity,
        };
        if let Some(previous) = self.previous
            && next.x > previous.x
            && (previous.intensity > 0.0 || next.intensity > 0.0)
        {
            self.edge(previous, next, true);
            self.edge(previous, next, false);
        }
        self.previous = Some(next);
    }

    pub fn extend_to(&mut self, x: f32) {
        if let Some(previous) = self.previous {
            self.push(x, previous.top, previous.bottom, previous.intensity);
        }
    }

    pub fn gap(&mut self, x: f32) {
        self.extend_to(x);
        self.previous = None;
    }

    pub fn paint(self, painter: &Painter) -> bool {
        let visible = !self.mesh.is_empty();
        if visible {
            painter.add(Shape::mesh(self.mesh));
        }
        visible
    }

    fn edge(&mut self, from: Point, to: Point, upper: bool) {
        let (from_y, to_y, direction) = if upper {
            (from.top, to.top, -1.0)
        } else {
            (from.bottom, to.bottom, 1.0)
        };
        let bounds = Rect::from_min_max(
            pos2(from.x, from_y.min(to_y) - self.pixel * 2.0),
            pos2(to.x, from_y.max(to_y) + self.pixel * 2.0),
        );
        if !bounds.intersects(self.clip) {
            return;
        }

        let bands = [(2.0, 0.0), (0.8, 0.22), (0.0, 0.85), (-0.65, 0.0)];
        for band in bands.windows(2) {
            let index = self.mesh.vertices.len() as u32;
            for (point, y, (offset, opacity)) in [
                (from, from_y, band[0]),
                (to, to_y, band[0]),
                (to, to_y, band[1]),
                (from, from_y, band[1]),
            ] {
                self.mesh.colored_vertex(
                    pos2(point.x, y + direction * offset * self.pixel),
                    Color32::from_rgba_unmultiplied(
                        theme::GREEN.r(),
                        theme::GREEN.g(),
                        theme::GREEN.b(),
                        (255.0 * opacity * point.intensity).round() as u8,
                    ),
                );
            }
            self.mesh.add_triangle(index, index + 1, index + 2);
            self.mesh.add_triangle(index, index + 2, index + 3);
        }
    }
}
