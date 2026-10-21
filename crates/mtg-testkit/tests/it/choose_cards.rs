//! Goal: Archon of Cruelty plays whole, as its card says, at tables of two, three and four seats: the target opponent chooses what to sacrifice among their creatures and planeswalkers and what to discard from their hand and loses 3 life while its controller draws a card and gains 3, whether it entered or attacked; nothing to sacrifice, an empty hand or a single candidate are taken without a question; an answer that was not offered, repeats a card or has the wrong number is refused by name and changes nothing; an engine rebuilt from the log asks the same question at each decision; and the log replays to the state it was played to at each of them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, names, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{edict, forest, free_creature, mind_rot, mountain, swamp, walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ARCHON: &str = "Archon of Cruelty";
/// The target's permanents in id order: three that can be sacrificed to the Archon and two that cannot.
const BOARD: [&str; 5] = [
    "Grizzly Bears",
    "Serra Angel",
    "Jace, the Mind Sculptor",
    "Sol Ring",
    "Forest",
];
const HAND: [&str; 3] = ["Lightning Bolt", "Giant Growth", "Counterspell"];
const SACRIFICE: &str = "Sacrifice a creature or planeswalker.";
const DISCARD: &str = "Discard a card.";

/// How the Archon's ability gets going.
#[derive(Debug, Clone, Copy)]
enum Trigger {
    /// Seat 0 casts it for {6}{B}{B} from two Swamps and six Forests.
    Enters,
    /// It is on the battlefield of seat 0 at the declaration of attackers, and attacks the target.
    Attacks,
}

/// Seat 0 has the Archon; `target` has `board` and `hand`; every other seat has two creatures and two cards.
fn scenario(
    seats: usize,
    target: usize,
    trigger: Trigger,
    board: &[&str],
    hand: &[&str],
) -> Scenario {
    let mut scenario = match trigger {
        Trigger::Enters => Scenario::new(seats).hand_named(0, [ARCHON]).battlefield(
            0,
            [
                swamp(),
                swamp(),
                forest(),
                forest(),
                forest(),
                forest(),
                forest(),
                forest(),
            ]
            .map(ready),
        ),
        Trigger::Attacks => Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield_named(0, [ARCHON]),
    };
    for seat in 1..seats {
        scenario = if seat == target {
            scenario
                .battlefield_named(seat, board.iter().copied())
                .hand_named(seat, hand.iter().copied())
        } else {
            scenario
                .battlefield_named(seat, ["Grizzly Bears", "Llanowar Elves"])
                .hand_named(seat, ["Opt", "Brainstorm"])
        };
    }
    scenario
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

/// Gets the Archon's ability on the stack aimed at `target`.
fn start(driver: &mut Driver, trigger: Trigger, target: usize) -> Result<(), ScriptError> {
    match trigger {
        Trigger::Enters => {
            driver.cast(ARCHON)?;
        }
        Trigger::Attacks => {
            driver.attack(&[(ARCHON, target)])?;
        }
    }
    passes_until_asked(driver)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the trigger asks its controller for a target");
    };
    assert!(options.triggered && options.legal.len() == driver.state().players().len() - 1);
    driver.choose_targets(&[driver.target_player(target)])?;
    passes_until_asked(driver)
}

/// The question pending, if it is a choice of cards.
fn question(driver: &Driver) -> Option<(Decision, mtg_engine::choose::CardsOptions)> {
    let decision = driver.decision().ok()?.clone();
    let DecisionKind::ChooseCards(options) = decision.kind.clone() else {
        return None;
    };
    Some((decision, options))
}

fn choose(driver: &mut Driver, cards: &[ObjectId]) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::ChooseCards(CardsChoice(cards.to_vec())))
}

fn from_names(driver: &Driver, cards: &[ObjectId]) -> Vec<String> {
    cards
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap()
}

/// What happened in a resolution, as the words the card uses.
fn story(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ChoiceMade { .. } => Some("choice"),
            Event::ZoneChanged {
                cause: ZoneCause::Sacrificed,
                ..
            } => Some("sacrificed"),
            Event::CardDiscarded { .. } => Some("discarded"),
            Event::LifeLost { amount: 3, .. } => Some("lost 3"),
            Event::CardDrawn { .. } => Some("drawn"),
            Event::LifeGained { amount: 3, .. } => Some("gained 3"),
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            } => Some("resolved"),
            _ => None,
        })
        .collect()
}

