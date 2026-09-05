//! Input sources and player-slot assignment. Platform layer only — the
//! simulation sees one [`Input`] per slot and nothing else.

use macroquad::prelude::*;
use pf_core::{buttons, Input};

/// Produces one [`Input`] per tick: a keyboard layout today, a gamepad or
/// GameCube adapter later. `&mut` because gamepad backends pump events on poll.
pub trait InputSource {
    fn poll(&mut self) -> Input;
    /// Short name for the HUD.
    fn label(&self) -> &str;
}

/// One keyboard layout: a move pair and a jump key.
pub struct Keyboard {
    left: KeyCode,
    right: KeyCode,
    jump: KeyCode,
    label: &'static str,
}

impl InputSource for Keyboard {
    fn poll(&mut self) -> Input {
        let mut input = Input::default();
        let mut sx: i32 = 0;
        if is_key_down(self.left) {
            sx -= 110;
        }
        if is_key_down(self.right) {
            sx += 110;
        }
        input.stick_x = sx as i8;
        if is_key_down(self.jump) {
            input.buttons |= buttons::JUMP;
        }
        input
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// The four built-in keyboard layouts. Any of them can drive any slot.
pub fn keyboard_sources() -> Vec<Box<dyn InputSource>> {
    let layout = |left, right, jump, label| -> Box<dyn InputSource> {
        Box::new(Keyboard {
            left,
            right,
            jump,
            label,
        })
    };
    vec![
        layout(
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Space,
            "Arrows + Space",
        ),
        layout(KeyCode::A, KeyCode::D, KeyCode::W, "A D + W"),
        layout(KeyCode::J, KeyCode::L, KeyCode::I, "J L + I"),
        layout(KeyCode::Kp4, KeyCode::Kp6, KeyCode::Kp8, "Numpad 4 6 + 8"),
    ]
}

/// Maps player slots to sources. Slots start empty; a source joins the lowest
/// free *local* slot by pressing jump, and that joining press is swallowed so
/// it does not also jump. Remote slots belong to another machine: they are
/// never claimed here and always emit `Input::default()`, since their input
/// arrives through the session.
pub struct Slots {
    /// slot → source index
    source_of: Vec<Option<usize>>,
    /// slot → driven from this machine?
    local: Vec<bool>,
    /// Last tick's raw poll per source, for edge detection.
    prev: Vec<Input>,
    /// This tick's polls; kept to avoid allocating every tick.
    cur: Vec<Input>,
    /// Sources that joined this tick.
    joined: Vec<bool>,
}

impl Slots {
    /// `local` lists the slots a source on this machine may claim.
    ///
    /// # Panics
    /// If `local` names a slot at or past `num_players` — a contract bug.
    pub fn new(num_players: usize, local: &[usize]) -> Self {
        let mut is_local = vec![false; num_players];
        for &slot in local {
            is_local[slot] = true;
        }
        Slots {
            source_of: vec![None; num_players],
            local: is_local,
            prev: Vec::new(),
            cur: Vec::new(),
            joined: Vec::new(),
        }
    }

    /// Poll every source, join newly pressed unassigned ones, and write one
    /// `Input` per slot into `out` (`default()` for empty and remote slots).
    pub fn tick(&mut self, sources: &mut [Box<dyn InputSource>], out: &mut [Input]) {
        assert_eq!(
            out.len(),
            self.source_of.len(),
            "tick needs one output Input per slot"
        );
        let n = sources.len();
        self.prev.resize(n, Input::default());
        self.cur.clear();
        self.cur.extend(sources.iter_mut().map(|s| s.poll()));
        self.joined.clear();
        self.joined.resize(n, false);

        for src in 0..n {
            let edge =
                self.cur[src].pressed(buttons::JUMP) && !self.prev[src].pressed(buttons::JUMP);
            if !edge || self.is_assigned(src) {
                continue;
            }
            if let Some(free) = self.lowest_free_local_slot() {
                self.source_of[free] = Some(src);
                self.joined[src] = true;
            }
        }

        self.prev.copy_from_slice(&self.cur);

        for (slot, out) in self.source_of.iter().zip(out.iter_mut()) {
            *out = match *slot {
                Some(src) if !self.joined[src] => self.cur[src],
                _ => Input::default(),
            };
        }
    }

    /// The source driving `slot`, if any.
    pub fn source_of(&self, slot: usize) -> Option<usize> {
        self.source_of[slot]
    }

    /// Whether `slot` is driven from this machine.
    pub fn is_local(&self, slot: usize) -> bool {
        self.local[slot]
    }

    fn is_assigned(&self, src: usize) -> bool {
        self.source_of.contains(&Some(src))
    }

    fn lowest_free_local_slot(&self) -> Option<usize> {
        self.source_of
            .iter()
            .zip(&self.local)
            .position(|(taken, &local)| local && taken.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::HashSet;
    use std::rc::Rc;

    /// A source the test drives by writing to the shared cell.
    struct Stub(Rc<Cell<Input>>);

    impl InputSource for Stub {
        fn poll(&mut self) -> Input {
            self.0.get()
        }
        fn label(&self) -> &str {
            "stub"
        }
    }

    fn jump() -> Input {
        Input {
            buttons: buttons::JUMP,
            ..Input::default()
        }
    }

    fn walk() -> Input {
        Input {
            stick_x: 100,
            ..Input::default()
        }
    }

    /// The test's handles on each stub, and the stubs as the app sees them.
    type Rig = (Vec<Rc<Cell<Input>>>, Vec<Box<dyn InputSource>>);

    fn rig(n: usize) -> Rig {
        let cells: Vec<_> = (0..n)
            .map(|_| Rc::new(Cell::new(Input::default())))
            .collect();
        let sources = cells
            .iter()
            .map(|c| Box::new(Stub(c.clone())) as Box<dyn InputSource>)
            .collect();
        (cells, sources)
    }

    /// Every slot local, as local play has it.
    fn all_local(n: usize) -> Vec<usize> {
        (0..n).collect()
    }

    #[test]
    fn slots_start_empty_and_emit_default_input() {
        let (_cells, mut sources) = rig(2);
        let mut slots = Slots::new(2, &all_local(2));
        let mut out = vec![walk(); 2]; // stale data must be overwritten
        slots.tick(&mut sources, &mut out);
        assert_eq!(slots.source_of(0), None);
        assert_eq!(slots.source_of(1), None);
        assert_eq!(out, vec![Input::default(); 2]);
    }

    #[test]
    fn pressing_jump_joins_the_lowest_free_slot() {
        let (cells, mut sources) = rig(3);
        let mut slots = Slots::new(2, &all_local(2));
        let mut out = vec![Input::default(); 2];
        cells[2].set(jump()); // the *third* source joins first
        slots.tick(&mut sources, &mut out);
        assert_eq!(slots.source_of(0), Some(2));
        assert_eq!(slots.source_of(1), None);
    }

    #[test]
    fn the_joining_press_does_not_jump() {
        let (cells, mut sources) = rig(1);
        let mut slots = Slots::new(1, &all_local(1));
        let mut out = vec![Input::default(); 1];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out);
        assert_eq!(out[0], Input::default());
    }

    #[test]
    fn an_assigned_source_drives_its_slot() {
        let (cells, mut sources) = rig(2);
        let mut slots = Slots::new(2, &all_local(2));
        let mut out = vec![Input::default(); 2];
        cells[1].set(jump());
        slots.tick(&mut sources, &mut out); // source 1 → slot 0
        cells[1].set(walk());
        slots.tick(&mut sources, &mut out);
        assert_eq!(out[0], walk());
        assert_eq!(out[1], Input::default());
    }

    #[test]
    fn holding_jump_is_not_a_second_join() {
        let (cells, mut sources) = rig(1);
        let mut slots = Slots::new(2, &all_local(2));
        let mut out = vec![Input::default(); 2];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out); // edge: joins slot 0
        slots.tick(&mut sources, &mut out); // still held: no edge
        assert_eq!(slots.source_of(0), Some(0));
        assert_eq!(slots.source_of(1), None);
        assert_eq!(out[0], jump()); // the held jump now flows through
    }

    #[test]
    fn an_assigned_source_cannot_claim_a_second_slot() {
        let (cells, mut sources) = rig(1);
        let mut slots = Slots::new(2, &all_local(2));
        let mut out = vec![Input::default(); 2];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out);
        cells[0].set(Input::default());
        slots.tick(&mut sources, &mut out);
        cells[0].set(jump()); // a fresh edge from an already-assigned source
        slots.tick(&mut sources, &mut out);
        assert_eq!(slots.source_of(0), Some(0));
        assert_eq!(slots.source_of(1), None);
        assert_eq!(out[0], jump()); // ...is an ordinary jump
    }

    #[test]
    fn a_full_roster_ignores_further_joins() {
        let (cells, mut sources) = rig(2);
        let mut slots = Slots::new(1, &all_local(1));
        let mut out = vec![Input::default(); 1];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out); // source 0 takes the only slot
        cells[0].set(Input::default());
        cells[1].set(jump());
        slots.tick(&mut sources, &mut out); // nowhere for source 1 to go
        cells[1].set(walk());
        slots.tick(&mut sources, &mut out);
        assert_eq!(slots.source_of(0), Some(0));
        assert_eq!(out[0], Input::default()); // source 1's walk goes nowhere
    }

