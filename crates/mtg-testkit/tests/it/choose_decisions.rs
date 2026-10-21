//! Goal: the three decisions a spell asks that are not cards play whole at tables of two, three and four seats: a "may" asks the player it names (the caster, or a target at any seat) and does its effects only on a yes; a choice among opponents asks the caster which, and not at all when there is one; a modal spell asks its modes as it is cast and then only the targets of the modes chosen, resolves them in printed order, a mode twice twice, and offers no mode whose targets cannot be met; backing out of the cast leaves no trace; an answer that was not offered, repeats a mode, has the wrong number or is of the wrong kind is refused by name and changes nothing; an engine rebuilt from the log asks the same question at each decision and the log replays to the state it was played to at each of them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{
    ChoiceOption, ModeChoice, ModeOptions, OptionChoice, Target, YesNoChoice, YesNoOptions,
};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain};
use mtg_testkit::choice_cards::{charm, gift, may_draw, scorn};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `spells`, an Island and two Mountains; every seat has forests to draw.
fn table(seats: usize, spells: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, spells)
        .battlefield(0, [ready(island()), ready(mountain()), ready(mountain())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| forest()));
    }
    scenario
}

/// Everyone passes until the stack is empty or something asks a question.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

fn pay(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver)
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn yes_no(driver: &Driver) -> (Decision, YesNoOptions) {
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::YesNo(options) = decision.kind.clone() else {
        panic!(
            "a yes-or-no question was expected, got {}",
            decision.kind.name()
        );
    };
    (decision, options)
}

fn answers_logged(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::ChoiceMade { .. }))
        .count()
}

#[rstest]
fn a_may_asks_the_player_it_names_and_does_its_effects_only_on_a_yes(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] yes: bool,
    #[values(false, true)] aimed: bool,
) -> Result<(), ScriptError> {
    let spell = if aimed {
        gift("Gift")
    } else {
        may_draw("Study")
    };
    let name = if aimed { "Gift" } else { "Study" };
    let mut driver = Driver::from_scenario(table(seats, vec![spell]))?;
    let decider = if aimed { seats - 1 } else { 0 };
    driver.begin_cast(name)?;
    if aimed {
        driver.choose_targets(&[driver.target_player(decider)])?;
    }
    let begun = driver.engine().events().len();
    pay(&mut driver)?;

    let (decision, options) = yes_no(&driver);
    assert_eq!(
        decision.player,
        pid(decider),
        "the player the effect names answers"
    );
    assert_eq!(options.text, if aimed { "" } else { "Draw a card?" });
    let (before, mine) = (hand(&driver, decider), hand(&driver, 0));
    driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
    settle(&mut driver)?;

    assert_eq!(driver.state().stack(), []);
    assert_eq!(hand(&driver, decider), before + usize::from(yes));
    if aimed {
        assert_eq!(hand(&driver, 0), mine, "the caster drew nothing");
        driver.expect_life(0, 22)?;
    }
    assert_eq!(answers_logged(&driver.engine().events()[begun..]), 1);
    Ok(())
}

/// The seat a loss of 3 life fell on, if any.
fn victim(driver: &Driver, seats: usize) -> Vec<usize> {
    (0..seats)
        .filter(|seat| driver.state().player(pid(*seat)).unwrap().life() == 17)
        .collect()
}

#[rstest]
fn a_choice_among_opponents_asks_the_caster_which_unless_there_is_only_one(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    for picked in 0..seats - 1 {
        let mut driver = Driver::from_scenario(table(seats, vec![scorn("Scorn")]))?;
        driver.begin_cast("Scorn")?;
        let begun = driver.engine().events().len();
        pay(&mut driver)?;
        if seats == 2 {
            assert_eq!(answers_logged(&driver.engine().events()[begun..]), 0);
        } else {
            let decision = driver.decision()?.clone();
            assert_eq!(decision.player, pid(0));
            let DecisionKind::ChooseOption(options) = decision.kind else {
                panic!("a choice of option was expected");
            };
            let opponents: Vec<ChoiceOption> = (1..seats)
                .map(|seat| ChoiceOption::Player(pid(seat)))
                .collect();
            assert_eq!(options.options, opponents);
            assert_eq!(options.text, "Choose an opponent.");
            driver.answer(Answer::ChooseOption(OptionChoice(picked)))?;
            settle(&mut driver)?;
            assert_eq!(answers_logged(&driver.engine().events()[begun..]), 1);
        }
        assert_eq!(
            victim(&driver, seats),
            [picked + 1],
            "{seats} seats, choice {picked}"
        );
    }
    Ok(())
}

fn modes(driver: &Driver) -> ModeOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseMode(options) => options.clone(),
        other => panic!("a choice of modes was expected, got {}", other.name()),
    }
}

