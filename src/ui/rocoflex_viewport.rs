use crate::synthesis::rocoflex::wire;
use eframe::egui::Vec2;
use std::time::{Duration, Instant};

const RESIZE_SETTLE: Duration = Duration::from_millis(90);
const SIZE_STEP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CanvasViewport {
    pub size: [f32; 2],
    pub scale: f32,
}

impl CanvasViewport {
    fn fit(size: Vec2, pixels_per_point: f32) -> Self {
        let ratio = (size.x / wire::MIN_SIZE[0]).min(size.y / wire::MIN_SIZE[1]).clamp(0.01, 1.0);
        let round = |value: f32| (value / SIZE_STEP).round() * SIZE_STEP;
        let mut viewport = Self {
            size: [round(size.x / ratio).clamp(wire::MIN_SIZE[0], 4096.0),
                   round(size.y / ratio).clamp(wire::MIN_SIZE[1], 4096.0)],
            scale: 1.0,
        };
        viewport.scale = (((pixels_per_point * viewport.zoom(size)).clamp(0.2, 8.0) * 8.0).round() / 8.0).max(0.2);
        viewport
    }

    pub fn zoom(self, room: Vec2) -> f32 {
        (room.x / self.size[0]).min(room.y / self.size[1]).max(0.01)
    }

    fn close_to(self, other: Self) -> bool {
        self.scale == other.scale && self.size.iter().zip(other.size)
            .all(|(old, new)| (old - new).abs() <= SIZE_STEP)
    }
}

#[derive(Default)]
pub(super) struct Viewport {
    wanted: Option<CanvasViewport>,
    pending: Option<(CanvasViewport, Instant)>,
}

impl Viewport {
    pub fn update(&mut self, room: Vec2, pixels_per_point: f32) -> (CanvasViewport, Option<Duration>) {
        let desired = CanvasViewport::fit(room, pixels_per_point);
        let current = *self.wanted.get_or_insert(desired);
        if desired.close_to(current) {
            self.pending = None;
            return (current, None);
        }

        let now = Instant::now();
        let (target, since) = if let Some((target, since)) = self.pending.filter(|(target, _)| *target == desired) {
            (target, since)
        } else {
            self.pending = Some((desired, now));
            (desired, now)
        };
        let remaining = RESIZE_SETTLE.saturating_sub(now.duration_since(since));
        if remaining.is_zero() {
            self.pending = None;
            self.wanted = Some(target);
            (target, None)
        } else {
            (current, Some(remaining))
        }
    }
}
