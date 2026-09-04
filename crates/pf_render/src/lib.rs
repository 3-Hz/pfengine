//! Presentation layer (macroquad).
//!
//! This crate only ever *reads* the simulation. It interpolates between the
//! previous and current [`World`] by `alpha` (the fraction of a tick elapsed) so
//! a 60 Hz simulation renders smoothly at any refresh rate. Converting
//! fixed-point to `f32` here is fine — nothing in this crate feeds back into the
//! deterministic core.

use macroquad::prelude::*;
use pf_core::World;

const FIGHTER_W: f32 = 30.0;
const FIGHTER_H: f32 = 50.0;
/// Per-slot fighter colors; slots past the end wrap around.
const PALETTE: [Color; 8] = [SKYBLUE, RED, LIME, GOLD, VIOLET, ORANGE, PINK, WHITE];

/// The color for player slot `slot`, wrapping past the palette.
pub fn color_for(slot: usize) -> Color {
    PALETTE[slot % PALETTE.len()]
}

/// Map simulation coordinates (origin near center, +y up) to screen pixels
/// (origin top-left, +y down).
fn to_screen(x: f32, y: f32) -> (f32, f32) {
    let cx = screen_width() / 2.0;
    let cy = screen_height() / 2.0 + 150.0;
    (cx + x, cy - y)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Draw the interpolated state. `curr`/`prev` are consecutive sim ticks and
/// `alpha` is in `[0, 1]`.
pub fn draw_world(curr: &World, prev: &World, alpha: f32) {
    // Stage floor.
    let floor = curr.stage.floor_y.to_num::<f32>();
    let left = curr.stage.left.to_num::<f32>();
    let right = curr.stage.right.to_num::<f32>();
    let (lx, ly) = to_screen(left, floor);
    let (rx, _ry) = to_screen(right, floor);
    draw_line(lx, ly, rx, ly, 4.0, GRAY);

    // Fighters.
    for (i, (c, p)) in curr.players.iter().zip(&prev.players).enumerate() {
        let x = lerp(p.pos.x.to_num::<f32>(), c.pos.x.to_num::<f32>(), alpha);
        let y = lerp(p.pos.y.to_num::<f32>(), c.pos.y.to_num::<f32>(), alpha);
        let (sx, sy) = to_screen(x, y);
        draw_rectangle(
            sx - FIGHTER_W / 2.0,
            sy - FIGHTER_H,
            FIGHTER_W,
            FIGHTER_H,
            color_for(i),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_for_wraps_past_the_palette() {
        assert_eq!(color_for(PALETTE.len()), color_for(0));
        assert_eq!(color_for(PALETTE.len() + 3), color_for(3));
    }

    #[test]
    fn color_for_any_slot_does_not_panic() {
        for slot in 0..100 {
            let _ = color_for(slot);
        }
    }

    #[test]
    fn palette_colors_are_distinct() {
        for (i, a) in PALETTE.iter().enumerate() {
            for b in &PALETTE[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
