//! Goal: each house rule for Commander changes its own part of the game and nothing else at 2, 3, 4 and 6 seats: the tax a commander costs for each earlier cast (none to 10, over several casts and deaths), the commander damage that makes a player lose (1 to 100, or none), and the deck check at game creation (the exact size, the singleton rule and its copies, the colour identity, and the commanders a seat starts with), each accepted with its rule and refused without it; the refused setups name their rule; the rules change the state hash and replay; and whole bot games under random rules end with the invariants kept.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation
)]

use crate::common::pid;
use crate::common::seeds::for_each_seed;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::house::{CommanderDamageLimit, DeckRules, HouseRules};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::SetupError;
use mtg_engine::format::{Commander, DeckError};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::player::{LossReason, PlayerStatus};
use mtg_engine::rng::GameRng;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{cost, forest, mountain, wastes};
use mtg_testkit::commander_cards::{commander_deck, legend};
use mtg_testkit::{
    Driver, GameFailure, Scenario, ScriptError, commander_fuzz_setup, play_game, ready,
};
use proptest::prelude::*;
use rstest::rstest;

fn rules(set: impl FnOnce(&mut HouseRules)) -> HouseRules {
    let mut rules = HouseRules::default();
    set(&mut rules);
    rules
}

fn deck_rules(set: impl FnOnce(&mut DeckRules)) -> HouseRules {
    rules(|r| set(&mut r.deck))
}

fn relaxed_singleton(most: Option<u32>) -> HouseRules {
    deck_rules(|d| {
        d.singleton = Some(false);
        d.max_copies = most;
    })
}

fn lethal(amount: u32) -> Option<CommanderDamageLimit> {
    Some(CommanderDamageLimit::Lethal(amount))
}

fn mana(text: &str) -> mtg_core::mana::ManaCost {
    mtg_core::mana::ManaCost::parse(text).unwrap()
}

// The tax.

/// A Commander table where seat 0 has the commander "Cmd" ({1}{R}) in its command zone and `lands` Mountains.
fn table(seats: usize, lands: usize, house: HouseRules) -> Scenario {
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .house_rules(house)
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .battlefield(0, (0..lands).map(|_| ready(mountain())))
}

fn commander(driver: &Driver) -> ObjectId {
    driver.state().commanders().next().unwrap()
}

fn zone_of(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

/// Starts the cast of "Cmd", pays it and lets it resolve at a table of `seats`.
fn cast_and_resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.begin_cast("Cmd")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(zone_of(driver, commander(driver)), Zone::Battlefield);
    Ok(())
}

/// The commander dies and its owner puts it back into the command zone.
fn dies_and_returns(driver: &mut Driver) -> Result<(), ScriptError> {
    let object = commander(driver);
    driver.manual(ManualAction::Move {
        object,
        to: ManualZone::Graveyard,
    })?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert_eq!(zone_of(driver, object), Zone::Command(PlayerId(0)));
    Ok(())
}

#[rstest]
fn each_cast_from_the_command_zone_costs_the_tax_of_the_rule_after_every_death(
    #[values(2, 3, 4, 6)] seats: usize,
    #[values(None, Some(0), Some(1), Some(2), Some(10))] tax: Option<u32>,
) -> Result<(), ScriptError> {
    let per_cast = tax.unwrap_or(2);
    let mut driver = Driver::from_scenario(table(seats, 70, rules(|r| r.commander_tax = tax)))?;
    for earlier in 0..4u32 {
        let id = commander(&driver);
        assert_eq!(driver.state().commander_casts(id), earlier);
        assert_eq!(driver.state().commander_tax(id), earlier * per_cast);
        driver.begin_cast("Cmd")?;
        let generic = 1 + earlier * per_cast;
        assert_eq!(
            driver.pay_options()?.cost,
            mana(&format!("{{{generic}}}{{R}}"))
        );
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        for _ in 0..seats {
            driver.pass()?;
        }
        assert_eq!(zone_of(&driver, id), Zone::Battlefield);
        dies_and_returns(&mut driver)?;
    }
    Ok(())
}

