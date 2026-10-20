//! Priority: who may act, what they may do, and when a round of passes is complete.

use std::collections::BTreeSet;

use mtg_cards::AbilityDef;
use mtg_cards::ability::{Ability, Limit, Timing};
use mtg_cards::assemble::{ManaProduced, SimpleManaAbility, simple_mana_ability};
use mtg_cards::cost::CostItem;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::abilities::AbilityRef;
use crate::answers::Outcome;
use crate::characteristics::characteristics;
use crate::choose::announceable;
use crate::decision::{PriorityAction, PriorityOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::{Event, ZoneCause};
use crate::keywords;
use crate::mana::{ManaOption, ManaSource, can_afford};
use crate::object::GameObject;
use crate::player::Player;
use crate::resolve::spell_ability;
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
        if chars.is_creature() && keywords::summoning_sick(state, id) {
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

/// The five colours "any color" offers, in the order of the wheel.
const COLORS: [ManaType; 5] = [
    ManaType::White,
    ManaType::Blue,
    ManaType::Black,
    ManaType::Red,
    ManaType::Green,
];

/// A mana ability from the rules of a permanent that is activated at priority, such as a Treasure's, with the option
/// that stands for each way to use it.
struct RuleMana {
    /// What it costs and adds.
    shape: SimpleManaAbility,
    /// One option for the mana it adds, or one for each colour when it adds any.
    options: Vec<ManaOption>,
}

/// The mana abilities in the rules of `object` that the engine activates at priority (`simple_mana_ability`).
///
/// An option is named by the object and an index past its printed mana abilities: rule `r` takes `r` times five and
/// then one more for each colour of an ability that adds any, so every colour is a distinct source of one permanent
/// and a client taps for the colour it wants.
fn rule_mana(object: &GameObject, printed: usize) -> Vec<RuleMana> {
    let mut found = Vec::new();
    for (index, rules) in object.card().rules.iter().enumerate() {
        let Some(shape) = simple_mana_ability(rules) else {
            continue;
        };
        let source = |colour: usize| ManaSource {
            object: object.id(),
            ability: printed + index * COLORS.len() + colour,
        };
        let options = match shape.produces {
            ManaProduced::Fixed(mana) => vec![ManaOption {
                source: source(0),
                produces: mana,
            }],
            ManaProduced::AnyColor => COLORS
                .iter()
                .enumerate()
                .map(|(colour, &mana)| ManaOption {
                    source: source(colour),
                    produces: mana,
                })
                .collect(),
        };
        found.push(RuleMana { shape, options });
    }
    found
}

/// The mana abilities of the rules of `player`'s permanents that they can activate now: a Treasure that is untapped,
/// or any permanent whose ability does not tap it. They are activated at priority and never offered to pay a spell:
/// the mana floats in the pool, so a source that sacrifices itself is never used up by an automatic payment.
pub(crate) fn rule_mana_options(state: &GameState, player: PlayerId) -> Vec<ManaOption> {
    let mut options = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        let Some(perm) = object.permanent() else {
            continue;
        };
        if object.controller() != player {
            continue;
        }
        let Some(chars) = characteristics(state, id) else {
            continue;
        };
        // Only creatures are subject to summoning sickness for {T} abilities (CR 302.6).
        let sick = chars.is_creature() && keywords::summoning_sick(state, id);
        for mana in rule_mana(object, chars.abilities.len()) {
            if mana.shape.tap && (perm.tapped() || sick) {
                continue;
            }
            options.extend(mana.options);
        }
    }
    options
}

/// The costs of the rule-based mana ability `source` stands for, if it is one.
fn rule_mana_costs(state: &GameState, source: ManaSource) -> Option<SimpleManaAbility> {
    let object = state.object(source.object)?;
    let printed = characteristics(state, source.object)?.abilities.len();
    rule_mana(object, printed)
        .into_iter()
        .find(|mana| mana.options.iter().any(|option| option.source == source))
        .map(|mana| mana.shape)
}

fn sorcery_timing(state: &GameState, player: PlayerId) -> bool {
    state.turn().active() == player && state.turn().step().is_main() && state.stack().is_empty()
}

fn hand_land(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|o| o.base_types().contains(CardType::Land))
}

