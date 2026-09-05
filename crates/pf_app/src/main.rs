//! pfengine entry point.
//!
//! Runs the documented fixed-timestep loop: the deterministic [`World`] advances
//! in whole 60 Hz ticks, while rendering interpolates between ticks for
//! smoothness. Input sources and slot assignment live here in the platform
//! layer (see [`input`]); the simulation only ever sees one [`Input`] per slot.

mod input;
#[cfg(target_arch = "wasm32")]
mod wasm_entropy;

use macroquad::prelude::*;
use pf_core::{Input, World};

use crate::input::{keyboard_sources, Slots};

/// Seconds per simulation tick (60 Hz).
const TICK: f32 = 1.0 / 60.0;
/// Guard against the "spiral of death" if a frame hitches badly.
const MAX_STEPS_PER_FRAME: u32 = 5;
/// Player count without `--players` — always the case on wasm, which has no argv.
const DEFAULT_PLAYERS: usize = 2;

fn window_conf() -> Conf {
    Conf {
        window_title: "pfengine".to_owned(),
        window_width: 960,
        window_height: 540,
        high_dpi: true,
        ..Default::default()
    }
}

/// Parse `--players N` from the arguments after the program name, ignoring
/// anything else.
///
/// # Panics
/// On a missing, non-numeric, or zero value — a usage error worth failing
/// loudly on.
fn parse_players<I: IntoIterator<Item = String>>(args: I) -> usize {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--players" {
            let value = args.next().expect("--players needs a value");
            let n: usize = value.parse().expect("--players must be a positive integer");
            assert!(n > 0, "--players must be at least 1");
            return n;
        }
    }
    DEFAULT_PLAYERS
}

#[macroquad::main(window_conf)]
async fn main() {
    let num_players = parse_players(std::env::args().skip(1));
    let mut world = World::new(num_players);
    let mut prev = world.clone();
    let mut acc: f32 = 0.0;

    let mut sources = keyboard_sources();
    let mut slots = Slots::new(num_players, &(0..num_players).collect::<Vec<_>>());
    let mut inputs = vec![Input::default(); num_players];

    loop {
        acc += get_frame_time();

        let mut steps = 0;
        while acc >= TICK && steps < MAX_STEPS_PER_FRAME {
            prev = world.clone();
            slots.tick(&mut sources, &mut inputs);
            world.advance(&inputs);
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
            format!("pfengine - {num_players} players   frame {}", world.frame),
            16.0,
            28.0,
            22.0,
            LIGHTGRAY,
        );
        for slot in 0..num_players {
            let status = match slots.source_of(slot) {
                Some(src) => sources[src].label(),
                None => "press jump to join",
            };
            draw_text(
                format!("P{}: {status}", slot + 1),
                16.0,
                52.0 + 20.0 * slot as f32,
                18.0,
                pf_render::color_for(slot),
            );
        }

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_to_two_players() {
        assert_eq!(parse_players(args(&[])), DEFAULT_PLAYERS);
    }

    #[test]
    fn reads_the_players_flag() {
        assert_eq!(parse_players(args(&["--players", "4"])), 4);
    }

    #[test]
    fn ignores_unrelated_args() {
        assert_eq!(parse_players(args(&["--foo", "--players", "3", "bar"])), 3);
    }

    #[test]
    #[should_panic]
    fn rejects_zero_players() {
        parse_players(args(&["--players", "0"]));
    }

    #[test]
    #[should_panic]
    fn rejects_non_numeric_players() {
        parse_players(args(&["--players", "four"]));
    }

    #[test]
    #[should_panic]
    fn rejects_a_missing_value() {
        parse_players(args(&["--players"]));
    }
}
