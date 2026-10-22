//! Goal: the corpus cards that follow the creature an Equipment is attached to (Skullclamp, Sword of Body and Mind), wait for a batch of events (The Thing), count the cards of a turn (Lat-Nam Adept) or read the amount of an event (Sanguine Bond, Fear of Failed Tests) trigger for the subjects they name and no others; a static ability that reads the opponents' life (Bloodghast) follows any, every and no opponent at two, three and four seats and when a seat has left; and whole bot games at two, three and four seats with decks of these cards finish, keep every invariant, trigger every card and replay to the same log and state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::fuzz_games;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{burn, creature, free_creature, island, mountain, study};
use mtg_testkit::corpus::card;
use mtg_testkit::family_cards::FAMILY_CARDS;
use mtg_testkit::{
    Driver, Scenario, ScriptError, attached_to, families_fuzz_setup, play_game, ready,
};
use rstest::rstest;

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority while the stack holds something, answering every "may" with `yes`.
fn settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => return Ok(()),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Plays on to the second main phase of the turn, blocking as `blocks` says.
fn through_combat(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".into()))
}

/// Seat 0 holds a Skullclamp on the creature at `host` (seat, place), has a Burn of `damage` and a Mountain, and seat 1 has a
/// Bear; the Burn is cast at `victim`.
#[rstest]
#[case::the_equipped_creature_dies(0, 1, "Host", 2)]
#[case::an_opponents_equipped_creature_dies(1, 0, "Host", 2)]
#[case::another_creature_dies(0, 1, "Other", 0)]
fn skullclamp_draws_two_when_the_creature_it_is_attached_to_dies(
    #[case] host_seat: usize,
    #[case] other_seat: usize,
    #[case] victim: &str,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    // Seat 0: Mountain, its creature if any, the Skullclamp; seat 1: its creature if any.
    let mut mine = vec![ready(mountain())];
    let mut theirs = Vec::new();
    let host_place = if host_seat == 0 { mine.len() } else { 0 };
    for (seat, name) in [(host_seat, "Host"), (other_seat, "Other")] {
        let list = if seat == 0 { &mut mine } else { &mut theirs };
        list.push(ready(free_creature(name, 2, 2)));
    }
    let host_place = if host_seat == 0 { 1 } else { host_place };
    mine.push(attached_to(
        ready(card("Skullclamp").unwrap()),
        host_seat,
        host_place,
    ));
    let scenario = Scenario::new(2)
        .hand(0, [burn("Ping", 1)])
        .battlefield(0, mine)
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    let cards = hand(&driver, 0);
    let target = driver.target_named(victim)?;
    driver.cast_at("Ping", &[target])?;
    settle(&mut driver, true)?;
    // The Ping leaves the hand; Skullclamp draws two for whoever's creature it was on.
    assert_eq!(hand(&driver, 0), cards - 1 + drawn);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::one_spell(1, 1)]
#[case::two_spells(2, 1)]
fn lat_nam_adept_grows_on_the_second_card_of_a_turn_only(
    #[case] spells: usize,
    #[case] grown: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [study("Study A"), study("Study B")])
            .battlefield(
                0,
                [
                    ready(card("Lat-Nam Adept").unwrap()),
                    ready(island()),
                    ready(island()),
                ],
            ),
    )?;
    assert_eq!(counters(&driver, "Lat-Nam Adept"), 0);
    for name in ["Study A", "Study B"].into_iter().take(spells) {
        driver.cast(name)?;
        settle(&mut driver, true)?;
    }
    assert_eq!(counters(&driver, "Lat-Nam Adept"), grown);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn sanguine_bond_drains_the_life_gained_from_the_opponent_you_choose(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Renewed Faith").unwrap()])
            .battlefield(
                0,
                [
                    ready(card("Sanguine Bond").unwrap()),
                    ready(island()),
                    ready(mtg_testkit::cards::plains()),
                    ready(mtg_testkit::cards::plains()),
                ],
            ),
    )?;
    driver.cast("Renewed Faith")?;
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[driver.target_player(seats - 1)])?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => break,
        }
    }
    assert_eq!(life(&driver, 0), 26, "Renewed Faith gains 6");
    assert_eq!(life(&driver, seats - 1), 14, "that much, once");
    for seat in 1..seats - 1 {
        assert_eq!(life(&driver, seat), 20);
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::unblocked(&[], 2)]
#[case::blocked(&[("Wall", "Fear of Failed Tests")], 0)]
fn fear_of_failed_tests_draws_as_many_cards_as_the_combat_damage_to_a_player(
    #[case] blocks: &[(&str, &str)],
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card("Fear of Failed Tests").unwrap())])
            .battlefield(1, [ready(free_creature("Wall", 0, 4))]),
    )?;
    let cards = hand(&driver, 0);
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Fear of Failed Tests", 1)])?;
    through_combat(&mut driver, blocks)?;
    settle(&mut driver, true)?;
    assert_eq!(hand(&driver, 0), cards + drawn);
    assert_eq!(life(&driver, 1), 20 - i32::try_from(drawn).unwrap());
    assert_replays(&driver);
    Ok(())
}

