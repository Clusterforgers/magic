//! Goal: Crew N plays as the rules say (CR 702.122) at tables of two, three and four seats: the ability is offered while untapped creatures with total power N or more can pay it, whether they are summoning sick or not and never when they are tapped or too weak; the creatures are asked for only when there is a choice and an answer with too little power is refused without a trace; exactly N power and more than N both pay, the chosen creatures are tapped and the others are not, backing out taps nothing, the Vehicle is an artifact creature with its printed power and toughness until end of turn and not after it, crewing a Vehicle that is already a creature changes nothing, an ability whose Vehicle left does nothing, a crewed Vehicle attacks and deals damage unless it came under its controller's control this turn, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{cost, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

/// A free 4/4 Vehicle with Crew `crew`.
fn skiff(crew: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Skiff")
            .with_type(CardType::Artifact)
            .with_power_toughness(4, 4)
            .with_rules(Ability::Keyword(Keyword::Crew(crew))),
    )
}

/// `{R}` instant: destroy target artifact.
fn shatter() -> Arc<CardDefinition> {
    let destroy = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
    Arc::new(
        CardDefinition::new("Shatter")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(serde_json::from_str::<Ability>(destroy).unwrap()),
    )
}

/// A creature of the kind `how` says ("ready", "sick" or "tapped") with this name and power.
fn member(how: &str, name: &str, power: i32) -> PresetPermanent {
    let card = free_creature(name, power, 2);
    match how {
        "sick" => sick(card),
        "tapped" => tapped(card),
        _ => ready(card),
    }
}

/// Seat 0 controls a Skiff with Crew `crew` and these creatures; seat 1 controls an Ogre.
fn table(seats: usize, crew: u32, members: Vec<PresetPermanent>) -> Scenario {
    let mut permanents = vec![ready(skiff(crew))];
    permanents.extend(members);
    Scenario::new(seats)
        .battlefield(0, permanents)
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .is_creature()
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Picks these creatures for the pending cost.
fn pick(driver: &mut Driver, names: &[&str]) -> Result<(), ScriptError> {
    let ids = names.iter().map(|name| driver.id_of(name).unwrap());
    driver.answer(Answer::ChooseCards(CardsChoice(ids.collect())))?;
    Ok(())
}

/// Crews the Skiff with these creatures (asked for unless there is only one to take) and lets the ability resolve.
fn crew(driver: &mut Driver, names: &[&str]) -> Result<(), ScriptError> {
    driver.begin_activation("Skiff")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        pick(driver, names)?;
    }
    settle(driver)
}

/// The creatures the pending question offers, sorted, and how many it takes at least and at most.
fn asked(driver: &Driver) -> (Vec<String>, u32, u32) {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a choice of creatures was expected");
    };
    let mut names: Vec<String> = options
        .from
        .iter()
        .map(|id| common::name_of(driver.state(), *id))
        .collect();
    names.sort();
    (names, options.min, options.max)
}

// Offering the ability

#[rstest]
#[case::one_creature_of_exactly_the_power(3, &[("ready", 3)], true)]
#[case::one_creature_with_more(3, &[("ready", 5)], true)]
#[case::two_creatures_together(3, &[("ready", 2), ("ready", 1)], true)]
#[case::three_small_creatures(3, &[("ready", 1), ("ready", 1), ("ready", 1)], true)]
#[case::summoning_sick_creatures_may_crew(2, &[("sick", 2)], true)]
#[case::too_little_power(3, &[("ready", 1), ("ready", 1)], false)]
#[case::tapped_creatures_do_not_count(3, &[("ready", 1), ("tapped", 5)], false)]
#[case::no_creature(1, &[], false)]
#[case::zero_power_does_not_help(1, &[("ready", 0), ("ready", 0)], false)]
fn crew_is_offered_while_untapped_creatures_have_the_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] crew: u32,
    #[case] members: &[(&str, i32)],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let members: Vec<PresetPermanent> = members
        .iter()
        .enumerate()
        .map(|(at, (how, power))| member(how, &format!("Member {at}"), *power))
        .collect();
    let driver = Driver::from_scenario(table(seats, crew, members))?;
    assert_eq!(!driver.offered_abilities("Skiff")?.is_empty(), offered);
    Ok(())
}

// Paying

