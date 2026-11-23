//! The policy's tunable constants, read from a per-thread table so a tuner can play two settings against each other
//! (SPSA, `examples/spsa_tune.rs`); every brain plays with [`Knobs::TUNED`] unless a tuner swaps them in.

use std::cell::Cell;

/// The policy constants a brain plays with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Knobs {
    /// What a play must gain over passing.
    pub margin: f64,
    /// What each mana source a main-phase play adds counts while the seat has few.
    pub ramp: f64,
    /// Mana sources below which ramp counts.
    pub ramp_until: f64,
    /// What an instant must gain to be cast in the seat's own main phase.
    pub instant_wait: f64,
    /// What a flash permanent must gain to be cast in the seat's own main phase.
    pub flash_wait: f64,
    /// The share of held interaction's worth a tap-out pays.
    pub tap_out: f64,
    /// The share of removal's and tricks' worth a tap-out loses (they still work on the seat's own turn).
    pub reactive: f64,
    /// What one piece of held interaction is worth with no card in the opponents' hands.
    pub hold_base: f64,
    /// What each card in the opponents' hands adds to it.
    pub hold_per_card: f64,
    /// Each further piece of held interaction counts this share of the one before.
    pub hold_decay: f64,
    /// A held wipe's share of the hold value.
    pub wipe: f64,
    /// A held counterspell's share.
    pub counter: f64,
    /// Held instant removal's share.
    pub removal: f64,
    /// A held fog's share.
    pub fog: f64,
    /// Held burn's share.
    pub burn: f64,
    /// A held trick's share.
    pub trick: f64,
    /// A blocker worth less than this share of the attacker trades up.
    pub trade_up: f64,
    /// What each untapped blocker an attacked player keeps counts against focusing them.
    pub wall: f64,
    /// What a lethal hit in a combat forecast costs.
    pub lethal_hit: f64,
    /// Lands the seat wants before more stop counting.
    pub lands_wanted: f64,
    /// 1 to aim at a multiplayer table by threat (opposing life counted flat, never low life alone), 0 for the former
    /// low-life lean.
    pub archenemy: f64,
    /// What each point of damage an opponent dealt the seat's team lately adds to focusing them (retaliation).
    pub rival: f64,
    /// What each point of damage an opponent took lately, past the least-hit opponent, takes from focusing them.
    pub spread: f64,
}

impl Knobs {
    /// The hand-set constants before tuning.
    pub const HAND_SET: Knobs = Knobs {
        margin: 0.25,
        ramp: 4.0,
        ramp_until: 5.0,
        instant_wait: 1.0,
        flash_wait: 2.0,
        tap_out: 1.0,
        reactive: 0.25,
        hold_base: 4.0,
        hold_per_card: 1.6,
        hold_decay: 0.5,
        wipe: 1.4,
        counter: 1.0,
        removal: 0.8,
        fog: 0.6,
        burn: 0.5,
        trick: 0.35,
        trade_up: 0.6,
        wall: 8.0,
        lethal_hit: 500.0,
        lands_wanted: 6.0,
        archenemy: 1.0,
        rival: 0.4,
        spread: 0.2,
    };

    /// The constants every brain plays with.
    pub const TUNED: Knobs = Knobs::HAND_SET;
}

thread_local! {
    static KNOBS: Cell<Knobs> = const { Cell::new(Knobs::TUNED) };
}

/// The constants the current thread plays with.
pub fn knobs() -> Knobs {
    KNOBS.with(Cell::get)
}

/// Runs `work` with `knobs` on the current thread, then restores the constants it had.
pub fn with_knobs<T>(knobs: Knobs, work: impl FnOnce() -> T) -> T {
    let before = KNOBS.with(|cell| cell.replace(knobs));
    let result = work();
    KNOBS.with(|cell| cell.set(before));
    result
}

#[cfg(test)]
mod tests {
    //! Goal: a thread plays with the tuned constants unless a tuner swaps others in for a while.
    use super::*;

    #[test]
    fn swapped_constants_last_only_while_the_work_runs() {
        assert_eq!(knobs(), Knobs::TUNED);
        let mut other = Knobs::TUNED;
        other.margin += 1.0;
        let seen = with_knobs(other, knobs);
        assert_eq!(seen, other);
        assert_eq!(knobs(), Knobs::TUNED);
    }
}