#[rstest]
#[case::none_to_pay(Some(0), 4, true)]
#[case::none_to_pay_but_the_first_cast(Some(0), 3, false)]
#[case::the_formats_two_needs_four_more(None, 5, false)]
#[case::the_formats_two_with_four_more(None, 6, true)]
#[case::ten_needs_twelve(Some(10), 13, false)]
#[case::ten_with_twelve(Some(10), 14, true)]
fn a_commander_is_offered_only_when_the_tax_of_the_rule_can_be_paid(
    #[values(2, 4)] seats: usize,
    #[case] tax: Option<u32>,
    #[case] lands: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, lands, rules(|r| r.commander_tax = tax)))?;
    cast_and_resolve(&mut driver, seats)?;
    dies_and_returns(&mut driver)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority expected");
    };
    assert_eq!(options.castable.contains(&commander(&driver)), offered);
    Ok(())
}

// The commander damage.

/// A table of `seats` where seat 0's commander "Cmd" ({0}, `power`/`power`) attacks seat 1 on seat 0's second turn,
/// every seat starting with 1000 life so that only commander damage can end the game.
fn attacked(
    seats: usize,
    power: i32,
    limit: Option<CommanderDamageLimit>,
) -> Result<Driver, ScriptError> {
    let house = rules(|r| {
        r.starting_life = Some(1000);
        r.commander_damage = limit;
    });
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .house_rules(house)
        .commanders(0, [legend("Cmd", 0, &[], power, power)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::DeclareAttackers)?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(turn, Step::EndCombat).ok();
    Ok(driver)
}

#[rstest]
#[case::one_is_enough_at_one(lethal(1), 1, true)]
#[case::two_at_one(lethal(1), 2, true)]
#[case::the_formats_twenty_one(None, 21, true)]
#[case::the_formats_twenty(None, 20, false)]
#[case::twenty_one_as_given(lethal(21), 21, true)]
#[case::twenty_below_it(lethal(21), 20, false)]
#[case::ten_is_lethal_below_the_formats(lethal(10), 10, true)]
#[case::twenty_one_is_not_enough_for_fifty(lethal(50), 21, false)]
#[case::fifty_is_enough_for_fifty(lethal(50), 50, true)]
#[case::a_hundred(lethal(100), 100, true)]
#[case::ninety_nine_below_a_hundred(lethal(100), 99, false)]
#[case::off_at_the_formats_amount(Some(CommanderDamageLimit::Off), 21, false)]
#[case::off_at_a_hundred(Some(CommanderDamageLimit::Off), 100, false)]
fn a_player_loses_to_the_commander_damage_of_the_rule_and_to_no_less(
    #[values(2, 3, 4, 6)] seats: usize,
    #[case] limit: Option<CommanderDamageLimit>,
    #[case] power: i32,
    #[case] eliminated: bool,
) -> Result<(), ScriptError> {
    let driver = attacked(seats, power, limit)?;
    let foe = driver.state().player(PlayerId(1)).unwrap();
    assert_eq!(foe.life(), 1000 - power);
    let expected = if eliminated {
        PlayerStatus::Lost(LossReason::CommanderDamage)
    } else {
        PlayerStatus::InGame
    };
    assert_eq!(foe.status().clone(), expected);
    // The damage is counted whether or not it is lethal.
    assert_eq!(
        driver
            .state()
            .commander_damage(commander(&driver), PlayerId(1)),
        power.cast_unsigned()
    );
    // The others take no part: a game of two ends with the loss, a longer one goes on without the loser.
    let result = driver.engine().result();
    assert_eq!(result.is_some(), eliminated && seats == 2, "{result:?}");
    Ok(())
}

// The deck check at game creation.

fn cmd(name: &str) -> Arc<CardDefinition> {
    legend(name, 1, &[Color::Red], 2, 2)
}

fn partner(name: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(r#"{"Keyword":"Partner"}"#).unwrap();
    Arc::new((*cmd(name)).clone().with_rules(rule))
}

/// A non-basic red card called `name`.
fn spell(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_power_toughness(1, 1),
    )
}

/// A game of `seats` seats under `house`: the seats before the last each have `others` Mountains and the commander
/// "Cmd", the last has `odd` as its deck and `odd_commanders`.
fn table_of(
    seats: usize,
    others: usize,
    odd: Vec<Arc<CardDefinition>>,
    odd_commanders: Vec<Arc<CardDefinition>>,
    house: HouseRules,
) -> GameSetup {
    let mut decks: Vec<_> = (1..seats).map(|_| vec![mountain(); others]).collect();
    let mut commanders: Vec<_> = (1..seats).map(|_| vec![cmd("Cmd")]).collect();
    decks.push(odd);
    commanders.push(odd_commanders);
    GameSetup::new(Arc::new(Commander), 7, decks)
        .with_commanders(commanders)
        .with_house_rules(house)
}

/// Why the setup's deck check refuses it: the seat and the rule it breaks.
fn refused(setup: &GameSetup) -> Option<(PlayerId, DeckError)> {
    match setup.validate() {
        Ok(()) => None,
        Err(SetupError::InvalidDeck { player, source }) => Some((player, source)),
        Err(other) => panic!("not a deck error: {other:?}"),
    }
}

/// The setup passes the deck check and a game starts from it.
fn starts(setup: GameSetup) {
    assert_eq!(refused(&setup), None);
    Engine::new(setup.clone()).unwrap();
}

#[rstest]
fn a_deck_of_the_size_the_table_sets_is_accepted_and_the_formats_size_is_refused(
    #[values(2, 3, 4, 6)] seats: usize,
    #[values(40, 60, 99, 250)] size: u32,
) {
    let house = deck_rules(|d| d.deck_size = Some(size));
    let others = size as usize - 1;
    // The commander is part of the size.
    starts(table_of(
        seats,
        others,
        vec![mountain(); others],
        vec![cmd("Cmd")],
        house.clone(),
    ));
    let under_the_formats_size = table_of(
        seats,
        others,
        vec![mountain(); others],
        vec![cmd("Cmd")],
        HouseRules::default(),
    );
    let (player, error) = refused(&under_the_formats_size).unwrap();
    assert_eq!(player, pid(0));
    assert_eq!(
        error,
        DeckError::WrongSize {
            expected: 100,
            got: size as usize
        }
    );
    // A deck of 100 is too many under another size, and the last seat is the one named.
    let odd = table_of(seats, others, vec![mountain(); 99], vec![cmd("Cmd")], house);
    if size != 100 {
        let (player, error) = refused(&odd).unwrap();
        assert_eq!(player, pid(seats - 1));
        assert_eq!(
            error,
            DeckError::WrongSize {
                expected: size as usize,
                got: 100
            }
        );
    }
}

#[rstest]
#[case::the_formats_singleton(HouseRules::default(), 2, Some(1))]
#[case::singleton_on(deck_rules(|d| d.singleton = Some(true)), 2, Some(1))]
#[case::singleton_off(relaxed_singleton(None), 9, None)]
#[case::singleton_off_for_two(relaxed_singleton(None), 2, None)]
#[case::copies_that_fit(relaxed_singleton(Some(9)), 9, None)]
#[case::one_copy_too_many(relaxed_singleton(Some(8)), 9, Some(8))]
#[case::a_cap_of_one(relaxed_singleton(Some(1)), 2, Some(1))]
#[case::a_cap_of_ninety_nine(relaxed_singleton(Some(99)), 90, None)]
fn a_card_may_be_played_in_the_copies_the_table_allows(
    #[values(2, 3, 4, 6)] seats: usize,
    #[case] house: HouseRules,
    #[case] copies: usize,
    #[case] refused_at: Option<usize>,
) {
    let mut deck = vec![mountain(); 99 - copies];
    deck.extend(vec![spell("Twin"); copies]);
    let setup = table_of(seats, 99, deck, vec![cmd("Cmd")], house);
    match refused_at {
        None => starts(setup),
        Some(max) => assert_eq!(
            refused(&setup),
            Some((
                pid(seats - 1),
                DeckError::TooManyCopies {
                    name: "Twin".into(),
                    max,
                    got: copies
                }
            ))
        ),
    }
}

#[rstest]
#[case::the_formats_identity(HouseRules::default(), false)]
#[case::identity_on(deck_rules(|d| d.colour_identity = Some(true)), false)]
#[case::identity_off(deck_rules(|d| d.colour_identity = Some(false)), true)]
fn a_card_outside_the_colour_identity_is_accepted_only_where_the_table_lifts_the_rule(
    #[values(2, 3, 4, 6)] seats: usize,
    #[case] house: HouseRules,
    #[case] accepted: bool,
) {
    // Forests are basic lands of a colour outside the red commander's identity.
    let mut deck = vec![mountain(); 96];
    deck.extend(vec![forest(); 3]);
    let setup = table_of(seats, 99, deck, vec![cmd("Cmd")], house);
    if accepted {
        starts(setup);
    } else {
        let (player, error) = refused(&setup).unwrap();
        assert_eq!(player, pid(seats - 1));
        assert!(
            matches!(&error, DeckError::OutsideColorIdentity { outside, identity, .. } if outside == "G" && identity == "R"),
            "{error:?}"
        );
    }
}

#[rstest]
#[case::the_formats_pair_needs_partners(HouseRules::default(), vec![cmd("A"), cmd("B")], Some(DeckError::NotPartners { first: "A".into(), second: "B".into() }))]
#[case::two_allow_any_two(rules(|r| r.commanders_per_seat = Some(2)), vec![cmd("A"), cmd("B")], None)]
#[case::two_allow_partners(rules(|r| r.commanders_per_seat = Some(2)), vec![partner("A"), partner("B")], None)]
#[case::two_allow_one(rules(|r| r.commanders_per_seat = Some(2)), vec![cmd("A")], None)]
#[case::two_refuse_three(rules(|r| r.commanders_per_seat = Some(2)), vec![cmd("A"), cmd("B"), cmd("C")], Some(DeckError::TooManyCommanders { max: 2, got: 3 }))]
#[case::partners_where_the_formats(HouseRules::default(), vec![partner("A"), partner("B")], None)]
#[case::one_refuses_partners(rules(|r| r.commanders_per_seat = Some(1)), vec![partner("A"), partner("B")], Some(DeckError::TooManyCommanders { max: 1, got: 2 }))]
fn a_seat_starts_with_the_commanders_the_table_allows(
    #[values(2, 3, 4, 6)] seats: usize,
    #[case] house: HouseRules,
    #[case] commanders: Vec<Arc<CardDefinition>>,
    #[case] error: Option<DeckError>,
) {
    let deck = vec![mountain(); 100 - commanders.len()];
    let named = commanders.len();
    let setup = table_of(seats, 99, deck, commanders, house);
    match error {
        None => {
            starts(setup.clone());
            let engine = Engine::new(setup).unwrap();
            assert_eq!(engine.state().commanders().count(), seats - 1 + named);
        }
        Some(error) => assert_eq!(refused(&setup), Some((pid(seats - 1), error))),
    }
}

#[test]
fn the_relaxations_do_not_lift_the_other_rules_of_the_deck() {
    let house = deck_rules(|d| {
        d.colour_identity = Some(false);
        d.singleton = Some(false);
        d.deck_size = Some(60);
    });
    let wrong_size = table_of(2, 59, vec![mountain(); 99], vec![cmd("Cmd")], house.clone());
    assert!(matches!(
        refused(&wrong_size),
        Some((_, DeckError::WrongSize { expected: 60, .. }))
    ));
    let no_commander = table_of(2, 59, vec![mountain(); 60], vec![], house.clone());
    assert_eq!(
        refused(&no_commander),
        Some((pid(1), DeckError::NoCommander))
    );
    let a_bear = Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    );
    let not_legendary = table_of(2, 59, vec![mountain(); 59], vec![a_bear], house);
    assert!(matches!(
        refused(&not_legendary),
        Some((_, DeckError::NotACommander { .. }))
    ));
}