/// Whether the ability `index` of the permanent `object` could be activated by `player` now.
///
/// The engine pays `{T}` and the sacrifice of the source for an ability; `Caps` keeps abilities with other costs out of
/// the game.
fn can_activate(state: &GameState, player: PlayerId, object: &GameObject, index: usize) -> bool {
    let Some(rules) = object.card().rules.get(index) else {
        return false;
    };
    let Ability::Activated {
        cost,
        timing,
        limit,
        ..
    } = &**rules
    else {
        return false;
    };
    if *limit == Some(Limit::OncePerTurn) && state.limit_spent(object.reference(), index) {
        return false;
    }
    if rules.is_mana_ability()
        || !cost
            .0
            .iter()
            .all(|item| matches!(item, CostItem::Tap | CostItem::SacrificeSelf))
    {
        return false;
    }
    if *timing == Timing::Sorcery && !sorcery_timing(state, player) {
        return false;
    }
    if cost.0.contains(&CostItem::Tap) {
        let Some(permanent) = object.permanent() else {
            return false;
        };
        // Only creatures are subject to summoning sickness for {T} abilities (CR 302.6).
        let sick = keywords::summoning_sick(state, object.id())
            && characteristics(state, object.id()).is_some_and(|c| c.is_creature());
        if permanent.tapped() || sick {
            return false;
        }
    }
    announceable(state, player, object.id(), rules)
}

/// Activated abilities of `player`'s permanents that are not mana abilities and could be used now.
pub(crate) fn other_abilities(state: &GameState, player: PlayerId) -> Vec<AbilityRef> {
    let mut found = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if object.controller() != player {
            continue;
        }
        for ability in 0..object.card().rules.len() {
            if can_activate(state, player, object, ability) {
                found.push(AbilityRef {
                    object: id,
                    ability,
                });
            }
        }
    }
    found
}

/// Whether `player` can do anything worth stopping for: play a land, cast a spell or use an ability that is not a mana ability.
///
/// `castable` already holds only the spells the floating mana and the untapped sources can pay for. Mana
/// abilities alone never count: tapping a land with nothing to spend the mana on is not an action.
pub(crate) fn can_act(
    playable_lands: &[ObjectId],
    castable: &[ObjectId],
    abilities: &[AbilityRef],
) -> bool {
    !playable_lands.is_empty() || !castable.is_empty() || !abilities.is_empty()
}

/// Everything `player` may do while holding priority.
pub(crate) fn priority_options(state: &GameState, player: PlayerId) -> PriorityOptions {
    let hand = state.zone(Zone::Hand(player));
    let plain_mana = mana_options(state, player);
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
            // The cost the engine pays has no {X} in it, so paying it would be casting for less.
            if object.card().x_in_cost > 0 {
                return false;
            }
            let timing = types.contains(CardType::Instant)
                || keywords::grants_instant_timing(state, *id)
                || sorcery_timing(state, player);
            // CR 601.2c: a spell that needs a target is cast only when it has a legal one; CR 700.2a: a modal
            // spell only when enough of its modes can be chosen.
            timing
                && spell_ability(object).is_none_or(|rules| announceable(state, player, *id, rules))
                && can_afford(cost, &pool, &plain_mana, life)
        })
        .collect();
    let abilities = other_abilities(state, player);
    let mut mana_abilities = plain_mana;
    mana_abilities.extend(rule_mana_options(state, player));
    PriorityOptions {
        can_act: can_act(&playable_lands, &castable, &abilities),
        playable_lands,
        castable,
        mana_abilities,
        abilities,
        can_pass: true,
    }
}

/// Validates a priority action and turns it into events (or a cast in progress).
pub(crate) fn answer(
    state: &GameState,
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
        PriorityAction::Activate(ability) => {
            if !options.abilities.contains(&ability) {
                return Err(IllegalAnswer::NotOffered("ability").into());
            }
            Ok(Outcome::BeginActivation(ability))
        }
        PriorityAction::ActivateMana(source) => {
            let option = options
                .mana_abilities
                .iter()
                .find(|o| o.source == source)
                .ok_or(IllegalAnswer::NotOffered("mana ability"))?;
            let mut events = Vec::new();
            let costs = rule_mana_costs(state, source);
            if costs.is_none_or(|costs| costs.tap) {
                events.push(Event::PermanentTapped {
                    object: source.object,
                });
            }
            if costs.is_some_and(|costs| costs.sacrifice)
                && let Some(owner) = state.object(source.object).map(GameObject::owner)
            {
                events.push(Event::ZoneChanged {
                    object: source.object,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(owner),
                    cause: ZoneCause::Sacrificed,
                });
            }
            events.push(Event::ManaAdded {
                player,
                mana: option.produces,
                source: Some(source.object),
            });
            Ok(Outcome::Events(events))
        }
    }
}