#[rstest]
fn crew_asks_which_creatures_and_refuses_too_little_power_without_a_trace(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let members = vec![
        member("ready", "Bear", 2),
        member("ready", "Mouse", 1),
        member("ready", "Wolf", 3),
    ];
    let mut driver = Driver::from_scenario(table(seats, 3, members))?;
    driver.begin_activation("Skiff")?;
    assert_eq!(
        asked(&driver),
        (vec!["Bear".into(), "Mouse".into(), "Wolf".into()], 1, 3)
    );
    let before = driver.state().state_hash();
    let error = pick(&mut driver, &["Bear"]).unwrap_err();
    assert!(
        matches!(
            error,
            ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooLittlePower {
                needed: 3,
                got: 2
            }))
        ),
        "{error:?}"
    );
    assert_eq!(driver.state().state_hash(), before);
    assert_eq!(asked(&driver).1, 1, "the same question is still pending");
    Ok(())
}

#[rstest]
#[case::exactly_the_power(&["Bear", "Mouse"], &["Bear", "Mouse"], &["Wolf"])]
#[case::more_than_needed(&["Bear", "Wolf"], &["Bear", "Wolf"], &["Mouse"])]
#[case::one_strong_creature(&["Wolf"], &["Wolf"], &["Bear", "Mouse"])]
#[case::everything(&["Bear", "Mouse", "Wolf"], &["Bear", "Mouse", "Wolf"], &[])]
fn crew_taps_the_chosen_creatures_and_only_them(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: &[&str],
    #[case] now_tapped: &[&str],
    #[case] untouched: &[&str],
) -> Result<(), ScriptError> {
    let members = vec![
        member("ready", "Bear", 2),
        member("ready", "Mouse", 1),
        member("ready", "Wolf", 3),
    ];
    let mut driver = Driver::from_scenario(table(seats, 3, members))?;
    assert!(!is_creature(&driver, "Skiff"));
    driver.begin_activation("Skiff")?;
    pick(&mut driver, chosen)?;
    // The cost is paid and the ability waits on the stack: the Vehicle is not a creature yet.
    assert_eq!(driver.state().stack().len(), 1);
    assert!(!is_creature(&driver, "Skiff"));
    for name in now_tapped {
        assert!(is_tapped(&driver, name), "{name} is tapped");
    }
    for name in untouched {
        assert!(!is_tapped(&driver, name), "{name} is not tapped");
    }
    assert!(
        !is_tapped(&driver, "Skiff"),
        "crewing does not tap the Vehicle"
    );
    settle(&mut driver)?;
    assert!(is_creature(&driver, "Skiff"));
    assert_eq!(stats(&driver, "Skiff"), (4, 4));
    Ok(())
}

#[rstest]
fn crew_with_one_creature_to_choose_asks_nothing_and_a_sick_creature_pays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 2, vec![member("sick", "Cub", 2)]))?;
    driver.begin_activation("Skiff")?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack().len(), 1);
    assert!(is_tapped(&driver, "Cub"));
    settle(&mut driver)?;
    assert!(is_creature(&driver, "Skiff"));
    Ok(())
}

#[rstest]
fn backing_out_of_crew_taps_nothing_and_leaves_no_trace(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let members = vec![member("ready", "Bear", 2), member("ready", "Wolf", 3)];
    let mut driver = Driver::from_scenario(table(seats, 2, members))?;
    let before = driver.state().state_hash();
    driver.begin_activation("Skiff")?;
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    assert_eq!(driver.state().state_hash(), before);
    assert!(!is_tapped(&driver, "Bear") && !is_tapped(&driver, "Wolf"));
    Ok(())
}

// The Vehicle as a creature

#[rstest]
fn the_vehicle_is_a_creature_until_the_end_of_the_turn_and_not_after_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 1, vec![member("ready", "Bear", 2)]))?;
    crew(&mut driver, &["Bear"])?;
    assert!(is_creature(&driver, "Skiff"));
    driver.idle_until(2, Step::Main1)?;
    assert!(!is_creature(&driver, "Skiff"));
    assert_eq!(driver.state().turn().number(), 2);
    Ok(())
}

#[rstest]
fn crewing_a_vehicle_that_is_a_creature_changes_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let members = vec![member("ready", "Bear", 2), member("ready", "Wolf", 3)];
    let mut driver = Driver::from_scenario(table(seats, 2, members))?;
    crew(&mut driver, &["Bear"])?;
    assert!(is_creature(&driver, "Skiff"));
    assert_eq!(driver.offered_abilities("Skiff")?, [0]);
    crew(&mut driver, &["Wolf"])?;
    assert!(is_creature(&driver, "Skiff"));
    assert_eq!(stats(&driver, "Skiff"), (4, 4));
    Ok(())
}

