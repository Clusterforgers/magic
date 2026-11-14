//! Who hurt whom lately: the damage each player's sources dealt to each other player, by turn, kept for the last
//! [`WINDOW_ROUNDS`] rounds. Rules never read it; bots read it to spread their pressure and to strike back.

use mtg_core::ids::PlayerId;

use crate::combat::DamageTarget;
use crate::events::Event;
use crate::state::GameState;

/// How many rounds of the table the record keeps.
pub const WINDOW_ROUNDS: u32 = 2;

/// The damage dealt to players in the window: (turn number, the source's controller, the player hurt, amount).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Rivalry {
    recent: Vec<(u32, PlayerId, PlayerId, u32)>,
}

impl Rivalry {
    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.recent.is_empty()
    }
}

/// The first turn number inside the window.
fn window_start(state: &GameState) -> u32 {
    let seats = u32::try_from(state.players().len()).unwrap_or(u32::MAX);
    state
        .turn
        .number
        .saturating_sub(WINDOW_ROUNDS.saturating_mul(seats))
        .saturating_add(1)
}

/// The damage `from`'s sources dealt to `to` in the window.
pub fn dealt(state: &GameState, from: PlayerId, to: PlayerId) -> u32 {
    let start = window_start(state);
    state
        .rivalry
        .recent
        .iter()
        .filter(|(turn, by, hurt, _)| *turn >= start && *by == from && *hurt == to)
        .map(|(_, _, _, amount)| *amount)
        .fold(0, u32::saturating_add)
}

/// The damage `to` took from every other player's sources in the window.
pub fn received(state: &GameState, to: PlayerId) -> u32 {
    let start = window_start(state);
    state
        .rivalry
        .recent
        .iter()
        .filter(|(turn, by, hurt, _)| *turn >= start && *hurt == to && *by != to)
        .map(|(_, _, _, amount)| *amount)
        .fold(0, u32::saturating_add)
}

/// Records damage `event` deals to a player, read before it is folded; drops what left the window as a turn begins.
pub(crate) fn note(state: &mut GameState, event: &Event) {
    match event {
        Event::DamageDealt {
            source,
            target: DamageTarget::Player(hurt),
            amount,
            ..
        } if *amount > 0 => {
            let Some(by) = crate::characteristics::controller_of(state, *source) else {
                return;
            };
            if by == *hurt {
                return;
            }
            let turn = state.turn.number;
            state.rivalry.recent.push((turn, by, *hurt, *amount));
        }
        Event::TurnBegan { .. } if !state.rivalry.recent.is_empty() => {
            let start = window_start(state);
            state.rivalry.recent.retain(|(turn, ..)| *turn >= start);
        }
        _ => {}
    }
}
