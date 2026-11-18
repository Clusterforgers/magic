//! How a bot answers a loop offer: a loop that never ends by itself is a draw, one that helps the seat runs a sensible
//! number of times (to its end when it ends by itself), and one that does not help is stopped.

use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_engine::decision::Answer;
use mtg_engine::loops::{LoopAnswer, LoopOptions, LoopStep};

/// Iterations a bot runs of a loop that grows without end.
pub const GROWING: u32 = 100;

/// What one iteration is worth to `seat`: its tokens and life up, the opponents' down.
pub fn loop_worth(seat: PlayerId, step: &LoopStep) -> i64 {
    let tokens: i64 = step
        .tokens
        .iter()
        .map(|tokens| {
            let count = i64::from(tokens.count);
            if tokens.controller == seat {
                count
            } else {
                -count
            }
        })
        .sum();
    let life: i64 = step
        .life
        .iter()
        .map(|life| {
            if life.player == seat {
                life.change
            } else {
                -life.change
            }
        })
        .sum();
    tokens + life
}

/// The bot's answer to the loop offer `options` asked of `seat`.
pub fn loop_answer(seat: PlayerId, options: &LoopOptions) -> Answer {
    if options.can_draw {
        return Answer::Loop(LoopAnswer::Draw);
    }
    if options.max == 0 || loop_worth(seat, &options.per_iteration) <= 0 {
        return Answer::Loop(LoopAnswer::Stop);
    }
    let times = if options.bounded {
        options.max
    } else {
        GROWING.min(options.max)
    };
    Answer::Loop(LoopAnswer::Repeat(times))
}

/// Iterations the boss's side of a Delves fight runs of any loop at most.
pub const BOSS_LOOP_CAP: u32 = 5;

/// [`loop_answer`] for a seat of the game `state`: a seat on a boss's side (`spotlight::is_boss_side`) never draws the
/// game and runs a loop at most [`BOSS_LOOP_CAP`] times, so a boss never wins or stalls a fight by a loop.
pub fn loop_answer_in(state: &GameState, seat: PlayerId, options: &LoopOptions) -> Answer {
    if !mtg_engine::spotlight::is_boss_side(state, seat) {
        return loop_answer(seat, options);
    }
    if options.max == 0 || loop_worth(seat, &options.per_iteration) <= 0 {
        return Answer::Loop(LoopAnswer::Stop);
    }
    Answer::Loop(LoopAnswer::Repeat(BOSS_LOOP_CAP.min(options.max)))
}