fn hero(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Hero")
            .with_power_toughness(1, 1),
    )
}

#[rstest]
#[case::the_thing_alone(&["The Thing, Ben Grimm"], 2)]
#[case::another_hero_alone(&["Hero A"], 2)]
#[case::three_heroes_deal_damage_at_once(&["The Thing, Ben Grimm", "Hero A", "Hero B"], 2)]
#[case::a_creature_that_is_no_hero(&["Bear"], 0)]
fn the_thing_grows_once_when_heroes_deal_damage_together(
    #[case] attackers: &[&str],
    #[case] grown: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(card("The Thing, Ben Grimm").unwrap()),
            ready(hero("Hero A")),
            ready(hero("Hero B")),
            ready(free_creature("Bear", 2, 2)),
        ],
    ))?;
    driver.pass_until(Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    through_combat(&mut driver, &[])?;
    settle(&mut driver, true)?;
    assert_eq!(counters(&driver, "The Thing, Ben Grimm"), grown);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::unblocked(&[], 4, 10, 1)]
#[case::blocked_by_a_red_creature(&[("Red", "Knight")], 0, 0, 0)]
fn the_sword_of_body_and_mind_makes_a_wolf_and_mills_the_player_it_hurt(
    #[case] blocks: &[(&str, &str)],
    #[case] damage: i32,
    #[case] milled: usize,
    #[case] wolves: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(free_creature("Knight", 2, 2)),
                    attached_to(ready(card("Sword of Body and Mind").unwrap()), 0, 0),
                ],
            )
            .battlefield(1, [ready(creature("Red", 0, &[Color::Red], 1, 8))]),
    )?;
    let library = driver.state().zone(Zone::Library(pid(1))).len();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Knight", 1)])?;
    through_combat(&mut driver, blocks)?;
    settle(&mut driver, true)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(1))).len(),
        library - milled
    );
    let wolf = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|o| o.card().name == "Wolf" && o.controller() == pid(0))
            .count()
    };
    assert_eq!(wolf(&driver), wolves);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::green("Green")]
#[case::blue("Blue")]
fn a_creature_with_the_sword_of_body_and_mind_has_protection_from_green_and_blue(
    #[case] color: &str,
) -> Result<(), ScriptError> {
    let color = if color == "Green" {
        Color::Green
    } else {
        Color::Blue
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(free_creature("Knight", 2, 2)),
                    attached_to(ready(card("Sword of Body and Mind").unwrap()), 0, 0),
                ],
            )
            .battlefield(1, [ready(creature("Blocker", 0, &[color], 1, 1))]),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Knight", 1)])?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(driver.block(&[("Blocker", "Knight")]).is_err());
    Ok(())
}

/// A creature that has haste as long as each opponent has 10 or less life.
fn each_ghost() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":{"Compare":{"left":{"HighestLife":"Opponents"},"op":"Le","right":10}}}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Each Ghost")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