fn offered(options: &ModeOptions) -> Vec<usize> {
    options.modes.iter().map(|mode| mode.index).collect()
}

fn choose_modes(driver: &mut Driver, chosen: &[usize]) -> Result<(), ScriptError> {
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(chosen.to_vec())))?;
    Ok(())
}

/// What happened as the spell resolved, in the words of the card.
fn story(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt { .. } => Some("damage".to_owned()),
            Event::LifeGained { player, amount } => {
                Some(format!("seat {} gains {amount}", player.index()))
            }
            Event::LifeLost { player, amount } => {
                Some(format!("seat {} loses {amount}", player.index()))
            }
            Event::CardDrawn { player, .. } => Some(format!("seat {} draws", player.index())),
            _ => None,
        })
        .collect()
}

/// A table of `seats` where seat 0 casts `spell` and seat 1 has a Bear.
fn charm_table(seats: usize, spell: Arc<CardDefinition>) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        table(seats, vec![spell]).battlefield(1, [ready(free_creature("Bear", 2, 2))]),
    )
}

#[rstest]
#[case::life(1, &[], &["seat 0 gains 3"])]
#[case::cards(2, &[], &["seat 0 draws"])]
#[case::damage(0, &[0], &["damage"])]
#[case::drain(3, &[1], &["seat 1 loses 2"])]
fn a_choose_one_spell_asks_only_the_targets_of_the_mode_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[case] mode: usize,
    #[case] aim: &[usize],
    #[case] happened: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = charm_table(seats, charm("Charm", 1, 1, false))?;
    driver.begin_cast("Charm")?;
    let options = modes(&driver);
    assert_eq!(offered(&options), [0, 1, 2, 3]);
    assert_eq!((options.min, options.max, options.repeat), (1, 1, false));
    assert!(options.can_cancel);
    assert_eq!(options.modes[1].label.as_deref(), Some("Life"));
    choose_modes(&mut driver, &[mode])?;
    // The targets of the mode chosen are asked and no others.
    for seat in aim {
        let DecisionKind::ChooseTargets(asked) = &driver.decision()?.kind else {
            panic!("the target of mode {mode} is asked");
        };
        assert_eq!(asked.spec, 0);
        let target = if mode == 0 {
            driver.target_named("Bear")?
        } else {
            driver.target_player(*seat)
        };
        driver.choose_targets(&[target])?;
    }
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    let begun = driver.engine().events().len();
    pay(&mut driver)?;
    let played = &driver.engine().events()[begun..];
    let resolution = played
        .iter()
        .position(|event| matches!(event, Event::ResolutionBegan { .. }))
        .unwrap();
    assert_eq!(story(&played[resolution..]), happened);
    Ok(())
}

#[rstest]
fn two_modes_are_asked_in_any_order_and_happen_in_the_order_printed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = charm_table(seats, charm("Double", 2, 2, false))?;
    driver.begin_cast("Double")?;
    // Drain (3) is answered before Damage (0); the card prints Damage first.
    choose_modes(&mut driver, &[3, 0])?;
    // The creature of mode 0 is asked first, then the player of mode 3.
    let DecisionKind::ChooseTargets(first) = &driver.decision()?.kind else {
        panic!("targets are asked");
    };
    assert_eq!(first.spec, 0);
    assert!(first.legal.iter().all(|t| matches!(t, Target::Object(_))));
    driver.choose_targets(&[driver.target_named("Bear")?])?;
    let DecisionKind::ChooseTargets(second) = &driver.decision()?.kind else {
        panic!("the second mode's targets are asked");
    };
    assert_eq!(second.spec, 1);
    assert!(second.legal.iter().all(|t| matches!(t, Target::Player(_))));
    driver.choose_targets(&[driver.target_player(seats - 1)])?;
    let begun = driver.engine().events().len();
    pay(&mut driver)?;
    let played = &driver.engine().events()[begun..];
    let resolution = played
        .iter()
        .position(|event| matches!(event, Event::ResolutionBegan { .. }))
        .unwrap();
    assert_eq!(
        story(&played[resolution..]),
        ["damage".to_owned(), format!("seat {} loses 2", seats - 1)]
    );
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Bear".to_owned()));
    Ok(())
}