#[test]
fn the_commanders_that_are_not_legendary_creatures_are_refused_whatever_the_rule() {
    let legendary_land = Arc::new(
        CardDefinition::new("Land")
            .with_type(CardType::Land)
            .with_supertype(Supertype::Legendary),
    );
    let setup = table_of(
        2,
        99,
        vec![mountain(); 98],
        vec![cmd("A"), legendary_land],
        rules(|r| r.commanders_per_seat = Some(2)),
    );
    assert!(matches!(
        refused(&setup),
        Some((_, DeckError::NotACommander { .. }))
    ));
}

// The ranges and the state.

#[rstest]
#[case::tax(rules(|r| r.commander_tax = Some(11)), "commander_tax")]
#[case::damage_none(rules(|r| r.commander_damage = lethal(0)), "commander_damage")]
#[case::damage_high(rules(|r| r.commander_damage = lethal(101)), "commander_damage")]
#[case::size_low(deck_rules(|d| d.deck_size = Some(39)), "deck.deck_size")]
#[case::size_high(deck_rules(|d| d.deck_size = Some(251)), "deck.deck_size")]
#[case::copies_none(relaxed_singleton(Some(0)), "deck.max_copies")]
#[case::copies_high(relaxed_singleton(Some(100)), "deck.max_copies")]
#[case::copies_of_a_singleton(deck_rules(|d| d.max_copies = Some(3)), "deck.max_copies")]
#[case::commanders_none(rules(|r| r.commanders_per_seat = Some(0)), "commanders_per_seat")]
#[case::commanders_three(rules(|r| r.commanders_per_seat = Some(3)), "commanders_per_seat")]
fn a_rule_out_of_range_refuses_the_setup_and_names_itself(
    #[values(2, 3, 4, 6)] seats: usize,
    #[case] house: HouseRules,
    #[case] field: &str,
) {
    let setup = table_of(seats, 99, vec![mountain(); 99], vec![cmd("Cmd")], house);
    match setup.validate() {
        Err(SetupError::HouseRules(error)) => assert_eq!(error.field(), field),
        other => panic!("expected {field} to be refused, got {other:?}"),
    }
    assert!(Engine::new(setup).is_err());
}

