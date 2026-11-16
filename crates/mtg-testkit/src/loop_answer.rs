//! How a bot answers a loop offer: a loop that never ends by itself is a draw, one that helps the seat runs a sensible
//! number of times (to its end when it ends by itself), and one that does not help is stopped.

use mtg_core::ids::PlayerId;
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