#[rstest]
fn a_mode_chosen_twice_asks_its_targets_twice_and_happens_twice(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = charm_table(seats, charm("Triple", 3, 3, true))?;
    driver.begin_cast("Triple")?;
    let options = modes(&driver);
    assert!(options.repeat && (options.min, options.max) == (3, 3));
    // Life once and Drain twice, answered out of order: Life (1) happens first, then the two drains.
    choose_modes(&mut driver, &[3, 1, 3])?;
    for (index, seat) in [1, seats - 1].into_iter().enumerate() {
        let DecisionKind::ChooseTargets(asked) = &driver.decision()?.kind else {
            panic!("the drain asks for a player each time it was chosen");
        };
        assert_eq!(asked.spec, index);
        driver.choose_targets(&[driver.target_player(seat)])?;
    }
    let begun = driver.engine().events().len();
    pay(&mut driver)?;
    let played = &driver.engine().events()[begun..];
    let resolution = played
        .iter()
        .position(|event| matches!(event, Event::ResolutionBegan { .. }))
        .unwrap();
    assert_eq!(
        story(&played[resolution..]),
        [
            "seat 0 gains 3".to_owned(),
            "seat 1 loses 2".to_owned(),
            format!("seat {} loses 2", seats - 1)
        ]
    );
    Ok(())
}

#[test]
fn a_mode_whose_targets_cannot_be_met_is_not_offered() -> Result<(), ScriptError> {
    // No creature anywhere: the damage mode has nothing to aim at.
    let mut driver = Driver::from_scenario(table(2, vec![charm("Charm", 1, 1, false)]))?;
    driver.begin_cast("Charm")?;
    let options = modes(&driver);
    assert_eq!(offered(&options), [1, 2, 3]);
    let before = (driver.engine().state_hash(), driver.engine().events().len());
    let error = driver
        .answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))
        .unwrap_err();
    assert_eq!(
        error,
        ScriptError::Engine(EngineError::Illegal(IllegalAnswer::NotOffered("mode")))
    );
    assert_eq!(
        (driver.engine().state_hash(), driver.engine().events().len()),
        before
    );
    Ok(())
}

#[test]
fn backing_out_of_the_modes_leaves_no_trace() -> Result<(), ScriptError> {
    let mut driver = charm_table(3, charm("Charm", 1, 1, false))?;
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    driver.begin_cast("Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Cancel))?;
    assert_eq!(driver.engine().events().len(), log, "nothing was logged");
    assert_eq!(driver.engine().state_hash(), hash);
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Charm"]);
    assert_eq!(driver.state().stack(), []);
    // The spell can be cast again afterwards.
    driver.begin_cast("Charm")?;
    choose_modes(&mut driver, &[1])?;
    pay(&mut driver)?;
    driver.expect_life(0, 23)?;
    Ok(())
}

fn refused(driver: &mut Driver, answer: Answer, why: IllegalAnswer) {
    let (hash, log, pending) = (
        driver.engine().state_hash(),
        driver.engine().events().len(),
        driver.decision().unwrap().clone(),
    );
    assert_eq!(
        driver.answer(answer).unwrap_err(),
        ScriptError::Engine(EngineError::Illegal(why))
    );
    assert_eq!(
        driver.engine().state_hash(),
        hash,
        "a refusal changes nothing"
    );
    assert_eq!(driver.engine().events().len(), log);
    assert_eq!(
        driver.decision().unwrap(),
        &pending,
        "the same question stands"
    );
}

#[test]
fn an_illegal_answer_to_the_modes_is_refused_by_name_and_changes_nothing() -> Result<(), ScriptError>
{
    let mut driver = charm_table(2, charm("Double", 2, 2, false))?;
    driver.begin_cast("Double")?;
    let answer = |modes: &[usize]| Answer::ChooseMode(ModeChoice::Modes(modes.to_vec()));
    refused(
        &mut driver,
        answer(&[0, 7]),
        IllegalAnswer::NotOffered("mode"),
    );
    refused(
        &mut driver,
        answer(&[1, 1]),
        IllegalAnswer::Duplicate("mode"),
    );
    refused(
        &mut driver,
        answer(&[1]),
        IllegalAnswer::WrongCount {
            expected: 2,
            got: 1,
        },
    );
    refused(
        &mut driver,
        answer(&[]),
        IllegalAnswer::WrongCount {
            expected: 2,
            got: 0,
        },
    );
    refused(
        &mut driver,
        answer(&[0, 1, 2]),
        IllegalAnswer::WrongCount {
            expected: 2,
            got: 3,
        },
    );
    refused(
        &mut driver,
        Answer::Priority(PriorityAction::Pass),
        IllegalAnswer::WrongKind {
            expected: "choose-mode",
            got: "priority",
        },
    );
    // A legal answer still goes through afterwards.
    choose_modes(&mut driver, &[2, 1])?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    Ok(())
}

#[test]
fn an_illegal_answer_to_a_may_or_a_choice_is_refused() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(3, vec![scorn("Scorn")]))?;
    driver.begin_cast("Scorn")?;
    pay(&mut driver)?;
    refused(
        &mut driver,
        Answer::ChooseOption(OptionChoice(2)),
        IllegalAnswer::NotOffered("option"),
    );
    refused(
        &mut driver,
        Answer::YesNo(YesNoChoice(true)),
        IllegalAnswer::WrongKind {
            expected: "choose-option",
            got: "yes-no",
        },
    );
    driver.answer(Answer::ChooseOption(OptionChoice(1)))?;
    settle(&mut driver)?;
    assert_eq!(victim(&driver, 3), [2]);
    Ok(())
}

