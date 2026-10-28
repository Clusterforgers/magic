//! Mana abilities (CR 605): the mana a player may make now, and the events of making it.
//!
//! A mana ability uses no stack. A plain `{T}: Add {X}` is a printed ability of the card (`AbilityDef::Mana`); any
//! other is in the rules of the card and read by `mana_shape`: its cost is `{T}`, sacrificing its source and fixed
//! amounts of life, energy and counters of the source; it may hold an activation condition; it adds mana (some number
//! of times, counted when it is offered) and runs effects that ask nothing. Every way to use one is a [`ManaOption`],
//! offered at priority and to pay for a spell or an ability.

use std::sync::Arc;

use mtg_cards::AbilityDef;
use mtg_cards::ability::Ability;
use mtg_cards::assemble::{ManaProduced, mana_shape};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;

use crate::abilities::AbilityRef;
use crate::ability_reader::abilities_of;
use crate::characteristics::{characteristics, controller_of_object};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::keywords;
use crate::mana::{ManaOption, ManaSource, Take};
use crate::mana_bonus;
use crate::mana_kinds::{COLORS, combinations_of, identity_types, mana_of, producible_types};
use crate::object::{Chosen, PermanentState};
use crate::parts::{Kind, Part, Payer, energy};
use crate::resolve::{Scope, run_inline};
use crate::state::GameState;
use crate::zones::Zone;

/// The most ways one ability makes its mana: one for each type, one for each list of a choice among lists and one for
/// each combination of mana (`mana_shape` refuses an ability with more).
const VARIANTS: usize = mtg_cards::assemble::MANA_COMBINATIONS;

/// A mana ability from the rules of a permanent, with the option that stands for each way to use it.
struct RuleMana {
    /// The ability.
    body: Arc<Ability>,
    /// Using it taps the source.
    tap: bool,
    /// Its address among the abilities of the object, which records a use once each turn.
    address: AbilityRef,
    /// It can be used once each turn.
    once: bool,
    /// One option for each type of mana it can make: one for a fixed type, one for each colour for "any color".
    options: Vec<ManaOption>,
}

/// The parts of the cost of `ability` besides `{T}` and the sacrifice, as the engine pays them.
fn parts_of(pays: &[&mtg_cards::cost::CostItem]) -> Vec<Part> {
    pays.iter()
        .filter_map(|item| Part::read(item).ok().flatten())
        .collect()
}

/// The parts of a cost that take other permanents of the payer, each with the permanents it may take now: the ones that
/// cannot attack come first when tapping, the order of the battlefield otherwise.
fn takes_of(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    barred: &[ObjectId],
    picks: &[&mtg_cards::cost::CostItem],
) -> Vec<Take> {
    let owner = Payer {
        state,
        player,
        source,
        barred,
    };
    picks
        .iter()
        .filter_map(|item| match Part::read(item).ok().flatten()? {
            Part::Pick(pick) => {
                let sacrifice = pick.kind == Kind::Sacrifice;
                let exile = pick.kind == Kind::ExileHand;
                let mut from = pick.candidates(&owner);
                if sacrifice || exile {
                    // The cheapest goes first, a land after the spells; the order of the zone settles a tie.
                    from.sort_by_key(|id| {
                        state.object(*id).map_or((false, 0), |object| {
                            (
                                exile && object.card().types.contains(&CardType::Land),
                                object.card().mana_value(),
                            )
                        })
                    });
                } else {
                    from.sort_by_key(|id| !keywords::summoning_sick(state, *id));
                }
                Some(Take {
                    sacrifice,
                    exile,
                    count: u32::try_from(pick.count).unwrap_or(u32::MAX),
                    from,
                })
            }
            _ => None,
        })
        .collect()
}

/// The life and the energy the parts of a cost pay.
fn paid_by(parts: &[Part]) -> (u32, u32) {
    parts
        .iter()
        .fold((0, 0), |(life, energy), part| match part {
            Part::Life(amount) => (life + amount, energy),
            Part::Energy(amount) => (life, energy + amount),
            _ => (life, energy),
        })
}

