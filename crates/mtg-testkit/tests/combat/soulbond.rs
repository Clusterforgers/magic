//! Goal: soulbond plays whole at tables of two, three and four seats: the pairing is offered as the soulbond creature enters and as another creature of its controller enters, may be declined, and is never offered for a creature that is paired already; it ends when either creature dies, changes controller, stops being a creature or is flickered (and the flickered creature can be paired again as it re-enters); two pairs stand side by side; Tandem Lookout gives its trigger to both creatures only while they are paired; every game replays to the same state, and bot games with the soulbond cards finish under the invariants and really make and end pairs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::seeds::fuzz_games;
use crate::common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, island, keyworded_creature, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, play_game, ready, soulbond_fuzz_setup};
use rstest::rstest;

fn bond(name: &str) -> Arc<CardDefinition> {
    keyworded_creature(name, 0, &[], 2, 2, &[Keyword::Soulbond])
}

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// Seat 0 is in its first main phase; every other seat has a Wall that deals no damage.
fn table(seats: usize) -> Scenario {
    (1..seats).fold(
        Scenario::new(seats).at(1, 0, Step::Main1),
        |scenario, seat| {
            scenario.battlefield(seat, [ready(free_creature(&format!("Wall {seat}"), 0, 4))])
        },
    )
}

/// The first two places of a battlefield: two permanents that start paired with each other.
fn starting_pair(first: Arc<CardDefinition>, second: Arc<CardDefinition>) -> Vec<PresetPermanent> {
    vec![
        PresetPermanent {
            paired_with: Some((0, 1)),
            ..ready(first)
        },
        PresetPermanent {
            paired_with: Some((0, 0)),
            ..ready(second)
        },
    ]
}

/// What the players were asked while the pairing was settled.
#[derive(Debug, Default)]
struct Asked {
    /// How many "may" questions.
    yes_no: usize,
    /// The creatures offered as the partner each time one was chosen, by name.
    offers: Vec<Vec<String>>,
}

/// Answers until the stack is empty at `until` (any step when `None`): a "may" with `pair`, a choice of the partner with
/// `partner` (the first offered when `None`), everything else passively.
fn settle(
    driver: &mut Driver,
    pair: bool,
    partner: Option<&str>,
    until: Option<Step>,
) -> Result<Asked, ScriptError> {
    let mut asked = Asked::default();
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && until.is_none_or(|step| driver.state().turn().step() == step) =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                asked.yes_no += 1;
                driver.answer(Answer::YesNo(YesNoChoice(pair)))?;
            }
            DecisionKind::ChooseCards(options) => {
                asked.offers.push(
                    options
                        .from
                        .iter()
                        .map(|id| name_of(driver.state(), *id))
                        .collect(),
                );
                let picked = match partner {
                    Some(name) => driver.id_of(name)?,
                    None => options.from[0],
                };
                driver.answer(Answer::ChooseCards(CardsChoice(vec![picked])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the game did not settle");
}

fn partner(driver: &Driver, name: &str) -> Option<String> {
    let id = driver.id_of(name).ok()?;
    driver
        .state()
        .partner_of(id)
        .map(|partner| name_of(driver.state(), partner))
}

fn count(driver: &Driver, wanted: fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| wanted(event))
        .count()
}

fn pairings(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Paired { .. }))
}