/// What a player who casts the spell and takes the first of everything answers.
fn answer_for(decision: &Decision, spell: bool) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => Answer::Priority(match options.castable.first() {
            Some(card) if spell && decision.player == pid(0) => PriorityAction::CastSpell(*card),
            _ => PriorityAction::Pass,
        }),
        DecisionKind::ChooseMode(options) => Answer::ChooseMode(ModeChoice::Modes(
            options
                .modes
                .iter()
                .take(options.min as usize)
                .map(|m| m.index)
                .collect(),
        )),
        DecisionKind::ChooseTargets(options) => {
            Answer::ChooseTargets(mtg_engine::choose::TargetChoice::Targets(vec![
                options.legal[0],
            ]))
        }
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
        DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
        other => panic!("the script does not expect {}", other.name()),
    }
}

/// The scenarios whose every decision is resumed and replayed, and the kind of decision each is about: a modal spell
/// at four seats, a "may" at another seat and a choice among opponents.
fn resumable() -> Vec<(&'static str, Scenario, &'static str)> {
    vec![
        (
            "charm",
            table(4, vec![charm("Double", 2, 2, false)])
                .battlefield(1, [ready(free_creature("Bear", 2, 2))]),
            "choose-mode",
        ),
        ("gift", table(3, vec![gift("Gift")]), "yes-no"),
        ("scorn", table(4, vec![scorn("Scorn")]), "choose-option"),
    ]
}

/// Plays the scenario from the cast to the end of the resolution, calling `at` with the driver before each answer.
fn play_through(
    scenario: Scenario,
    mut at: impl FnMut(&Driver, &Decision) -> Result<(), ScriptError>,
) -> Result<Vec<&'static str>, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    let (mut kinds, mut cast) = (Vec::new(), false);
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        at(&driver, &decision)?;
        kinds.push(decision.kind.name());
        cast |= matches!(decision.kind, DecisionKind::PayMana(_));
        driver.answer(answer_for(&decision, !cast))?;
        if cast && driver.state().stack().is_empty() {
            break;
        }
    }
    assert!(driver.state().stack().is_empty(), "the spell resolved");
    Ok(kinds)
}

#[test]
fn an_engine_rebuilt_from_the_log_asks_the_same_question_at_every_decision()
-> Result<(), ScriptError> {
    for (name, scenario, kind) in resumable() {
        let setup: GameSetup = scenario.clone().setup()?;
        let kinds = play_through(scenario, |driver, decision| {
            let resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
            assert_eq!(resumed.state_hash(), driver.engine().state_hash(), "{name}");
            // A half-finished cast lives outside the state, so a rebuilt engine asks for priority again.
            let mid_cast = matches!(
                decision.kind,
                DecisionKind::PayMana(_)
                    | DecisionKind::ChooseMode(_)
                    | DecisionKind::ChooseTargets(_)
            );
            if !mid_cast {
                assert_eq!(resumed.decision(), Some(decision), "{name}");
            }
            Ok(())
        })?;
        assert!(
            kinds.contains(&kind),
            "{name}: {kind} was asked ({kinds:?})"
        );
    }
    Ok(())
}

#[test]
fn the_log_replays_to_the_state_it_was_played_to_at_every_decision() -> Result<(), ScriptError> {
    for (name, scenario, _) in resumable() {
        let setup = scenario.clone().setup()?;
        let mut points = Vec::new();
        let mut log = Vec::new();
        play_through(scenario, |driver, _| {
            points.push((driver.engine().events().len(), driver.engine().state_hash()));
            log = driver.engine().events().to_vec();
            Ok(())
        })?;
        assert!(points.len() > 4, "{name}");
        for (length, hash) in points {
            assert_eq!(
                replay(&setup, &log[..length])?.state_hash(),
                hash,
                "{name} after {length} events"
            );
        }
    }
    Ok(())
}

#[test]
fn a_player_who_has_left_is_not_asked_and_the_spell_goes_on() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(3, vec![gift("Gift")]))?;
    driver.begin_cast("Gift")?;
    driver.choose_targets(&[driver.target_player(2)])?;
    pay(&mut driver)?;
    assert_eq!(yes_no(&driver).0.player, PlayerId(2));
    driver.concede(pid(2))?;
    settle(&mut driver)?;
    assert_eq!(driver.state().stack(), []);
    Ok(())
}