/// The mana of one addition split into the type the option names, how many of it and the rest.
pub(crate) fn split(made: &[ManaType]) -> Option<(ManaType, u32, Vec<ManaType>)> {
    let (&first, _) = made.split_first()?;
    let run = made.iter().take_while(|mana| **mana == first).count();
    Some((
        first,
        u32::try_from(run).unwrap_or(u32::MAX),
        made[run..].to_vec(),
    ))
}

/// The mana abilities among the abilities of `id` that the engine activates at priority (`mana_shape`), for an object
/// with `printed` plain mana abilities. The number of mana is counted now, so Gaea's Cradle offers as many as there are
/// creatures; an ability whose condition does not hold or whose cost cannot be paid is not offered.
///
/// The source of the `variant`th option of the ability at position `place` among those the reader lists is numbered
/// `printed + place * 6 + variant`: the printed abilities come first and each ability has a slot for every type, so
/// every type is a distinct source of one permanent and a client taps for the type it wants.
// One pass over the shape of each ability: its condition, limit, cost, the mana it adds and what it does besides.
#[allow(clippy::too_many_lines)]
fn rule_mana(state: &GameState, player: PlayerId, id: ObjectId, printed: usize) -> Vec<RuleMana> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    let scope = Scope::announcing(state, player, object.reference());
    let mut found = Vec::new();
    'abilities: for (place, (address, rules)) in abilities_of(state, id).into_iter().enumerate() {
        let Some(shape) = mana_shape(&rules) else {
            continue;
        };
        // The mana is counted after what the ability does first (a counter put on the source).
        let ahead = c8_time_ahead(state, player, object, shape.before);
        let ahead_scope = ahead
            .as_ref()
            .map(|after| Scope::announcing(after, player, object.reference()));
        let counting = ahead_scope.as_ref().unwrap_or(&scope);
        if let Some(condition) = shape.condition
            && !scope.holds(condition).unwrap_or(false)
        {
            continue;
        }
        if shape.once && state.limit_spent(object.reference(), address.source, address.ability) {
            continue;
        }
        let pays = parts_of(&shape.pays);
        let owner = Payer {
            state,
            player,
            source: id,
            barred: &[],
        };
        if pays.iter().any(|part| part.payable(&owner, &[]).is_none()) {
            continue;
        }
        // What the cost takes: the source is not offered to itself when the cost taps or loses it.
        let barred = if shape.tap || shape.sacrifice {
            vec![id]
        } else {
            Vec::new()
        };
        let takes = takes_of(state, player, id, &barred, &shape.picks);
        if takes
            .iter()
            .any(|take| take.from.len() < take.count as usize)
        {
            continue;
        }
        let (life, energy) = paid_by(&pays);
        // A cost that removes X counters pays any number up to what the source holds; each is a way of its own.
        let x_most = pays.iter().filter_map(|part| part.most(&owner)).min();
        let times = counting
            .amount(shape.times)
            .ok()
            .and_then(|times| u32::try_from(times).ok())
            .unwrap_or(0);
        // "Add {G}. If you control four or more creatures, add {G}{G} instead.": the branch that holds now.
        let mut produces = shape.produces;
        while let ManaProduced::Instead {
            base,
            when,
            instead,
        } = produces
        {
            let branch = if scope.holds(when).unwrap_or(false) {
                instead
            } else {
                base
            };
            let Some(next) = ManaProduced::of(branch) else {
                continue 'abilities;
            };
            produces = next;
        }
        // Each way to add the mana, as the types one addition makes.
        let additions: Vec<Vec<ManaType>> = match produces {
            ManaProduced::Fixed { mana, count } => vec![vec![mana; count as usize]],
            ManaProduced::Mixed(list) => vec![list.to_vec()],
            ManaProduced::AnyColor(count) => {
                // "Remove X +1/+1 counters from this creature: Add X mana of any one color.": every X the cost can
                // pay now, fewest mana first, in each color.
                let counts: Vec<usize> = match x_most {
                    Some(most) => (1..=usize::try_from(most).unwrap_or(0).min(VARIANTS)).collect(),
                    None => vec![
                        counting
                            .amount(count)
                            .ok()
                            .and_then(|count| usize::try_from(count).ok())
                            .unwrap_or(0),
                    ],
                };
                counts
                    .into_iter()
                    .flat_map(|count| COLORS.iter().map(move |c| vec![*c; count]))
                    .collect()
            }
            ManaProduced::Instead { .. } => Vec::new(),
            ManaProduced::OneOf(types) => types.iter().map(|t| vec![*t]).collect(),
            ManaProduced::AnyCombination { count, among } => {
                let counts: Vec<u32> = match x_most {
                    // Every X the cost can pay now, none for X = 0.
                    Some(most) => {
                        (1..=most.min(u32::try_from(VARIANTS).unwrap_or(u32::MAX))).collect()
                    }
                    None => vec![
                        counting
                            .amount(count)
                            .ok()
                            .and_then(|count| u32::try_from(count).ok())
                            .unwrap_or(0),
                    ],
                };
                combinations_of(among, counts, VARIANTS)
            }
            ManaProduced::OneOfLists(lists) => lists.to_vec(),
            ManaProduced::ChosenColor(count) => {
                match object.permanent().and_then(PermanentState::chosen) {
                    Some(Chosen::Color(color)) => vec![vec![mana_of(*color); count as usize]],
                    _ => Vec::new(),
                }
            }
            ManaProduced::OrChosenColor(printed) => {
                let mut types = vec![printed];
                if let Some(Chosen::Color(color)) =
                    object.permanent().and_then(PermanentState::chosen)
                    && mana_of(*color) != printed
                {
                    types.push(mana_of(*color));
                }
                types.into_iter().map(|t| vec![t]).collect()
            }
            ManaProduced::CommanderIdentity(count) => identity_types(state, player)
                .into_iter()
                .map(|t| vec![t; count as usize])
                .collect(),
            ManaProduced::ProducedBy {
                among,
                count,
                colorless,
            } => producible_types(state, player, (id, Some(place)), among, colorless)
                .into_iter()
                .map(|t| vec![t; count as usize])
                .collect(),
        };
        let (tap, sacrifice, extra) = (
            shape.tap,
            shape.sacrifice,
            !shape.then.is_empty()
                || !shape.before.is_empty()
                || !pays.is_empty()
                || shape.once
                || !takes.is_empty(),
        );
        // "of the chosen type" is the type the source chose; without one the mana could pay for nothing.
        let chosen = match object.permanent().and_then(PermanentState::chosen) {
            Some(Chosen::CreatureType(subtype)) => Some(subtype),
            _ => None,
        };
        let Some(only) = mtg_cards::spend::resolve_uses(shape.only, chosen) else {
            continue;
        };
        let once = shape.once;
        // Every use takes others, so what limits the uses is what the cost takes.
        let repeat = !tap
            && !sacrifice
            && !once
            && !takes.is_empty()
            && shape.pays.is_empty()
            && shape.mana.symbols().is_empty()
            && shape.condition.is_none()
            && shape.times.is_one();
        let options = additions
            .into_iter()
            .take(VARIANTS)
            .enumerate()
            .filter_map(|(variant, addition)| {
                let made: Vec<ManaType> = (0..times).flat_map(|_| addition.clone()).collect();
                let (produces, count, also) = split(&made)?;
                Some(ManaOption {
                    source: ManaSource {
                        object: id,
                        ability: printed + place * VARIANTS + variant,
                    },
                    produces,
                    count,
                    sacrifice,
                    extra,
                    tapless: !tap,
                    only: only.clone(),
                    also,
                    life,
                    energy,
                    energy_held: if energy > 0 {
                        energy::held(state, player)
                    } else {
                        0
                    },
                    cost: shape.mana.clone(),
                    takes: takes.clone(),
                    repeat,
                })
            })
            .collect();
        found.push(RuleMana {
            body: rules,
            tap,
            address,
            once,
            options,
        });
    }
    found
}