fn steps(events: &[Event]) -> Vec<Vec<usize>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::StepDone { position, .. } => Some(position.clone()),
            _ => None,
        })
        .collect()
}

#[rstest]
fn the_target_chooses_what_to_sacrifice_and_discard_and_the_text_is_played_whole(
    #[values((2, 1), (3, 1), (3, 2), (4, 1), (4, 3))] table: (usize, usize),
    #[values(Trigger::Enters, Trigger::Attacks)] trigger: Trigger,
) -> Result<(), ScriptError> {
    let (seats, target) = table;
    let mut driver = Driver::from_scenario(scenario(seats, target, trigger, &BOARD, &HAND))?;
    let bystanders: Vec<(usize, Vec<String>, Vec<String>)> = (1..seats)
        .filter(|seat| *seat != target)
        .map(|seat| {
            (
                seat,
                names(driver.state(), Zone::Hand(pid(seat))),
                names(driver.state(), Zone::Graveyard(pid(seat))),
            )
        })
        .collect();
    start(&mut driver, trigger, target)?;
    let begun = driver.engine().events().len();

    // The target player is asked, not the controller, and only for their creatures and planeswalkers.
    let (decision, options) = question(&driver).expect("the sacrifice is asked");
    assert_eq!(decision.player, pid(target));
    assert_eq!(options.source, driver.id_of(ARCHON)?);
    assert_eq!(
        from_names(&driver, &options.from),
        ["Grizzly Bears", "Serra Angel", "Jace, the Mind Sculptor"]
    );
    assert_eq!((options.min, options.max), (1, 1));
    assert_eq!(options.text, SACRIFICE);
    let angel = options.from[1];
    choose(&mut driver, &[angel])?;

    // Then they are asked for a card of their own hand.
    let (decision, options) = question(&driver).expect("the discard is asked");
    assert_eq!(decision.player, pid(target));
    assert_eq!(from_names(&driver, &options.from), HAND);
    assert_eq!(options.from, driver.state().zone(Zone::Hand(pid(target))));
    assert_eq!((options.min, options.max), (1, 1));
    assert_eq!(options.text, DISCARD);
    let growth = options.from[1];
    choose(&mut driver, &[growth])?;
    passes_until_asked(&mut driver)?;

    let played = &driver.engine().events()[begun..];
    assert_eq!(
        story(played),
        [
            "choice",
            "sacrificed",
            "choice",
            "discarded",
            "lost 3",
            "drawn",
            "gained 3",
            "resolved"
        ]
    );
    assert_eq!(steps(played), [[1], [2], [3], [4], [5]]);
    driver.expect_life(target, 17)?;
    driver.expect_life(0, 23)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(target))),
        ["Serra Angel", "Giant Growth"]
    );
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(target))),
        ["Lightning Bolt", "Counterspell"]
    );
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Serra Angel".to_owned()));
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        1,
        "the controller drew a card"
    );
    for (seat, hand, graveyard) in bystanders {
        assert_eq!(names(driver.state(), Zone::Hand(pid(seat))), hand);
        assert_eq!(names(driver.state(), Zone::Graveyard(pid(seat))), graveyard);
        driver.expect_life(seat, 20)?;
    }
    Ok(())
}

