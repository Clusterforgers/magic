//! Priority: who may act, what they may do, and when a round of passes is complete.

use std::collections::BTreeSet;

use mtg_core::card::{AbilityDef, CardType};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::characteristics;
use crate::decision::{PriorityAction, PriorityOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mana::{ManaOption, ManaSource, can_afford};
use crate::player::Player;
use crate::state::GameState;
use crate::turn::LAND_DROPS_PER_TURN;
use crate::zones::Zone;

/// Who holds priority and who has passed since the last action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PriorityState {
    pub(crate) holder: PlayerId,
    pub(crate) passed: BTreeSet<PlayerId>,
}

impl PriorityState {
    /// The player who may act now.
    pub const fn holder(&self) -> PlayerId {
        self.holder
    }

    /// Players who passed in succession.
    pub const fn passed(&self) -> &BTreeSet<PlayerId> {
        &self.passed
    }
}

/// True once every living player has passed in succession.
pub(crate) fn all_passed(state: &GameState) -> bool {
    state
        .priority()
        .is_some_and(|p| state.alive_players().all(|a| p.passed.contains(&a)))
}

/// The mana abilities `player` can activate right now.
pub(crate) fn mana_options(state: &GameState, player: PlayerId) -> Vec<ManaOption> {
    let mut options = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        let Some(perm) = object.permanent() else {
            continue;
        };
        if object.controller() != player || perm.tapped() {
            continue;
        }
        let Some(chars) = characteristics(state, id) else {
            continue;
        };
        // Only creatures are subject to summoning sickness for {T} abilities (CR 302.6).
        if chars.is_creature() && perm.summoning_sick() {
            continue;
        }
        for (ability, def) in chars.abilities.iter().enumerate() {
            let AbilityDef::Mana(mana) = def;
            options.push(ManaOption {
                source: ManaSource {
                    object: id,
                    ability,
                },
                produces: mana.produces,
            });
        }
    }
    options
}

fn sorcery_timing(state: &GameState, player: PlayerId) -> bool {
    state.turn().active() == player && state.turn().step().is_main() && state.stack().is_empty()
}

fn hand_land(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.base_types().contains(CardType::Land))
}

/// Activated abilities of `player`'s permanents that are not mana abilities and could be used now.
///
/// No card has one yet (see [`AbilityDef`]); when one does, list it here and [`can_act`] counts it.
fn other_abilities(_state: &GameState, _player: PlayerId) -> Vec<ObjectId> {
    Vec::new()
}

/// Whether `player` can do anything worth stopping for: play a land, cast a spell or use an ability that is not a mana ability.
///
/// `castable` already holds only the spells the floating mana and the untapped sources can pay for. Mana
/// abilities alone never count: tapping a land with nothing to spend the mana on is not an action.
pub(crate) fn can_act(
    state: &GameState,
    player: PlayerId,
    playable_lands: &[ObjectId],
    castable: &[ObjectId],
) -> bool {
    !playable_lands.is_empty() || !castable.is_empty() || !other_abilities(state, player).is_empty()
}

/// Everything `player` may do while holding priority.
pub(crate) fn priority_options(state: &GameState, player: PlayerId) -> PriorityOptions {
    let hand = state.zone(Zone::Hand(player));
    let mana_abilities = mana_options(state, player);
    let pool = state.player(player).map(|p| *p.mana()).unwrap_or_default();
    let life = state.player(player).map_or(0, Player::life);
    let may_play_land =
        sorcery_timing(state, player) && state.turn().lands_played() < LAND_DROPS_PER_TURN;
    let playable_lands: Vec<ObjectId> = if may_play_land {
        hand.iter()
            .copied()
            .filter(|id| hand_land(state, *id))
            .collect()
    } else {
        Vec::new()
    };
    let castable: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|id| {
            let Some(object) = state.object(*id) else {
                return false;
            };
            let types = object.base_types();
            let Some(cost) = object.card().mana_cost.as_ref() else {
                return false;
            };
            if types.contains(CardType::Land) {
                return false;
            }
            let timing = types.contains(CardType::Instant) || sorcery_timing(state, player);
            timing && can_afford(cost, &pool, &mana_abilities, life)
        })
        .collect();
    PriorityOptions {
        can_act: can_act(state, player, &playable_lands, &castable),
        playable_lands,
        castable,
        mana_abilities,
        can_pass: true,
    }
}

/// Validates a priority action and turns it into events (or a cast in progress).
pub(crate) fn answer(
    player: PlayerId,
    options: &PriorityOptions,
    action: PriorityAction,
) -> Result<Outcome, EngineError> {
    match action {
        PriorityAction::Pass => Ok(Outcome::Events(vec![Event::PriorityPassed { player }])),
        PriorityAction::PlayLand(object) => {
            if !options.playable_lands.contains(&object) {
                return Err(IllegalAnswer::NotOffered("land").into());
            }
            Ok(Outcome::Events(vec![Event::LandPlayed { player, object }]))
        }
        PriorityAction::CastSpell(object) => {
            if !options.castable.contains(&object) {
                return Err(IllegalAnswer::NotOffered("spell").into());
            }
            Ok(Outcome::BeginCast(object))
        }
        PriorityAction::ActivateMana(source) => {
            let option = options
                .mana_abilities
                .iter()
                .find(|o| o.source == source)
                .ok_or(IllegalAnswer::NotOffered("mana ability"))?;
            Ok(Outcome::Events(vec![
                Event::PermanentTapped {
                    object: source.object,
                },
                Event::ManaAdded {
                    player,
                    mana: option.produces,
                    source: Some(source.object),
                },
            ]))
        }
    }
}