/// The mana abilities `player` can activate now: the printed ones of an untapped permanent they control and the
/// ones in its rules whose cost can be paid. A creature that came under their control this turn cannot tap (CR 302.6).
pub(crate) fn options(state: &GameState, player: PlayerId) -> Vec<ManaOption> {
    let mut found = Vec::new();
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
        let sick = chars.is_creature() && keywords::summoning_sick(state, id);
        let can_tap = !perm.tapped() && !sick;
        if can_tap {
            for (ability, def) in chars.abilities.iter().enumerate() {
                let AbilityDef::Mana(mana) = def;
                found.push(ManaOption::new(
                    ManaSource {
                        object: id,
                        ability,
                    },
                    mana.produces,
                ));
            }
        }
        for mana in rule_mana(state, player, id, chars.abilities.len()) {
            if !mana.tap || can_tap {
                found.extend(mana.options);
            }
        }
    }
    mana_bonus::fold(state, player, found)
}

/// The state after the effects a mana ability runs before its mana (CR 605.3b: in the order written), or `None` when
/// it runs none or they cannot run now.
fn c8_time_ahead(
    state: &GameState,
    player: PlayerId,
    object: &crate::object::GameObject,
    before: &[mtg_cards::effect::Effect],
) -> Option<GameState> {
    if before.is_empty() {
        return None;
    }
    let mut after = state.clone();
    for event in run_inline(state, player, object, before).ok()? {
        crate::fold::apply_event(&mut after, &event).ok()?;
    }
    Some(after)
}

