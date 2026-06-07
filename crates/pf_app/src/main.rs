//! pfengine entry point (Phase 0).
//!
//! Runs the documented fixed-timestep loop: the deterministic [`World`] advances
//! in whole 60 Hz ticks, while rendering interpolates between ticks for
//! smoothness. Input *source* (the keyboard) lives here in the platform layer;
//! the simulation only ever sees a [`pf_core::Input`].

use macroquad::prelude::*;
use pf_core::{buttons, Input, World};

/// Seconds per simulation tick (60 Hz).
const TICK: f32 = 1.0 / 60.0;
/// Guard against the "spiral of death" if a frame hitches badly.
const MAX_STEPS_PER_FRAME: u32 = 5;

fn window_conf() -> Conf {
    Conf {
        window_title: "pfengine".to_owned(),
        window_width: 960,
        window_height: 540,
        high_dpi: true,
        ..Default::default()
    }
}

/// Read one player's input from a key triplet.
fn poll(left: KeyCode, right: KeyCode, jump: KeyCode) -> Input {
    let mut input = Input::default();
    let mut sx: i32 = 0;
    if is_key_down(left) {
        sx -= 110;
    }
    if is_key_down(right) {
        sx += 110;
    }
    input.stick_x = sx as i8;
    if is_key_down(jump) {
        input.buttons |= buttons::JUMP;
    }
    input
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    let mut prev = world.clone();
    let mut acc: f32 = 0.0;

    loop {
        acc += get_frame_time();

        let mut steps = 0;
        while acc >= TICK && steps < MAX_STEPS_PER_FRAME {
            prev = world.clone();
            let p1 = poll(KeyCode::Left, KeyCode::Right, KeyCode::Space);
            let p2 = poll(KeyCode::A, KeyCode::D, KeyCode::W);
            world.advance([p1, p2]);
            acc -= TICK;
            steps += 1;
        }
        // If we hit the step cap, drop the backlog rather than spiral.
        if steps == MAX_STEPS_PER_FRAME {
            acc = 0.0;
        }

        let alpha = (acc / TICK).clamp(0.0, 1.0);

        clear_background(Color::from_rgba(18, 18, 24, 255));
        pf_render::draw_world(&world, &prev, alpha);
        draw_text(
            "pfengine - Phase 0   P1: <- -> / Space    P2: A D / W",
            16.0,
            28.0,
            22.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!("frame {}", world.frame),
            16.0,
            52.0,
            18.0,
            DARKGRAY,
        );

        next_frame().await;
    }
}