#[rstest]
fn a_vehicle_that_leaves_before_the_ability_resolves_is_not_animated(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(skiff(1)),
                member("ready", "Bear", 2),
                ready(mountain()),
            ],
        )
        .hand(0, [shatter()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Skiff")?;
    let target = driver.target_named("Skiff")?;
    driver.cast_at("Shatter", &[target])?;
    settle(&mut driver)?;
    assert!(
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .all(|id| common::name_of(driver.state(), *id) != "Skiff")
    );
    assert_eq!(driver.state().stack().len(), 0);
    Ok(())
}

// Attacking

#[rstest]
fn a_crewed_vehicle_attacks_and_deals_its_damage(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 2, vec![member("sick", "Cub", 2)]))?;
    let life = driver.state().player(pid(1)).unwrap().life();
    crew(&mut driver, &["Cub"])?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Skiff", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), life - 4);
    assert!(is_tapped(&driver, "Skiff"));
    Ok(())
}

#[rstest]
fn a_vehicle_that_came_under_control_this_turn_can_be_crewed_but_cannot_attack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [sick(skiff(1)), member("ready", "Bear", 2)])
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))]);
    let mut driver = Driver::from_scenario(scenario)?;
    crew(&mut driver, &["Bear"])?;
    assert!(is_creature(&driver, "Skiff"));
    driver.pass_until(Step::DeclareAttackers).ok();
    if let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind {
        assert!(
            options
                .attackers
                .iter()
                .all(|a| a.object != driver.id_of("Skiff").unwrap())
        );
    }
    Ok(())
}

// Replay

#[rstest]
fn the_log_of_a_crewed_game_replays_to_the_live_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        table(
            seats,
            3,
            vec![member("ready", "Bear", 2), member("ready", "Wolf", 3)],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    crew(&mut driver, &["Bear", "Wolf"])?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Skiff", 1)])?;
    driver.idle_until(2, Step::Main1)?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

// "If you do" after a "may" (Smuggler's Copter)

/// Seat 0 controls a crewed-ready Copter and a Cub, holds a Spare and has a Top and a Next in its library.
fn copter_table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(mtg_testkit::corpus::card("Smuggler's Copter").unwrap()),
                member("sick", "Cub", 1),
            ],
        )
        .hand(0, [free_creature("Spare", 1, 1)])
        .library(0, [free_creature("Top", 1, 1), free_creature("Next", 1, 1)])
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
}

/// Crews the Copter, attacks with it and answers the loot's question `yes`; returns whether it asked for a discard.
fn loot(driver: &mut Driver, yes: bool) -> Result<bool, ScriptError> {
    driver.begin_activation("Smuggler's Copter")?;
    settle(driver)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Smuggler's Copter", 1)])?;
    let mut discards = false;
    for _ in 0..50 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(_) | DecisionKind::Discard(_) => {
                discards = true;
                driver.answer_passively()?;
            }
            _ if driver.state().turn().step() != Step::DeclareAttackers
                && driver.state().stack().is_empty() =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(discards)
}

#[rstest]
fn if_you_do_discards_only_after_the_draw_was_accepted(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] yes: bool,
) -> Result<(), ScriptError> {
    let scenario = || copter_table(seats);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    let discards = loot(&mut driver, yes)?;
    assert_eq!(discards, yes, "a discard is asked only after a draw");
    // Accepted: a card drawn and a card discarded, so the hand keeps its size, the library is one card shorter and
    // the graveyard has the discard. Declined: nothing drawn and nothing discarded.
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 1);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        library - usize::from(yes)
    );
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len(),
        usize::from(yes)
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

// Whole games

#[test]
fn bot_games_with_vehicles_keep_every_invariant_crew_and_replay() {
    use std::sync::Mutex;

    use mtg_engine::Event;
    use mtg_testkit::vehicle_cards::VEHICLE_CARDS;
    use mtg_testkit::{PlayOptions, play_game_with, vehicle_fuzz_setup};

    use crate::common::seeds::for_each_seed;

    // Crew abilities activated and Vehicles cast in all the games.
    let seen = Mutex::new((0u64, 0u64));
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                vehicle_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&vehicle_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::AbilityActivated { source, .. } => {
                        let name = replayed.object(source.id).map(|o| o.card().name.clone());
                        if name.is_some_and(|name| {
                            name.starts_with("Plain Vehicle")
                                || VEHICLE_CARDS.contains(&name.as_str())
                        }) {
                            seen.0 += 1;
                        }
                    }
                    Event::SpellCast { object, .. } => {
                        let name = replayed.object(*object).map(|o| o.card().name.clone());
                        if name.is_some_and(|name| name.starts_with("Plain Vehicle")) {
                            seen.1 += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    let (crewed, cast) = seen.into_inner().unwrap();
    assert!(cast > 0, "some Vehicles were cast");
    assert!(crewed > 0, "some Vehicle was crewed");
}