/// The events of `player` using the mana ability `option` stands for: the payment of its cost (life, energy, counters,
/// the tap and the sacrifice), the mana added to their pool, and what it does besides (a painland's damage, an egg's
/// draw), all at once (CR 605.3b).
pub(crate) fn activation(
    state: &GameState,
    player: PlayerId,
    option: &ManaOption,
    taken: &[Vec<ObjectId>],
) -> Result<Vec<Event>, EngineError> {
    let id = option.source.object;
    let object = state
        .object(id)
        .ok_or_else(|| EngineError::InvalidEvent("a mana ability of nothing".into()))?;
    let printed = characteristics(state, id).map_or(0, |chars| chars.abilities.len());
    // The variants a triggered mana ability's pick makes of an option are numbered past the ability's own sources.
    let base = option.source.ability % mana_bonus::STRIDE;
    let rule = rule_mana(state, player, id, printed)
        .into_iter()
        .find(|mana| {
            mana.options
                .iter()
                .any(|o| o.source.object == id && o.source.ability == base)
        });
    let shape = rule.as_ref().and_then(|mana| mana_shape(&mana.body));
    // The X a cost counts is the mana the way adds, without what a triggered mana ability adds to it.
    let x = rule
        .iter()
        .flat_map(|mana| &mana.options)
        .find(|o| o.source.object == id && o.source.ability == base)
        .map_or(0, |o| u32::try_from(o.made().len()).unwrap_or(0));
    let mut events = Vec::new();
    if let Some(rule) = rule.as_ref().filter(|rule| rule.once) {
        // A use is logged for the limit; no ability goes on the stack.
        events.push(Event::AbilityActivated {
            player,
            source: object.reference(),
            ability: rule.address.ability,
            origin: rule.address.source,
        });
    }
    if let Some(shape) = &shape {
        for part in parts_of(&shape.pays) {
            events.extend(part.at(x).events(state, player, id, &[]));
        }
        let picks = shape
            .picks
            .iter()
            .filter_map(|item| Part::read(item).ok().flatten());
        for (part, objects) in picks.zip(taken) {
            events.extend(part.events(state, player, id, objects));
        }
    }
    if shape.as_ref().is_none_or(|shape| shape.tap) {
        events.push(Event::PermanentTapped { object: id });
    }
    if shape.as_ref().is_some_and(|shape| shape.sacrifice) {
        events.push(Event::ZoneChanged {
            object: id,
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::Sacrificed,
        });
    }
    if let Some(shape) = &shape {
        events.extend(run_inline(state, player, object, shape.before)?);
    }
    events.extend(option.made().into_iter().map(|mana| Event::ManaAdded {
        player,
        mana,
        source: Some(id),
        only: option.only.clone(),
        lasts: mtg_cards::spend::ManaLasts::Step,
    }));
    if let Some(shape) = shape {
        events.extend(run_inline(state, player, object, shape.then)?);
    }
    Ok(events)
}