#[rstest]
#[case::tax_none(rules(|r| r.commander_tax = Some(0)))]
#[case::tax_one(rules(|r| r.commander_tax = Some(1)))]
#[case::a_value_equal_to_the_formats(rules(|r| r.commander_tax = Some(2)))]
#[case::damage(rules(|r| r.commander_damage = lethal(30)))]
#[case::damage_equal_to_the_formats(rules(|r| r.commander_damage = lethal(21)))]
#[case::damage_off(rules(|r| r.commander_damage = Some(CommanderDamageLimit::Off)))]
#[case::singleton(deck_rules(|d| d.singleton = Some(false)))]
#[case::copies(relaxed_singleton(Some(4)))]
#[case::identity(deck_rules(|d| d.colour_identity = Some(false)))]
#[case::size(deck_rules(|d| d.deck_size = Some(100)))]
#[case::commanders(rules(|r| r.commanders_per_seat = Some(1)))]
fn each_rule_makes_another_game(
    #[case] house: HouseRules,
    #[values(2, 4)] seats: usize,
) -> Result<(), mtg_engine::EngineError> {
    let hash = |house: HouseRules| -> Result<u64, mtg_engine::EngineError> {
        Ok(Engine::new(
            commander_fuzz_setup(seats, 5)
                .unwrap()
                .with_house_rules(house),
        )?
        .state()
        .state_hash())
    };
    assert_ne!(hash(HouseRules::default())?, hash(house)?);
    Ok(())
}