/// What `Bloodghast` ("an opponent has 10 or less life") and `Each Ghost` ("each opponent has 10 or less life") have when the
/// seats after the first have these life totals and these seats have left the game.
#[rstest]
#[case::two_seats_low(&[10], &[], true, true)]
#[case::two_seats_high(&[11], &[], false, false)]
#[case::two_seats_dying(&[1], &[], true, true)]
#[case::three_seats_one_low(&[11, 10], &[], true, false)]
#[case::three_seats_both_low(&[10, 3], &[], true, true)]
#[case::three_seats_none_low(&[11, 20], &[], false, false)]
#[case::four_seats_the_last_low(&[20, 20, 5], &[], true, false)]
#[case::four_seats_none_low(&[20, 20, 20], &[], false, false)]
#[case::four_seats_all_low(&[10, 9, 8], &[], true, true)]
#[case::the_low_seat_has_left(&[10, 20], &[1], false, false)]
#[case::the_high_seat_has_left(&[20, 10], &[1], true, true)]
#[case::the_high_seat_of_four_has_left(&[20, 10, 5], &[1], true, true)]
fn a_static_ability_reads_the_life_of_any_or_every_opponent_still_in_the_game(
    #[case] lives: &[i32],
    #[case] left: &[usize],
    #[case] any: bool,
    #[case] every: bool,
) -> Result<(), ScriptError> {
    let seats = lives.len() + 1;
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Bloodghast").unwrap()), ready(each_ghost())])
        .life(0, 3);
    for (index, total) in lives.iter().enumerate() {
        scenario = scenario.life(index + 1, *total);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for seat in left {
        driver.concede(pid(*seat))?;
    }
    let haste =
        |name: &str| has_keyword(driver.state(), driver.id_of(name).unwrap(), &Keyword::Haste);
    assert_eq!(haste("Bloodghast"), any, "an opponent has 10 or less");
    assert_eq!(haste("Each Ghost"), every, "each opponent has 10 or less");
    Ok(())
}

#[test]
fn bloodghast_gains_and_loses_haste_as_the_life_of_an_opponent_moves() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card("Bloodghast").unwrap()), ready(mountain())])
            .hand(0, [burn("Bolt", 3)])
            .life(1, 12),
    )?;
    let haste = |driver: &Driver| {
        has_keyword(
            driver.state(),
            driver.id_of("Bloodghast").unwrap(),
            &Keyword::Haste,
        )
    };
    assert!(!haste(&driver));
    let target = driver.target_player(1);
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver, true)?;
    assert_eq!(life(&driver, 1), 9);
    assert!(haste(&driver));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::yes(true, true)]
#[case::no(false, false)]
fn bloodghast_returns_from_the_graveyard_when_a_land_enters_and_its_controller_says_yes(
    #[case] yes: bool,
    #[case] returned: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard(0, [card("Bloodghast").unwrap()])
            .hand(0, [mountain()]),
    )?;
    driver.play_land("Mountain")?;
    settle(&mut driver, yes)?;
    let on_battlefield = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .any(|o| o.card().name == "Bloodghast");
    assert_eq!(on_battlefield, returned);
    assert_replays(&driver);
    Ok(())
}

/// The cards a game's triggers came from, by name.
fn triggered_cards(setup: &GameSetup, events: &[Event]) -> BTreeSet<String> {
    let state = replay(setup, events).unwrap();
    events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityTriggered(triggered) => state.object(triggered.source.id),
            _ => None,
        })
        .map(|object| object.card().name.clone())
        .collect()
}

fn games() -> u64 {
    fuzz_games(20)
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_family_games_finish_keep_every_invariant_and_trigger_every_card(#[case] players: usize) {
    let games = games();
    let mut seen = BTreeSet::new();
    for seed in 0..games {
        let setup = families_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.extend(triggered_cards(&setup, &outcome.events));
    }
    for &(name, _) in FAMILY_CARDS {
        assert!(
            seen.contains(name),
            "{name} never triggered in {games} games of {players} seats: {seen:?}"
        );
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn a_seeded_family_game_is_the_same_game_every_time_and_replays_to_its_state(
    #[case] players: usize,
) {
    for seed in 0..6u64 {
        let setup = families_fuzz_setup(players, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.events, second.events, "seed {seed}");
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        let state = replay(&setup, &first.events).unwrap();
        assert_eq!(state.state_hash(), first.final_hash, "seed {seed}");
    }
}
