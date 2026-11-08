//! The life damage really takes from a player under a life floor, which "whenever you lose life" triggers read (CR 120.3a, 614.1).

use mtg_core::ids::PlayerId;

use crate::combat::{DamageForm, DamageTarget};
use crate::events::Event;
use crate::state::GameState;

/// The life `amount` damage to `player` takes, as the batch stands: the events `earlier` in the batch have already moved the
/// life total from what it was in `pre`, and a floor leaves the total at it. Without a floor it is the whole amount.
pub(super) fn damage_lost(
    pre: &GameState,
    earlier: &[Event],
    player: PlayerId,
    amount: u32,
) -> u32 {
    let Some(mut life) = pre.player(player).map(crate::player::Player::life) else {
        return amount;
    };
    let wanted = |amount: u32| i32::try_from(amount).unwrap_or(i32::MAX);
    let capped = |life: i32, amount: u32| match crate::rules::damage_life_floor(pre, player, life) {
        Some(floor) => wanted(amount).min(life.saturating_sub(floor)),
        None => wanted(amount),
    };
    for event in earlier {
        life = match event {
            Event::DamageDealt {
                target: DamageTarget::Player(who),
                amount,
                form,
                ..
            } if *who == player && !matches!(form, DamageForm::Infect | DamageForm::NoLifeLoss) => {
                life.saturating_sub(capped(life, *amount))
            }
            Event::LifeLost {
                player: who,
                amount,
            }
            | Event::LifePaid {
                player: who,
                amount,
            } if *who == player => life.saturating_sub(wanted(*amount)),
            Event::LifeGained {
                player: who,
                amount,
            } if *who == player => life.saturating_add(wanted(*amount)),
            _ => life,
        };
    }
    u32::try_from(capped(life, amount)).unwrap_or(0)
}