#[rstest]
fn a_game_under_the_rules_replays_to_the_live_state(
    #[values(2, 3, 4, 6)] seats: usize,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.commander_tax = Some(3);
        r.commander_damage = Some(CommanderDamageLimit::Off);
    });
    let mut driver = Driver::from_scenario(table(seats, 30, house.clone()))?;
    for _ in 0..3 {
        cast_and_resolve(&mut driver, seats)?;
        dies_and_returns(&mut driver)?;
    }
    let id = commander(&driver);
    assert_eq!(driver.state().commander_tax(id), 9);
    let setup = table(seats, 30, house).setup().unwrap();
    let rebuilt = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
    assert_eq!(rebuilt.commander_tax(id), 9);
    assert_eq!(rebuilt.house_rules().commander_tax, Some(3));
    Ok(())
}

// Whole games.

/// A fuzz game of `players` seats under `house`, its decks built to meet the deck rules: the size, the colour identity
/// and the copies the rules allow are used, not only met, and a table that allows two commanders gives every seat two.
fn game_under(players: usize, seed: u64, house: &HouseRules) -> GameSetup {
    let mut setup = commander_fuzz_setup(players, seed).unwrap();
    let pair = house.commanders_per_seat == Some(2);
    for (seat, player) in setup.players.iter_mut().enumerate() {
        if pair {
            let first = player.commanders[0].name.clone();
            let second = (0..64u64)
                .map(|n| {
                    commander_deck(&mut GameRng::substream(seed, 9_500 + 64 * seat as u64 + n))
                })
                .filter_map(Result::ok)
                .map(|(_, card)| card)
                .find(|card| {
                    card.name != first && player.library.iter().all(|c| c.name != card.name)
                })
                .expect("a second commander");
            player.commanders.push(second);
        }
        let size = house.deck.deck_size.map_or(100, |n| n as usize);
        let library = size - player.commanders.len();
        player.library.truncate(library);
        if house.deck.colour_identity == Some(false) {
            for slot in player.library.iter_mut().take(3) {
                *slot = forest();
            }
        }
        if house.deck.singleton == Some(false) {
            let copies = house.deck.max_copies.map_or(3, |n| n as usize).min(4);
            let twin = player
                .library
                .iter()
                .find(|card| card.singleton_limit() == Some(1))
                .cloned();
            if let Some(twin) = twin {
                let mut made = 1;
                for slot in &mut player.library {
                    if made < copies && slot.singleton_limit() == Some(1) && slot.name != twin.name
                    {
                        *slot = Arc::clone(&twin);
                        made += 1;
                    }
                }
            }
        }
        while player.library.len() < library {
            player.library.push(wastes());
        }
    }
    setup.with_house_rules(house.clone())
}