#[rstest]
#[case::nothing_to_sacrifice(&["Sol Ring", "Forest"], &HAND, &[DISCARD])]
#[case::an_empty_hand(&BOARD, &[], &[SACRIFICE])]
#[case::one_creature_and_one_card(&["Grizzly Bears", "Sol Ring"], &["Opt"], &[])]
#[case::one_creature_and_a_hand(&["Grizzly Bears", "Sol Ring"], &HAND, &[DISCARD])]
#[case::a_planeswalker_alone(&["Jace, the Mind Sculptor"], &HAND, &[DISCARD])]
#[case::two_creatures_and_one_card(&["Grizzly Bears", "Serra Angel"], &["Opt"], &[SACRIFICE])]
#[case::nothing_at_all(&["Forest"], &[], &[])]
fn what_there_is_no_choice_about_is_taken_without_a_question(
    #[case] board: &[&str],
    #[case] hand: &[&str],
    #[case] asked: &[&str],
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(seats, 1, Trigger::Attacks, board, hand))?;
    let (candidates, cards) = (
        sacrificable(&driver, 1),
        driver.state().zone(Zone::Hand(pid(1))).len(),
    );
    start(&mut driver, Trigger::Attacks, 1)?;
    let mut texts = Vec::new();
    while let Some((decision, options)) = question(&driver) {
        assert_eq!(decision.player, pid(1));
        texts.push(options.text.clone());
        let first = options.from[0];
        choose(&mut driver, &[first])?;
        passes_until_asked(&mut driver)?;
    }
    assert_eq!(texts, asked);
    let story = story(driver.engine().events());
    let sacrificed = story.iter().filter(|s| **s == "sacrificed").count();
    let discarded = story.iter().filter(|s| **s == "discarded").count();
    assert_eq!(sacrificed, usize::from(candidates > 0));
    assert_eq!(discarded, usize::from(cards > 0));
    assert_eq!(
        story.iter().filter(|s| **s == "choice").count(),
        asked.len()
    );
    assert!(story.ends_with(&["lost 3", "drawn", "gained 3", "resolved"]));
    driver.expect_life(1, 17)?;
    driver.expect_life(0, 23)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(1))).len(),
        cards - discarded
    );
    assert_eq!(sacrificable(&driver, 1), candidates - sacrificed);
    Ok(())
}

/// How many creatures and planeswalkers `seat` controls.
fn sacrificable(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == pid(seat))
        .filter(|object| {
            ["Grizzly Bears", "Serra Angel", "Jace, the Mind Sculptor"]
                .contains(&object.card().name.as_str())
        })
        .count()
}

/// Seat 0's Archon is aimed at seat 1 and the sacrifice is pending.
fn at_the_sacrifice() -> Result<(Driver, GameSetup), ScriptError> {
    let scenario = scenario(2, 1, Trigger::Attacks, &BOARD, &HAND);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    start(&mut driver, Trigger::Attacks, 1)?;
    assert!(question(&driver).is_some());
    Ok((driver, setup))
}

fn refused(driver: &mut Driver, answer: Answer, expected: IllegalAnswer) {
    let (hash, pending) = (
        driver.engine().state_hash(),
        driver.decision().unwrap().clone(),
    );
    let log = driver.engine().events().len();
    let error = driver.answer(answer).unwrap_err();
    assert_eq!(error, ScriptError::Engine(EngineError::Illegal(expected)));
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

fn cards(ids: &[ObjectId]) -> Answer {
    Answer::ChooseCards(CardsChoice(ids.to_vec()))
}

#[test]
fn an_illegal_answer_to_the_sacrifice_is_refused_by_name_and_changes_nothing()
-> Result<(), ScriptError> {
    let (mut driver, _) = at_the_sacrifice()?;
    let bears = id_in(&driver, Zone::Battlefield, "Grizzly Bears");
    let angel = id_in(&driver, Zone::Battlefield, "Serra Angel");
    let ring = id_in(&driver, Zone::Battlefield, "Sol Ring");
    let archon = driver.id_of(ARCHON)?;
    let in_hand = driver.state().zone(Zone::Hand(pid(1)))[0];
    let not_offered = IllegalAnswer::NotOffered("card");
    refused(&mut driver, cards(&[ring]), not_offered.clone());
    refused(&mut driver, cards(&[archon]), not_offered.clone());
    refused(&mut driver, cards(&[in_hand]), not_offered.clone());
    refused(
        &mut driver,
        cards(&[bears, bears]),
        IllegalAnswer::Duplicate("card"),
    );
    refused(
        &mut driver,
        cards(&[bears, angel]),
        IllegalAnswer::WrongCount {
            expected: 1,
            got: 2,
        },
    );
    refused(
        &mut driver,
        cards(&[]),
        IllegalAnswer::WrongCount {
            expected: 1,
            got: 0,
        },
    );
    refused(
        &mut driver,
        Answer::Priority(PriorityAction::Pass),
        IllegalAnswer::WrongKind {
            expected: "choose-cards",
            got: "priority",
        },
    );
    // A legal answer still goes through afterwards.
    choose(&mut driver, &[bears])?;
    assert!(question(&driver).is_some_and(|(_, o)| o.text == DISCARD));
    Ok(())
}

#[test]
fn an_illegal_answer_to_the_discard_is_refused_by_name_and_changes_nothing()
-> Result<(), ScriptError> {
    let (mut driver, _) = at_the_sacrifice()?;
    let bears = id_in(&driver, Zone::Battlefield, "Grizzly Bears");
    choose(&mut driver, &[bears])?;
    let hand = driver.state().zone(Zone::Hand(pid(1))).to_vec();
    let on_the_table = id_in(&driver, Zone::Battlefield, "Serra Angel");
    let in_the_graveyard = id_in(&driver, Zone::Graveyard(pid(1)), "Grizzly Bears");
    refused(
        &mut driver,
        cards(&[on_the_table]),
        IllegalAnswer::NotOffered("card"),
    );
    refused(
        &mut driver,
        cards(&[in_the_graveyard]),
        IllegalAnswer::NotOffered("card"),
    );
    refused(
        &mut driver,
        cards(&[hand[0], hand[0]]),
        IllegalAnswer::Duplicate("card"),
    );
    refused(
        &mut driver,
        cards(&hand[..2]),
        IllegalAnswer::WrongCount {
            expected: 1,
            got: 2,
        },
    );
    refused(
        &mut driver,
        cards(&[]),
        IllegalAnswer::WrongCount {
            expected: 1,
            got: 0,
        },
    );
    choose(&mut driver, &[hand[2]])?;
    passes_until_asked(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(1))),
        ["Grizzly Bears", "Counterspell"]
    );
    Ok(())
}

