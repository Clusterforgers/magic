//! The manual action that flips a coin or rolls a die: the engine draws the result from the game's random stream, so
//! every seat reads the same number and a replay of the log reproduces it.

use mtg_core::ids::PlayerId;

use crate::events::Event;
use crate::manual::{MAX_SIDES, MIN_SIDES, ManualRefusal, ManualRoll};
use crate::rng::GameRng;
use crate::state::GameState;

/// The result of `kind` for `player`: the next substream of the game's random stream gives a face from 1 to the sides
/// (a coin has two), logged as `ManualRolled`, whose fold moves the stream on by one.
pub(crate) fn roll(
    state: &GameState,
    player: PlayerId,
    kind: ManualRoll,
) -> Result<Vec<Event>, ManualRefusal> {
    let sides = match kind {
        ManualRoll::Coin {} => 2,
        ManualRoll::Die { sides } if (MIN_SIDES..=MAX_SIDES).contains(&sides) => sides,
        ManualRoll::Die { .. } => return Err(ManualRefusal::OutOfRange("the number of faces")),
    };
    let below = GameRng::substream(state.seed, state.rng_index).below(u64::from(sides));
    Ok(vec![Event::ManualRolled {
        player,
        kind,
        result: u32::try_from(below).unwrap_or(0) + 1,
    }])
}