#[rstest]
fn the_fuzz_decks_use_the_relaxations_they_are_built_for(#[values(2, 3, 6)] players: usize) {
    let house = rules(|r| {
        r.deck = DeckRules {
            singleton: Some(false),
            colour_identity: Some(false),
            deck_size: Some(80),
            max_copies: Some(2),
        };
        r.commanders_per_seat = Some(2);
    });
    let setup = game_under(players, 4, &house);
    assert_eq!(setup.validate(), Ok(()));
    for player in &setup.players {
        assert_eq!(player.commanders.len(), 2);
        assert_eq!(player.library.len() + player.commanders.len(), 80);
        assert!(player.library.iter().any(|card| card.name == "Forest"));
        let twins = player
            .library
            .iter()
            .filter(|card| card.singleton_limit() == Some(1))
            .filter(|card| {
                player
                    .library
                    .iter()
                    .filter(|c| c.name == card.name)
                    .count()
                    == 2
            })
            .count();
        assert!(twins >= 2, "a card appears twice: {twins}");
    }
}

fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(4, |n| (n / 30).max(1))
}

#[rstest]
#[case::no_tax(rules(|r| r.commander_tax = Some(0)))]
#[case::a_heavy_tax(rules(|r| r.commander_tax = Some(10)))]
#[case::one_damage(rules(|r| r.commander_damage = lethal(1)))]
#[case::no_commander_damage(rules(|r| r.commander_damage = Some(CommanderDamageLimit::Off)))]
#[case::sixty_cards(deck_rules(|d| d.deck_size = Some(60)))]
#[case::forty_cards(deck_rules(|d| d.deck_size = Some(40)))]
#[case::copies(relaxed_singleton(Some(3)))]
#[case::any_copies(relaxed_singleton(None))]
#[case::any_colour(deck_rules(|d| d.colour_identity = Some(false)))]
#[case::two_commanders(rules(|r| r.commanders_per_seat = Some(2)))]
#[case::all_at_once(rules(|r| {
    r.commander_tax = Some(1);
    r.commander_damage = lethal(10);
    r.deck = DeckRules { singleton: Some(false), colour_identity: Some(false), deck_size: Some(80), max_copies: Some(2) };
    r.commanders_per_seat = Some(2);
}))]
fn bot_games_under_each_rule_end_with_the_invariants_kept(
    #[values(2, 3, 4, 6)] players: usize,
    #[case] house: HouseRules,
) {
    for_each_seed(0..games(), |seed| {
        let setup = game_under(players, seed, &house);
        play_game(setup, seed).unwrap_or_else(|failure: GameFailure| {
            panic!("{players} seats, seed {seed}, {house:?}: {failure}")
        });
    });
}

fn any_commander_rules() -> impl Strategy<Value = HouseRules> {
    (
        proptest::option::of(0..=10u32),
        proptest::option::of(prop_oneof![
            (1..=100u32).prop_map(CommanderDamageLimit::Lethal),
            Just(CommanderDamageLimit::Off)
        ]),
        proptest::option::of(any::<bool>()),
        proptest::option::of(any::<bool>()),
        proptest::option::of(40..=140u32),
        proptest::option::of(1..=99u32),
        proptest::option::of(1..=2u32),
    )
        .prop_map(
            |(tax, damage, singleton, identity, size, copies, per_seat)| HouseRules {
                commander_tax: tax,
                commander_damage: damage,
                deck: DeckRules {
                    singleton,
                    colour_identity: identity,
                    deck_size: size,
                    max_copies: copies.filter(|_| singleton == Some(false)),
                },
                commanders_per_seat: per_seat,
                ..HouseRules::default()
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    #[test]
    fn a_commander_game_under_any_commander_rules_plays_to_its_end(
        house in any_commander_rules(),
        players in 2usize..=6,
        seed in 0u64..10_000,
    ) {
        prop_assert_eq!(house.validate_for(players), Ok(()));
        let outcome = play_game(game_under(players, seed, &house), seed);
        prop_assert!(outcome.is_ok(), "{:?}", outcome.err());
    }
}