#[test]
fn an_engine_rebuilt_from_the_log_asks_the_same_question_and_goes_on_the_same_way()
-> Result<(), ScriptError> {
    for trigger in [Trigger::Enters, Trigger::Attacks] {
        let scenario = scenario(3, 2, trigger, &BOARD, &HAND);
        let setup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        let mut asked = 0;
        for _ in 0..200 {
            let decision = driver.decision()?.clone();
            let mut resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
            assert_eq!(resumed.state_hash(), driver.engine().state_hash());
            let mid_cast = matches!(&decision.kind, DecisionKind::PayMana(_))
                || matches!(&decision.kind, DecisionKind::ChooseTargets(o) if !o.triggered);
            if !mid_cast {
                // A half-finished cast lives outside the state; everything else is asked again as it was.
                assert_eq!(resumed.decision(), Some(&decision));
            }
            asked += usize::from(question(&driver).is_some());
            // Both engines take the same answer to the same events.
            let answer = answer_for(&decision, trigger, 2);
            let live = driver.answer(answer.clone())?;
            if !mid_cast {
                assert_eq!(resumed.apply(answer)?, live);
                assert_eq!(resumed.state_hash(), driver.engine().state_hash());
            }
            if asked == 2 && driver.state().stack().is_empty() {
                break;
            }
        }
        assert_eq!(asked, 2, "the sacrifice and the discard were both asked");
    }
    Ok(())
}

/// What a player who plays the Archon at `target` answers: cast it, attack with it, pay, aim, and take the first cards.
fn answer_for(decision: &Decision, trigger: Trigger, target: usize) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => {
            Answer::Priority(match (trigger, options.castable.first()) {
                (Trigger::Enters, Some(spell)) if decision.player == pid(0) => {
                    PriorityAction::CastSpell(*spell)
                }
                _ => PriorityAction::Pass,
            })
        }
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseTargets(options) => {
            let aimed = Target::Player(pid(target));
            Answer::ChooseTargets(TargetChoice::Targets(vec![
                if options.legal.contains(&aimed) {
                    aimed
                } else {
                    options.legal[0]
                },
            ]))
        }
        DecisionKind::ChooseCards(options) => cards(&options.from[..options.min as usize]),
        DecisionKind::DeclareAttackers(options) => Answer::DeclareAttackers(
            options
                .attackers
                .iter()
                .map(|attacker| (attacker.object, AttackTarget::Player(pid(target))))
                .collect(),
        ),
        DecisionKind::DeclareBlockers(_) => Answer::DeclareBlockers(Vec::new()),
        other => panic!("the script does not expect {}", other.name()),
    }
}