fn unpairings(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Unpaired { .. }))
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn the_pairing_is_made_as_either_creature_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] bond_enters: bool,
) -> Result<(), ScriptError> {
    let (waiting, entering) = if bond_enters {
        ("Ally", "Bond")
    } else {
        ("Bond", "Ally")
    };
    let cards = |name: &str| -> Arc<CardDefinition> {
        if name == "Bond" {
            bond("Bond")
        } else {
            free_creature("Ally", 1, 1)
        }
    };
    let scenario = table(seats)
        .battlefield(0, [ready(cards(waiting))])
        .hand(0, [cards(entering)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(entering)?;
    let asked = settle(&mut driver, true, None, None)?;
    assert_eq!(asked.yes_no, 1);
    assert_eq!(partner(&driver, "Bond").as_deref(), Some("Ally"));
    assert_eq!(partner(&driver, "Ally").as_deref(), Some("Bond"));
    assert_eq!((pairings(&driver), unpairings(&driver)), (1, 0));
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_declined_pairing_leaves_both_free_and_is_offered_again_when_another_creature_enters(
    #[values(2, 4)] seats: usize,
    #[values(true, false)] bond_enters: bool,
) -> Result<(), ScriptError> {
    let (waiting, entering) = if bond_enters {
        (free_creature("Ally 1", 1, 1), bond("Bond"))
    } else {
        (bond("Bond"), free_creature("Ally 1", 1, 1))
    };
    let scenario = table(seats)
        .battlefield(0, [ready(waiting)])
        .hand(0, [entering, free_creature("Ally 2", 2, 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(if bond_enters { "Bond" } else { "Ally 1" })?;
    let asked = settle(&mut driver, false, None, None)?;
    assert_eq!(asked.yes_no, 1);
    assert_eq!(pairings(&driver), 0);
    assert_eq!(partner(&driver, "Bond"), None);
    // Both are still free, so a third creature offers the pairing again; Bond is paired with the one it chooses.
    driver.cast("Ally 2")?;
    let asked = settle(&mut driver, true, Some("Ally 2"), None)?;
    assert!(asked.yes_no >= 1);
    assert_eq!(partner(&driver, "Bond").as_deref(), Some("Ally 2"));
    assert_eq!(pairings(&driver), 1);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::no_free_creature(0, 0)]
#[case::one_free_creature(1, 1)]
#[case::two_free_creatures(2, 1)]
fn a_second_soulbond_creature_cannot_take_a_creature_that_is_paired(
    #[case] free: usize,
    #[case] questions: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = starting_pair(bond("Bond A"), free_creature("Ally 1", 1, 1));
    mine.extend((0..free).map(|n| ready(free_creature(&format!("Free {n}"), 1, 1))));
    let scenario = table(seats).battlefield(0, mine).hand(0, [bond("Bond B")]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Bond B")?;
    let asked = settle(&mut driver, true, Some("Free 1"), None)?;
    assert_eq!(asked.yes_no, questions, "{asked:?}");
    // The pair that stood is untouched, and the new creature took one of the free creatures only.
    assert_eq!(partner(&driver, "Bond A").as_deref(), Some("Ally 1"));
    assert_eq!(partner(&driver, "Ally 1").as_deref(), Some("Bond A"));
    let expected = match free {
        0 => None,
        1 => Some("Free 0"),
        _ => Some("Free 1"),
    };
    assert_eq!(partner(&driver, "Bond B").as_deref(), expected);
    if free == 2 {
        assert_eq!(asked.offers, [vec!["Free 0", "Free 1"]]);
    } else {
        assert!(asked.offers.is_empty(), "nothing to choose between");
    }
    assert_eq!(pairings(&driver), usize::from(free > 0));
    replays(&driver);
    Ok(())
}

/// How a pair is ended in `a_pair_ends_when_either_creature_..`.
#[derive(Debug, Clone, Copy)]
enum Ending {
    /// Lightning Bolt kills the partner.
    Dies,
    /// An opponent takes the partner with Act of Treason.
    ChangesController,
    /// Imprisoned in the Moon makes the partner a land.
    StopsBeingACreature,
}

#[rstest]
#[case::the_partner_dies(Ending::Dies)]
#[case::the_partner_changes_controller(Ending::ChangesController)]
#[case::the_partner_stops_being_a_creature(Ending::StopsBeingACreature)]
fn a_pair_ends_when_either_creature_leaves_changes_controller_or_stops_being_a_creature(
    #[case] ending: Ending,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (spell, caster, land): (&str, usize, fn() -> Arc<CardDefinition>) = match ending {
        Ending::Dies => ("Lightning Bolt", 0, mountain),
        Ending::ChangesController => ("Act of Treason", 1, mountain),
        Ending::StopsBeingACreature => ("Imprisoned in the Moon", 0, island),
    };
    let mut mine = starting_pair(bond("Bond A"), free_creature("Ally 1", 1, 1));
    // The bystander is no part of the pair and keeps the table from being empty.
    mine.push(ready(free_creature("Bystander", 3, 3)));
    if caster == 0 {
        mine.extend(lands(3, land));
    }
    // Act of Treason is cast in the main phase of seat 1, on turn 2.
    let mut scenario = table(seats)
        .at(1 + u32::try_from(caster).unwrap(), caster, Step::Main1)
        .battlefield(0, mine)
        .hand(caster, [card(spell)]);
    if caster == 1 {
        let mut theirs = lands(3, land);
        theirs.push(ready(free_creature("Wall 1", 0, 4)));
        scenario = scenario.battlefield(1, theirs);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(partner(&driver, "Bond A").as_deref(), Some("Ally 1"));
    let target = driver.target_named("Ally 1")?;
    driver.cast_at(spell, &[target])?;
    let asked = settle(&mut driver, false, None, None)?;
    assert_eq!(asked.yes_no, 0, "nothing enters, so nothing is offered");
    assert_eq!(partner(&driver, "Bond A"), None);
    assert_eq!(partner(&driver, "Ally 1"), None);
    assert_eq!((pairings(&driver), unpairings(&driver)), (0, 1));
    // The creature that stayed is free to be paired again: it keeps no trace of the old pair.
    let bond_id = driver.id_of("Bond A")?;
    let held = driver
        .state()
        .object(bond_id)
        .and_then(|o| o.permanent())
        .unwrap();
    assert_eq!(held.paired_with(), None);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::the_partner_is_flickered("Ally 1", "Bond A")]
#[case::the_soulbond_creature_is_flickered("Bond A", "Ally 1")]
fn a_flickered_creature_is_a_new_object_and_is_paired_again_only_if_its_controller_chooses(
    #[case] flickered: &str,
    #[case] other: &str,
    #[values(true, false)] pair_again: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = starting_pair(bond("Bond A"), free_creature("Ally 1", 1, 1));
    mine.extend(lands(2, plains));
    let scenario = table(seats)
        .battlefield(0, mine)
        .hand(0, [card("Cloudshift")]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(flickered)?;
    driver.cast_at("Cloudshift", &[target])?;
    let asked = settle(&mut driver, pair_again, None, None)?;
    // Either way the old pair ended as the creature left (CR 702.95e), before it could be offered again.
    assert_eq!(unpairings(&driver), 1);
    assert_eq!(asked.yes_no, 1);
    let events = driver.engine().events();
    let ended = events
        .iter()
        .position(|event| matches!(event, Event::Unpaired { .. }))
        .unwrap();
    let made = events
        .iter()
        .position(|event| matches!(event, Event::Paired { .. }));
    assert_eq!(made.is_some(), pair_again);
    assert!(
        made.is_none_or(|made| ended < made),
        "the old pair ends first"
    );
    let expected = pair_again.then(|| other.to_owned());
    assert_eq!(partner(&driver, flickered), expected);
    assert_eq!(
        partner(&driver, other),
        pair_again.then(|| flickered.to_owned())
    );
    replays(&driver);
    Ok(())
}

#[test]
fn two_pairs_stand_side_by_side_and_one_ending_leaves_the_other() -> Result<(), ScriptError> {
    let mut mine = starting_pair(bond("Bond A"), free_creature("Ally 1", 1, 1));
    mine.push(PresetPermanent {
        paired_with: Some((0, 3)),
        ..ready(bond("Bond B"))
    });
    mine.push(PresetPermanent {
        paired_with: Some((0, 2)),
        ..ready(free_creature("Ally 2", 2, 2))
    });
    mine.extend(lands(1, mountain));
    let scenario = table(2)
        .battlefield(0, mine)
        .hand(0, [card("Lightning Bolt")]);
    let mut driver = Driver::from_scenario(scenario)?;
    for (a, b) in [("Bond A", "Ally 1"), ("Bond B", "Ally 2")] {
        assert_eq!(partner(&driver, a).as_deref(), Some(b));
        assert_eq!(partner(&driver, b).as_deref(), Some(a));
    }
    let target = driver.target_named("Ally 1")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    settle(&mut driver, false, None, None)?;
    assert_eq!(partner(&driver, "Bond A"), None);
    assert_eq!(partner(&driver, "Bond B").as_deref(), Some("Ally 2"));
    assert_eq!(partner(&driver, "Ally 2").as_deref(), Some("Bond B"));
    assert_eq!(unpairings(&driver), 1);
    replays(&driver);
    Ok(())
}

/// The cards in seat 0's hand.
fn hand_size(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(pid(0))).len()
}

#[rstest]
#[case::paired_with_one_attacker(true, &["Tandem Lookout", "Dasher"], 2)]
#[case::paired_and_a_third_attacks_too(true, &["Tandem Lookout", "Dasher", "Stranger"], 2)]
#[case::not_paired(false, &["Tandem Lookout", "Dasher", "Stranger"], 0)]
fn tandem_lookout_gives_its_trigger_to_both_creatures_only_while_they_are_paired(
    #[case] paired: bool,
    #[case] attackers: &[&str],
    #[case] drawn: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let lookout = card("Tandem Lookout");
    let dasher = free_creature("Dasher", 2, 2);
    let mut mine = if paired {
        starting_pair(lookout, dasher)
    } else {
        vec![ready(lookout), ready(dasher)]
    };
    mine.push(ready(free_creature("Stranger", 1, 1)));
    // Nobody blocks: the opponents have no creatures.
    let scenario = (1..seats)
        .fold(
            Scenario::new(seats).at(1, 0, Step::Main1),
            |scenario, seat| scenario.library(seat, (0..8).map(|_| plains())),
        )
        .battlefield(0, mine)
        .library(0, (0..8).map(|_| plains()));
    let mut driver = Driver::from_scenario(scenario)?;
    let before = hand_size(&driver);
    driver.pass_until(Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    settle(&mut driver, false, None, Some(Step::EndCombat))?;
    assert_eq!(hand_size(&driver) - before, drawn);
    assert_eq!(partner(&driver, "Tandem Lookout").is_some(), paired);
    replays(&driver);
    Ok(())
}

#[test]
fn tandem_lookout_pairs_with_a_creature_that_enters_after_it() -> Result<(), ScriptError> {
    let scenario = table(2)
        .battlefield(0, [ready(card("Tandem Lookout"))])
        .hand(0, [free_creature("Ally", 1, 1)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Ally")?;
    let asked = settle(&mut driver, true, None, None)?;
    assert_eq!(asked.yes_no, 1);
    assert_eq!(partner(&driver, "Tandem Lookout").as_deref(), Some("Ally"));
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_soulbond_cards_finish_and_make_and_end_pairs(#[case] seats: usize) {
    let (mut made, mut ended) = (0, 0);
    for seed in 0..fuzz_games(40) {
        let setup = soulbond_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        made += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::Paired { .. }))
            .count();
        ended += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::Unpaired { .. }))
            .count();
    }
    assert!(made > 0, "some creature was paired");
    assert!(ended > 0, "some pair ended");
}

#[rstest]
#[case::two(2, 41)]
#[case::four(4, 42)]
fn a_game_with_soulbond_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = soulbond_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = soulbond_fuzz_setup(seats, seed).unwrap();
    assert_eq!(
        replay(&setup, &first.events).unwrap().state_hash(),
        first.final_hash
    );
}
