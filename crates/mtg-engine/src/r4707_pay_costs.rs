//! The ban on paying costs with life or sacrifices (`Prohibited::PayCosts`, CR 601.2h, 602.2b): "Players can't pay life
//! or sacrifice creatures to cast spells or activate abilities." A spell or ability whose cost needs a barred payment
//! can't be cast or activated, a Phyrexian symbol is paid with mana, paying 0 life stays open, and a resolving effect
//! that makes a player pay life or sacrifice is not a cost (the rulings of Angel of Jubilation and Karn's Sylex).

use mtg_core::ids::{ObjectId, PlayerId};

use crate::prohibit::{Doing, forbidden};
use crate::state::GameState;

/// What a payment is for, which decides whether a ban on paying costs reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaysFor {
    /// The cost of casting a spell or activating an ability other than a mana ability.
    Cost,
    /// The cost of activating a mana ability.
    ManaAbility,
    /// A payment a resolving spell or ability, a replacement or a rule asks for; no cost.
    Effect,
}

/// Whether a rule keeps `player` from paying life for what `pays` names.
pub(crate) fn r4707_life_barred(state: &GameState, player: PlayerId, pays: PaysFor) -> bool {
    pays != PaysFor::Effect
        && forbidden(
            state,
            Doing::PayLifeForCost {
                player,
                mana_ability: pays == PaysFor::ManaAbility,
            },
        )
}

/// Whether a rule keeps `player` from sacrificing `object` for what `pays` names.
pub(crate) fn r4707_sacrifice_barred(
    state: &GameState,
    player: PlayerId,
    object: ObjectId,
    pays: PaysFor,
) -> bool {
    pays != PaysFor::Effect
        && forbidden(
            state,
            Doing::SacrificeForCost {
                player,
                object,
                mana_ability: pays == PaysFor::ManaAbility,
            },
        )
}

/// The life `player` may spend on the Phyrexian symbols of the cost of a spell or of an ability other than a mana
/// ability: `life`, or none of it while a rule keeps them from paying life for costs.
pub(crate) fn r4707_phyrexian_life(state: &GameState, player: PlayerId, life: i32) -> i32 {
    if r4707_life_barred(state, player, PaysFor::Cost) {
        life.min(0)
    } else {
        life
    }
}

/// The life another part of a cost reserves, raised to the whole life total while a rule keeps `player` from paying life
/// for costs, so no Phyrexian symbol of the cost of a spell or an ability is paid with life.
pub(crate) fn r4707_reserved_life(state: &GameState, player: PlayerId, reserved: u32) -> u32 {
    if r4707_life_barred(state, player, PaysFor::Cost) {
        let life = state
            .player(player)
            .map_or(0, |seat| u32::try_from(seat.life()).unwrap_or(0));
        reserved.max(life)
    } else {
        reserved
    }
}