#[test]
fn the_log_replays_to_the_state_it_was_played_to_at_every_decision() -> Result<(), ScriptError> {
    let scenario = scenario(4, 3, Trigger::Enters, &BOARD, &HAND);
    let setup = scenario.clone().setup()?;
    let mut driver = Driver::from_scenario(scenario)?;
    let mut points = Vec::new();
    for _ in 0..200 {
        points.push((driver.engine().events().len(), driver.engine().state_hash()));
        let answer = answer_for(driver.decision()?, Trigger::Enters, 3);
        driver.answer(answer)?;
        if driver.state().stack().is_empty()
            && driver.state().zone(Zone::Graveyard(pid(3))).len() == 2
        {
            break;
        }
    }
    points.push((driver.engine().events().len(), driver.engine().state_hash()));
    let log = driver.engine().events().to_vec();
    assert!(points.len() > 5);
    for (length, hash) in points {
        assert_eq!(
            replay(&setup, &log[..length])?.state_hash(),
            hash,
            "after {length} events"
        );
    }
    Ok(())
}

#[test]
fn spells_that_make_a_player_sacrifice_or_discard_ask_that_player() -> Result<(), ScriptError> {
    // Seat 0 casts an edict and then a discard spell at seat 2 of three.
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .hand(0, [edict("Edict"), mind_rot("Rot")])
            .battlefield(
                0,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
            .battlefield(
                2,
                [
                    ready(free_creature("Ogre", 3, 3)),
                    ready(free_creature("Wolf", 2, 2)),
                    ready(walker("Walker", 0)),
                ],
            )
            .hand_named(2, ["Opt", "Brainstorm", "Counterspell"]),
    )?;
    driver.cast_at("Edict", &[driver.target_player(2)])?;
    passes_until_asked(&mut driver)?;
    let (decision, options) = question(&driver).expect("the edict asks");
    assert_eq!(decision.player, pid(2));
    assert_eq!(options.text, "Sacrifice a creature.");
    assert_eq!(from_names(&driver, &options.from), ["Ogre", "Wolf"]);
    let wolf = options.from[1];
    choose(&mut driver, &[wolf])?;
    passes_until_asked(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(2))), ["Wolf"]);

    driver.cast_at("Rot", &[driver.target_player(2)])?;
    passes_until_asked(&mut driver)?;
    let (decision, options) = question(&driver).expect("the discard spell asks");
    assert_eq!(decision.player, pid(2));
    assert_eq!(options.text, "Discard 2 cards.");
    assert_eq!((options.min, options.max), (2, 2));
    choose(&mut driver, &options.from[1..])?;
    passes_until_asked(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(2))), ["Opt"]);
    assert_eq!(driver.state().stack(), []);
    Ok(())
}

#[test]
fn faithless_looting_asks_its_controller_to_discard_two_after_drawing_two()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Faithless Looting", "Opt"])
            .battlefield(0, [ready(mountain())])
            .library_named(0, ["Forest", "Forest", "Sol Ring", "Forest"]),
    )?;
    driver.cast("Faithless Looting")?;
    passes_until_asked(&mut driver)?;
    let (decision, options) = question(&driver).expect("the controller is asked");
    assert_eq!(decision.player, pid(0));
    assert_eq!(options.text, "Discard 2 cards.");
    // Opt and the two cards just drawn are in the hand.
    assert_eq!(options.from.len(), 3);
    choose(&mut driver, &options.from[..2])?;
    passes_until_asked(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 1);
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).len(),
        3,
        "the sorcery and the two discards"
    );
    Ok(())
}

#[test]
fn a_target_that_left_before_the_ability_resolved_leaves_everything_alone()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(3, 2, Trigger::Attacks, &BOARD, &HAND))?;
    driver.attack(&[(ARCHON, 2)])?;
    passes_until_asked(&mut driver)?;
    driver.choose_targets(&[Target::Player(pid(2))])?;
    driver.concede(pid(2))?;
    passes_until_asked(&mut driver)?;
    assert!(question(&driver).is_none(), "nobody is asked");
    driver.expect_life(0, 20)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        0,
        "the controller did not draw"
    );
    Ok(())
}
