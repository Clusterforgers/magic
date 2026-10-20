//! Priority: who may act, what they may do, and when a round of passes is complete.

use std::collections::BTreeSet;

use mtg_cards::AbilityDef;
use mtg_cards::ability::{Ability, Limit, Timing};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::AbilityRef;
use crate::answers::Outcome;
use crate::characteristics::characteristics;
use crate::choose::announceable;
use crate::decision::{PriorityAction, PriorityOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::keywords;
use crate::mana::{ManaOption, ManaSource, can_afford};
use crate::object::GameObject;
use crate::player::Player;
use crate::price::Price;
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
/// The engine pays `{T}` and mana for an ability (`Price`) and nothing else; `Caps` keeps abilities with other costs
/// out of the game. The mana must be affordable with the floating mana and the other permanents the player taps.
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
    let Ok(price) = Price::of(cost) else {
        return false;
    };
    if rules.is_mana_ability() {
        return false;
    }
    if *timing == Timing::Sorcery && !sorcery_timing(state, player) {
        return false;
    }
    if price.tap {
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
    if let Some(mana) = &price.mana {
        let Some(seat) = state.player(player) else {
            return false;
        };
        let sources = price.sources(state, player, object.id());
        if !can_afford(mana, seat.mana(), &sources, seat.life()) {
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
                && can_afford(cost, &pool, &mana_abilities, life)
        })
        .collect();
    let abilities = other_abilities(state, player);
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

#[cfg(test)]
mod tests {
    //! Goal: an activated ability is offered when its whole price can be paid now: a `{T}` needs an untapped source, mana needs floating mana or untapped mana sources (the source of an ability that taps it is none of them), and an `{X}` or a part the engine does not pay is never offered.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::definition::ManaAbility;
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::fold::apply_event;

    fn gadget(cost: &str, makes_mana: bool) -> Arc<CardDefinition> {
        let rules = format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"Draw":{{}}}}]}}}}"#);
        let card = CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&rules).unwrap());
        Arc::new(if makes_mana {
            card.with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Colorless,
            }))
        } else {
            card
        })
    }

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    fn offered(game: &GameState) -> bool {
        !other_abilities(game, PlayerId(0)).is_empty()
    }

    #[rstest]
    #[case::free("[]", 0, true)]
    #[case::a_tap("[\"Tap\"]", 0, true)]
    #[case::mana_without_a_land("\"{1}\"", 0, false)]
    #[case::mana_with_a_land("\"{1}\"", 1, true)]
    #[case::two_mana_with_one_land("\"{2}\"", 1, false)]
    #[case::two_mana_with_two_lands("\"{2}\"", 2, true)]
    #[case::a_colour_the_lands_do_not_make("\"{R}\"", 3, false)]
    #[case::mana_and_a_tap("[{\"Mana\":\"{1}\"},\"Tap\"]", 1, true)]
    #[case::mana_and_a_tap_without_a_land("[{\"Mana\":\"{1}\"},\"Tap\"]", 0, false)]
    #[case::x("[{\"Mana\":\"{X}\"}]", 3, false)]
    #[case::life_as_well("[{\"Mana\":\"{1}\"},{\"PayLife\":1}]", 3, false)]
    fn an_ability_is_offered_when_its_price_can_be_paid(
        #[case] cost: &str,
        #[case] lands: usize,
        #[case] expected: bool,
    ) {
        let mut cards = vec![gadget(cost, false)];
        cards.extend(vec![forest(); lands]);
        let game = state(vec![vec![], vec![]], vec![cards, vec![]]);
        assert_eq!(offered(&game), expected);
    }

    #[rstest]
    #[case::taps_itself_for_its_own_ability("\"{1}\"", true)]
    #[case::cannot_tap_twice("[{\"Mana\":\"{1}\"},\"Tap\"]", false)]
    fn the_source_pays_for_its_ability_only_when_it_is_not_tapped_for_the_cost(
        #[case] cost: &str,
        #[case] expected: bool,
    ) {
        let game = state(vec![vec![], vec![]], vec![vec![gadget(cost, true)], vec![]]);
        assert_eq!(offered(&game), expected);
    }

    #[test]
    fn floating_mana_pays_and_a_tapped_source_cannot_tap() {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![gadget("[{\"Mana\":\"{1}\"},\"Tap\"]", false)], vec![]],
        );
        assert!(!offered(&game));
        apply_event(
            &mut game,
            &Event::ManaAdded {
                player: PlayerId(0),
                mana: ManaType::Red,
                source: None,
            },
        )
        .unwrap();
        assert!(offered(&game));
        let gadget = game.zone(Zone::Battlefield)[0];
        apply_event(&mut game, &Event::PermanentTapped { object: gadget }).unwrap();
        assert!(!offered(&game));
    }

    #[test]
    fn the_other_seat_is_not_offered_the_ability() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![gadget("[]", false)], vec![forest()]],
        );
        assert_eq!(
            other_abilities(&game, PlayerId(1)),
            Vec::<AbilityRef>::new()
        );
    }
}