    #[test]
    fn remote_slots_are_never_claimed() {
        let (cells, mut sources) = rig(1);
        let mut slots = Slots::new(2, &[1]); // slot 0 belongs to another machine
        let mut out = vec![Input::default(); 2];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out);
        assert!(!slots.is_local(0));
        assert!(slots.is_local(1));
        assert_eq!(slots.source_of(0), None);
        assert_eq!(slots.source_of(1), Some(0));
        assert_eq!(out[0], Input::default()); // a remote slot's input comes over the wire
    }

    #[test]
    fn the_joiner_takes_the_lowest_free_local_slot() {
        let (cells, mut sources) = rig(2);
        let mut slots = Slots::new(3, &[0, 2]);
        let mut out = vec![Input::default(); 3];
        cells[0].set(jump());
        slots.tick(&mut sources, &mut out); // source 0 -> slot 0
        cells[0].set(Input::default());
        cells[1].set(jump());
        slots.tick(&mut sources, &mut out); // source 1 skips remote slot 1
        assert_eq!(slots.source_of(0), Some(0));
        assert_eq!(slots.source_of(1), None);
        assert_eq!(slots.source_of(2), Some(1));
    }

    #[test]
    fn keyboard_sources_are_four_distinct_layouts() {
        let sources = keyboard_sources();
        assert_eq!(sources.len(), 4);
        let labels: HashSet<&str> = sources.iter().map(|s| s.label()).collect();
        assert_eq!(labels.len(), 4);
    }
}