#[cfg(test)]
mod combos;
#[cfg(test)]
mod counted;
#[cfg(test)]
mod c8_time_counted;
#[cfg(test)]
mod replaced;

#[cfg(test)]
mod tests {
    //! Goal: a mana ability in the rules of a permanent is offered as one source for each type of mana it can add, only while its cost can be paid, with the number of mana counted now; using it taps, sacrifices, adds that many mana and runs what else it does (a painland's damage, an egg's draw) in one batch without the stack, and the plain tap-for-mana of a land is unchanged.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::answers::Outcome;
    use crate::combat::DamageTarget;
    use crate::decision::PriorityAction;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::priority::{answer, priority_options};

    const ME: PlayerId = PlayerId(0);

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    const TREASURE: &str = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;
    const PETAL: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#;
    const BATTERY: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#;
    const SOL_RING: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#;
    const CRADLE: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#;
    const PAINLAND: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}},{"DealDamage":{"to":"You","amount":1}}]}}"#;
    const EGG: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}},{"Draw":{}}]}}"#;

    fn artifact(name: &str, json: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Artifact)
                .with_rules(ability(json)),
        )
    }

    fn spark() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Spark")
                .with_type(CardType::Instant)
                .with_mana_cost(ManaCost::parse("{R}").unwrap()),
        )
    }

    fn game(permanent: Arc<CardDefinition>) -> GameState {
        state(vec![vec![spark()], vec![]], vec![vec![permanent], vec![]])
    }

    fn produced(options: &[ManaOption]) -> Vec<(ManaType, u32)> {
        options.iter().map(|o| (o.produces, o.count)).collect()
    }

    #[test]
    fn a_treasure_is_one_source_for_each_color() {
        let game = game(artifact("Treasure", TREASURE));
        let found = options(&game, ME);
        assert_eq!(
            produced(&found),
            COLORS.iter().map(|c| (*c, 1)).collect::<Vec<_>>()
        );
        assert!(found.iter().all(|o| o.sacrifice));
        let id = game.zone(Zone::Battlefield)[0];
        assert!(found.iter().all(|o| o.source.object == id));
        let mut keys: Vec<usize> = found.iter().map(|o| o.source.ability).collect();
        keys.dedup();
        assert_eq!(keys.len(), 5, "every colour has its own source");
        assert!(
            options(&game, PlayerId(1)).is_empty(),
            "only its controller's"
        );
    }

    #[rstest]
    #[case::treasure_that_is_tapped(TREASURE, true, 0)]
    #[case::treasure_that_is_ready(TREASURE, false, 5)]
    #[case::a_tap_without_a_sacrifice_that_is_tapped(BATTERY, true, 0)]
    #[case::a_sacrifice_needs_no_untapped_source(PETAL, true, 1)]
    #[case::two_mana_from_a_tap(SOL_RING, false, 1)]
    #[case::a_choice_of_two(PAINLAND, false, 2)]
    #[case::a_choice_of_two_when_tapped(PAINLAND, true, 0)]
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
        assert_eq!(options(&game, ME).len(), offered);
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
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(ability(json)),
        );
        let entered = crate::tokens::TokenCreated {
            object: game.next_object(),
            copy_of: None,
            controller: ME,
            name: "Gem Elf".into(),
            tapped: false,
            attacking: None,
            definition: gem,
        };
        apply_event(&mut game, &Event::TokenCreated(entered)).unwrap();
        assert_eq!(options(&game, ME).len(), offered);
    }

    fn activate(game: &GameState, colour: usize) -> Vec<Event> {
        let offered = priority_options(game, ME);
        let source = offered.mana_abilities[colour].source;
        let Outcome::Events(events) =
            answer(game, ME, &offered, PriorityAction::ActivateMana(source)).unwrap()
        else {
            panic!("a mana ability is not a cast");
        };
        events
    }

    fn added(events: &[Event]) -> Vec<ManaType> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::ManaAdded { mana, .. } => Some(*mana),
                _ => None,
            })
            .collect()
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
                    source: Some(id),
                    only: Vec::new(),
                    lasts: mtg_cards::spend::ManaLasts::Step,
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
        assert_eq!(added(&events), [ManaType::Red]);
    }

    #[test]
    fn a_land_still_taps_for_its_mana_and_is_not_sacrificed() {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![Arc::new(CardDefinition::basic_land(Some(Color::Green)))],
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
                    source: Some(id),
                    only: Vec::new(),
                    lasts: mtg_cards::spend::ManaLasts::Step,
                },
            ]
        );
    }

    #[test]
    fn a_source_that_is_not_offered_is_refused() {
        let game = game(artifact("Treasure", TREASURE));
        let offered = priority_options(&game, ME);
        let id = game.zone(Zone::Battlefield)[0];
        let unknown = ManaSource {
            object: id,
            ability: 99,
        };
        let error = answer(&game, ME, &offered, PriorityAction::ActivateMana(unknown));
        assert!(error.is_err());
    }

    #[test]
    fn a_sol_ring_adds_two_colorless_mana_for_one_tap() {
        let game = game(artifact("Sol Ring", SOL_RING));
        let id = game.zone(Zone::Battlefield)[0];
        let found = options(&game, ME);
        assert_eq!(produced(&found), [(ManaType::Colorless, 2)]);
        let events = activate(&game, 0);
        assert_eq!(events[0], Event::PermanentTapped { object: id });
        assert_eq!(added(&events), [ManaType::Colorless; 2]);
    }

    #[rstest]
    #[case::no_creature(0)]
    #[case::one(1)]
    #[case::four(4)]
    fn a_cradle_adds_a_mana_for_each_creature_its_controller_has(#[case] creatures: usize) {
        let mut battlefield = vec![artifact("Cradle", CRADLE)];
        battlefield.extend((0..creatures).map(|n| creature(&format!("Elf {n}"), 1, 1)));
        let game = state(
            vec![vec![], vec![]],
            vec![battlefield, vec![creature("Foe", 2, 2)]],
        );
        let found = options(&game, ME);
        if creatures == 0 {
            assert!(found.is_empty(), "a source of nothing is not offered");
            return;
        }
        assert_eq!(
            produced(&found),
            [(ManaType::Green, u32::try_from(creatures).unwrap())]
        );
        assert_eq!(added(&activate(&game, 0)), vec![ManaType::Green; creatures]);
    }

    #[test]
    fn a_painland_adds_a_color_of_two_and_deals_one_damage_to_its_controller() {
        let game = game(artifact("Painland", PAINLAND));
        let id = game.zone(Zone::Battlefield)[0];
        let found = options(&game, ME);
        assert_eq!(
            produced(&found),
            [(ManaType::White, 1), (ManaType::Blue, 1)]
        );
        let events = activate(&game, 1);
        assert_eq!(events[0], Event::PermanentTapped { object: id });
        assert_eq!(added(&events), [ManaType::Blue]);
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::DamageDealt { source, target: DamageTarget::Player(player), amount: 1, .. }
                    if *source == id && *player == ME
            )),
            "{events:?}"
        );
    }

    #[test]
    fn an_egg_adds_its_mana_and_draws_after_it_is_sacrificed() {
        let game = game(artifact("Egg", EGG));
        let id = game.zone(Zone::Battlefield)[0];
        let events = activate(&game, 2);
        assert!(matches!(
            events[0],
            Event::ZoneChanged { object, cause: ZoneCause::Sacrificed, .. } if object == id
        ));
        assert_eq!(added(&events), [ManaType::Black]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::CardDrawn { player, .. } if *player == ME)),
            "{events:?}"
        );
    }

    #[test]
    fn the_color_chosen_as_a_permanent_entered_is_the_mana_it_adds() {
        let json =
            r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ChosenColor":2}}}]}}"#;
        let mut game = game(artifact("Pillar", json));
        let id = game.zone(Zone::Battlefield)[0];
        assert!(options(&game, ME).is_empty(), "no colour was chosen");
        let entered = Event::Entered {
            object: id,
            tapped: false,
            controller: ME,
            chosen: Some(Chosen::Color(Color::Blue)),
        };
        apply_event(&mut game, &entered).unwrap();
        assert_eq!(produced(&options(&game, ME)), [(ManaType::Blue, 2)]);
    }

    const BLOOM: &str = r#"{"Activated":{"cost":[{"ExileFromHand":{"filter":[]}}],"effects":[{"AddMana":{"mana":{"OneOfLists":[["Black","Black"],["Green","Green"]]}}}]}}"#;

    fn land() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Swamp").with_type(CardType::Land))
    }

    /// A game where the bloom is in play and `hand` is in seat 0's hand.
    fn bloom_game(hand: Vec<Arc<CardDefinition>>) -> GameState {
        state(
            vec![hand, vec![]],
            vec![vec![artifact("Bloom", BLOOM)], vec![]],
        )
    }

    #[rstest]
    #[case::no_card_to_exile(0, 0)]
    #[case::one_card(1, 2)]
    #[case::two_cards(2, 2)]
    fn a_source_that_exiles_a_card_is_offered_for_each_list_only_with_a_card_in_the_hand(
        #[case] cards: usize,
        #[case] offered: usize,
    ) {
        let found = options(&bloom_game((0..cards).map(|_| spark()).collect()), ME);
        assert_eq!(found.len(), offered);
        for option in &found {
            let [take] = option.takes.as_slice() else {
                panic!("one part exiles a card: {:?}", option.takes);
            };
            assert!(take.exile && !take.sacrifice);
            assert_eq!((take.count as usize, take.from.len()), (1, cards));
            assert_eq!(option.count, 2, "one list of two mana for one activation");
            assert!(option.extra && option.tapless);
        }
        assert_eq!(
            found.iter().map(|o| o.produces).collect::<Vec<_>>(),
            [ManaType::Black, ManaType::Green][..offered]
        );
    }

    #[test]
    fn the_cheapest_spell_is_offered_first_and_a_land_last() {
        let costly = Arc::new(
            CardDefinition::new("Costly")
                .with_type(CardType::Sorcery)
                .with_mana_cost(ManaCost::parse("{4}{R}").unwrap()),
        );
        let game = bloom_game(vec![land(), costly, spark()]);
        let hand = game.zone(Zone::Hand(ME)).to_vec();
        let found = options(&game, ME);
        assert_eq!(found[0].takes[0].from, [hand[2], hand[1], hand[0]]);
    }

    #[rstest]
    #[case::a_card_beside_the_spell(2, true)]
    #[case::the_spell_alone(1, false)]
    fn a_payment_for_a_spell_never_exiles_that_spell(
        #[case] cards: usize,
        #[case] affordable: bool,
    ) {
        let game = bloom_game((0..cards).map(|_| spark()).collect());
        let spell = game.zone(Zone::Hand(ME))[0];
        let all = options(&game, ME);
        let sources = crate::mana_takes::for_card(&game, &all, spell);
        assert!(sources.iter().all(|o| !o.takes[0].from.contains(&spell)));
        let cost = ManaCost::parse("{B}{B}").unwrap();
        let pool = crate::mana::ManaPool::default();
        let seen = |sources: &[ManaOption]| {
            crate::mana::can_afford(&cost, &pool, sources, 20, &crate::spend::Spend::other())
        };
        assert_eq!(seen(&sources), affordable);
        // A payment that is not for a card in the hand may exile any card in it.
        assert!(seen(&all));
    }

    #[test]
    fn the_sources_of_a_card_that_is_not_in_a_hand_are_unchanged() {
        let game = bloom_game(vec![spark()]);
        let all = options(&game, ME);
        let bloom = game.zone(Zone::Battlefield)[0];
        assert!(matches!(
            crate::mana_takes::for_card(&game, &all, bloom),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn a_treasure_and_a_sol_ring_pay_for_a_spell_the_lands_cannot() {
        let game = state(
            vec![vec![spark()], vec![]],
            vec![vec![artifact("Treasure", TREASURE)], vec![]],
        );
        let offered = priority_options(&game, ME);
        assert_eq!(offered.castable.len(), 1, "a Treasure can pay {{R}}");
        assert!(offered.can_act);
    }
}
