//! Priority: who may act, what they may do, and when a round of passes is complete.

use std::collections::BTreeSet;

use mtg_cards::AbilityDef;
use mtg_cards::ability::{Ability, Limit, Timing};
use mtg_cards::assemble::{ManaProduced, SimpleManaAbility, simple_mana_ability};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::abilities::AbilityRef;
use crate::ability_reader::abilities_of;
use crate::answers::Outcome;
use crate::characteristics::{characteristics, controller_of_object};
use crate::choose::announceable;
use crate::decision::{PriorityAction, PriorityOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::{Event, ZoneCause};
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
        if controller_of_object(state, object) != player || perm.tapped() {
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

/// The mana source for the `colour`th option of the ability at position `place` among those the reader lists for
/// `object`, which has `printed` plain mana abilities: its number follows the printed ones with five for each ability the
/// reader lists, one for each colour it can add, so every colour is a distinct source of one permanent and a client taps
/// for the colour it wants.
const fn rule_source(printed: usize, object: ObjectId, place: usize, colour: usize) -> ManaSource {
    ManaSource {
        object,
        ability: printed + place * COLORS.len() + colour,
    }
}

/// The mana abilities among the abilities of `id` that the engine activates at priority (`simple_mana_ability`), for
/// an object with `printed` plain mana abilities.
fn rule_mana(state: &GameState, id: ObjectId, printed: usize) -> Vec<RuleMana> {
    let mut found = Vec::new();
    for (place, (_, rules)) in abilities_of(state, id).into_iter().enumerate() {
        let Some(shape) = simple_mana_ability(&rules) else {
            continue;
        };
        let source = |colour: usize| rule_source(printed, id, place, colour);
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
        if controller_of_object(state, object) != player {
            continue;
        }
        let Some(chars) = characteristics(state, id) else {
            continue;
        };
        // Only creatures are subject to summoning sickness for {T} abilities (CR 302.6).
        let sick = chars.is_creature() && keywords::summoning_sick(state, id);
        for mana in rule_mana(state, id, chars.abilities.len()) {
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
    let printed = characteristics(state, source.object)?.abilities.len();
    rule_mana(state, source.object, printed)
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

/// Whether the ability `rules`, which `address` names among the abilities of the permanent `object`, could be
/// activated by `player` now.
///
/// The engine pays `{T}`, mana and the sacrifice of the source for an ability (`Price`) and nothing else; `Caps` keeps
/// abilities with other costs out of the game. The mana must be affordable with the floating mana and the other
/// permanents the player taps.
fn can_activate(
    state: &GameState,
    player: PlayerId,
    object: &GameObject,
    address: AbilityRef,
    rules: &Ability,
) -> bool {
    let Ability::Activated {
        cost,
        timing,
        limit,
        ..
    } = rules
    else {
        return false;
    };
    if *limit == Some(Limit::OncePerTurn)
        && state.limit_spent(object.reference(), address.source, address.ability)
    {
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
        if controller_of_object(state, object) != player {
            continue;
        }
        for (address, rules) in abilities_of(state, id) {
            if can_activate(state, player, object, address, &rules) {
                found.push(address);
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
                && spell_ability(state, *id)
                    .is_none_or(|(_, rules)| announceable(state, player, *id, &rules))
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

#[cfg(test)]
mod rule_mana_tests {
    //! Goal: a mana ability in the rules of a permanent (a Treasure) is offered at priority as one source for each colour it can add, only while its cost can be paid, is activated with its costs and its mana in one batch and without the stack, and never counts toward what a spell can be paid with; the plain tap-for-mana of a land is unchanged.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::state;
    use crate::fold::apply_event;

    const ME: PlayerId = PlayerId(0);

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    const TREASURE: &str = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;
    const PETAL: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#;
    const BATTERY: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#;

    fn artifact(name: &str, json: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(mtg_core::card::CardType::Artifact)
                .with_rules(ability(json)),
        )
    }

    fn spark() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Spark")
                .with_type(mtg_core::card::CardType::Instant)
                .with_mana_cost(ManaCost::parse("{R}").unwrap()),
        )
    }

    fn game(permanent: Arc<CardDefinition>) -> GameState {
        state(vec![vec![spark()], vec![]], vec![vec![permanent], vec![]])
    }

    fn produced(options: &[ManaOption]) -> Vec<ManaType> {
        options.iter().map(|o| o.produces).collect()
    }

    #[test]
    fn a_treasure_is_one_source_for_each_color() {
        let game = game(artifact("Treasure", TREASURE));
        let options = rule_mana_options(&game, ME);
        assert_eq!(produced(&options), COLORS);
        let id = game.zone(Zone::Battlefield)[0];
        assert!(options.iter().all(|o| o.source.object == id));
        let mut keys: Vec<usize> = options.iter().map(|o| o.source.ability).collect();
        keys.dedup();
        assert_eq!(keys.len(), 5, "every colour has its own source");
        assert!(
            rule_mana_options(&game, PlayerId(1)).is_empty(),
            "only its controller's"
        );
    }

    #[rstest]
    #[case::treasure_that_is_tapped(TREASURE, true, 0)]
    #[case::treasure_that_is_ready(TREASURE, false, 5)]
    #[case::a_tap_without_a_sacrifice_that_is_tapped(BATTERY, true, 0)]
    #[case::a_sacrifice_needs_no_untapped_source(PETAL, true, 1)]
    fn the_cost_of_the_ability_decides_whether_it_is_offered(
        #[case] json: &str,
        #[case] tapped: bool,
        #[case] offered: usize,
    ) {
        let mut game = game(artifact("Gem", json));
        let id = game.zone(Zone::Battlefield)[0];
        if tapped {
            apply_event(&mut game, &Event::PermanentTapped { object: id }).unwrap();
        }
        assert_eq!(rule_mana_options(&game, ME).len(), offered);
    }

    #[rstest]
    #[case::a_tap_cost_waits_for_the_next_turn(TREASURE, 0)]
    #[case::a_sacrifice_alone_does_not(PETAL, 1)]
    fn a_creature_that_just_entered_is_held_back_by_summoning_sickness_only_for_a_tap_cost(
        #[case] json: &str,
        #[case] offered: usize,
    ) {
        let mut game = game(spark());
        let gem = Arc::new(
            CardDefinition::new("Gem Elf")
                .with_type(mtg_core::card::CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(ability(json)),
        );
        let entered = crate::tokens::TokenCreated {
            object: game.next_object(),
            controller: ME,
            name: "Gem Elf".into(),
            tapped: false,
            attacking: None,
            definition: gem,
        };
        apply_event(&mut game, &Event::TokenCreated(entered)).unwrap();
        assert_eq!(rule_mana_options(&game, ME).len(), offered);
    }

    fn activate(game: &GameState, colour: usize) -> Vec<Event> {
        let options = priority_options(game, ME);
        let source = options.mana_abilities[colour].source;
        let Outcome::Events(events) =
            answer(game, ME, &options, PriorityAction::ActivateMana(source)).unwrap()
        else {
            panic!("a mana ability is not a cast");
        };
        events
    }

    #[rstest]
    #[case::white(0, ManaType::White)]
    #[case::red(3, ManaType::Red)]
    #[case::green(4, ManaType::Green)]
    fn a_treasure_taps_sacrifices_and_adds_the_color_chosen_in_one_batch(
        #[case] colour: usize,
        #[case] mana: ManaType,
    ) {
        let game = game(artifact("Treasure", TREASURE));
        let id = game.zone(Zone::Battlefield)[0];
        assert_eq!(
            activate(&game, colour),
            [
                Event::PermanentTapped { object: id },
                Event::ZoneChanged {
                    object: id,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(ME),
                    cause: ZoneCause::Sacrificed,
                },
                Event::ManaAdded {
                    player: ME,
                    mana,
                    source: Some(id)
                },
            ]
        );
    }

    #[test]
    fn an_ability_without_a_tap_cost_does_not_tap() {
        let game = game(artifact("Petal", PETAL));
        let id = game.zone(Zone::Battlefield)[0];
        let events = activate(&game, 0);
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            Event::ZoneChanged { object, cause: ZoneCause::Sacrificed, .. } if object == id
        ));
        assert!(matches!(
            events[1],
            Event::ManaAdded {
                mana: ManaType::Red,
                ..
            }
        ));
    }

    #[test]
    fn a_land_still_taps_for_its_mana_and_is_not_sacrificed() {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![Arc::new(CardDefinition::basic_land(Some(
                    mtg_core::color::Color::Green,
                )))],
                vec![],
            ],
        );
        let id = game.zone(Zone::Battlefield)[0];
        assert_eq!(
            activate(&game, 0),
            [
                Event::PermanentTapped { object: id },
                Event::ManaAdded {
                    player: ME,
                    mana: ManaType::Green,
                    source: Some(id)
                },
            ]
        );
    }

    #[test]
    fn a_source_that_is_not_offered_is_refused() {
        let game = game(artifact("Treasure", TREASURE));
        let options = priority_options(&game, ME);
        let id = game.zone(Zone::Battlefield)[0];
        let unknown = ManaSource {
            object: id,
            ability: 99,
        };
        let error = answer(&game, ME, &options, PriorityAction::ActivateMana(unknown));
        assert!(error.is_err());
    }

    #[test]
    fn the_mana_of_a_treasure_is_made_first_and_never_counts_toward_a_spell() {
        let game = game(artifact("Treasure", TREASURE));
        let options = priority_options(&game, ME);
        assert_eq!(options.mana_abilities.len(), 5);
        assert!(
            options.castable.is_empty(),
            "{{R}} is not paid by a source the payment cannot sacrifice"
        );
        assert!(!options.can_act, "a mana ability alone is not an action");
    }

    #[test]
    fn a_land_and_a_treasure_are_listed_together_and_the_spell_is_paid_by_the_land() {
        let game = state(
            vec![vec![spark()], vec![]],
            vec![
                vec![
                    Arc::new(CardDefinition::basic_land(Some(
                        mtg_core::color::Color::Red,
                    ))),
                    artifact("Treasure", TREASURE),
                ],
                vec![],
            ],
        );
        let options = priority_options(&game, ME);
        assert_eq!(options.mana_abilities.len(), 6);
        assert_eq!(options.castable.len(), 1);
    }
}
