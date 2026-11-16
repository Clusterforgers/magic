//! Goal: loops at tables of two to four seats with corpus cards (CR 104.4b, 732): a loop of mandatory actions that never ends is a draw for everyone still in the game, one that ends (a life drain) runs to the end of the game, a loop driven by a choice ("may", a target, an activation) is offered to its driver, who runs N iterations in one batch summed up by one event, a player who could respond is asked about a mandatory loop and may let it be a draw, and a third player can still respond once the batch is over.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::Target;
use mtg_engine::choose::{OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::loops::{LoopAnswer, LoopOptions, LoopSummary};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameResult, replay};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers the table's questions with `policy`, passively where it has none, until a loop is offered, the game ends or
/// `limit` answers are given; returns the loop offer if one came.
fn play(
    driver: &mut Driver,
    limit: usize,
    mut policy: impl FnMut(&Driver, &Decision) -> Option<Answer>,
) -> Result<Option<(PlayerId, LoopOptions)>, ScriptError> {
    for _ in 0..limit {
        if driver.state().result().is_some() {
            return Ok(None);
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::Loop(options) = &decision.kind {
            return Ok(Some((decision.player, options.clone())));
        }
        let answer = policy(driver, &decision).unwrap_or_else(|| Driver::passive_answer(&decision));
        driver.answer(answer)?;
    }
    Ok(None)
}

fn tokens_named(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

fn summaries(events: &[Event]) -> Vec<LoopSummary> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::LoopRepeated(summary) => Some((**summary).clone()),
            _ => None,
        })
        .collect()
}

fn forests(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land(Color::Green))).collect()
}

/// Seat 0 casts Polyraptor beside `damager` (Aether Flash deals 2 to each creature that enters; Where Ancients Tread
/// may deal 5 to any target); `responder` holds a Lightning Bolt with a Mountain.
fn polyraptor_table(seats: usize, damager: &str, responder: Option<usize>) -> Driver {
    let mut permanents = forests(8);
    permanents.push(ready(card(damager).unwrap()));
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Polyraptor").unwrap()])
        .battlefield(0, permanents);
    if let Some(seat) = responder {
        scenario = scenario
            .hand(seat, [card("Lightning Bolt").unwrap()])
            .battlefield(seat, [ready(land(Color::Red))]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Polyraptor").unwrap();
    driver
}

#[rstest]
fn a_mandatory_loop_that_never_ends_is_a_draw_for_everyone_still_in_the_game(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = polyraptor_table(seats, "Aether Flash", None);
    let offer = play(&mut driver, 400, |_, _| None)?;
    assert_eq!(offer, None, "no one has a choice, so no one is asked");
    assert_eq!(driver.state().result(), Some(GameResult::Draw));
    assert!(
        driver
            .state()
            .loop_draw()
            .is_some_and(|cards| cards.iter().any(|c| c == "Polyraptor"))
    );
    assert!((0..seats).all(|seat| driver.state().is_alive(pid(seat))));
    assert!(
        tokens_named(&driver, "Polyraptor") < 8,
        "found within a few cycles"
    );
    let events = driver.engine().events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::LoopDrawn { .. }))
    );
    let replayed = replay(driver.engine().setup(), events).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_player_who_could_respond_is_asked_and_may_let_the_loop_be_a_draw(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let responder = seats - 1;
    let mut driver = polyraptor_table(seats, "Aether Flash", Some(responder));
    let (asked, options) = play(&mut driver, 400, |_, _| None)?.expect("the responder is asked");
    assert_eq!(asked, pid(responder));
    assert!(options.mandatory && options.can_draw && !options.bounded);
    assert_eq!(options.asked, 0);
    driver.answer(Answer::Loop(LoopAnswer::Stop))?;
    // Played on by hand, the loop comes round again and is offered again.
    let (again, options) = play(&mut driver, 400, |_, _| None)?.expect("asked again");
    assert_eq!((again, options.asked), (pid(responder), 1));
    driver.answer(Answer::Loop(LoopAnswer::Draw))?;
    assert_eq!(driver.state().result(), Some(GameResult::Draw));
    Ok(())
}

/// The newest Polyraptor is Where Ancients Tread's target; its "may" is yes while `yes` holds.
fn tread(driver: &Driver, decision: &Decision, yes: bool) -> Option<Answer> {
    match &decision.kind {
        DecisionKind::ChooseTargets(options) => {
            let newest = options
                .legal
                .iter()
                .filter_map(|target| match target {
                    Target::Object(object)
                        if name_of(driver.state(), object.id) == "Polyraptor" =>
                    {
                        Some(*target)
                    }
                    _ => None,
                })
                .max_by_key(|target| match target {
                    Target::Object(object) => object.id,
                    Target::Player(_) => ObjectId(0),
                })?;
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![newest])))
        }
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(yes))),
        _ => None,
    }
}

#[rstest]
fn a_may_loop_is_offered_to_its_driver_and_runs_the_number_chosen_in_one_batch(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = polyraptor_table(seats, "Where Ancients Tread", None);
    let (asked, options) = play(&mut driver, 400, |d, decision| tread(d, decision, true))?
        .expect("the driver is asked");
    assert_eq!(asked, pid(0));
    assert!(!options.mandatory && !options.can_draw && !options.bounded);
    for name in ["Polyraptor", "Where Ancients Tread"] {
        assert!(options.cards.iter().any(|card| card == name), "{name}");
    }
    assert!(options.max >= 1000);
    let before = driver.engine().events().len();
    let events = driver.answer(Answer::Loop(LoopAnswer::Repeat(1000)))?;
    let summary = summaries(&events).pop().expect("one summary");
    assert_eq!(summary.player, Some(pid(0)));
    assert_eq!(
        summary.iterations, 1000,
        "a loop that stays small runs the thousand"
    );
    let made: u32 = summary
        .total
        .tokens
        .iter()
        .filter(|t| t.name == "Polyraptor")
        .map(|t| t.count)
        .sum();
    assert_eq!(made, 1000);
    assert_eq!(
        summary.events as usize,
        driver.engine().events().len() - before - 1
    );
    // Normal play resumes at the loop's start; the driver says no and the loop ends.
    let offer = play(&mut driver, 40, |d, decision| tread(d, decision, false))?;
    assert_eq!(offer, None);
    assert_eq!(driver.state().stack(), &[] as &[ObjectId]);
    assert_eq!(tokens_named(&driver, "Polyraptor"), 1);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_life_drain_loop_is_no_draw_and_ends_with_the_game(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents: Vec<_> = (0..3).map(|_| ready(land(Color::White))).collect();
    permanents.push(ready(card("Sanguine Bond").unwrap()));
    permanents.push(ready(card("Exquisite Blood").unwrap()));
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Renewed Faith").unwrap()])
        .battlefield(0, permanents);
    for seat in 1..seats {
        scenario = scenario.life(seat, 300);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Renewed Faith")?;
    let mut offers = 0;
    while driver.state().result().is_none() {
        match play(&mut driver, 2000, |_, _| None)? {
            Some((asked, options)) => {
                assert_eq!(asked, pid(0), "the controller chooses the targets");
                assert!(options.bounded && !options.can_draw && !options.mandatory);
                offers += 1;
                driver.answer(Answer::Loop(LoopAnswer::Repeat(options.max)))?;
            }
            None => break,
        }
    }
    assert_eq!(driver.state().result(), Some(GameResult::Winner(pid(0))));
    assert!(driver.state().loop_draw().is_none());
    let ran = summaries(driver.engine().events());
    assert!(!ran.is_empty(), "the loop runs as a shortcut");
    if seats == 2 {
        assert_eq!(
            offers, 0,
            "one opponent: every target is forced, so the loop runs by itself"
        );
        assert_eq!(ran[0].player, None);
    } else {
        assert!(offers >= 1);
    }
    Ok(())
}

/// Seat 0 activates Kiki-Jiki at an empty stack, copying `creature`, whose copy untaps Kiki-Jiki.
fn kiki(kiki: ObjectId, creature: ObjectId) -> impl FnMut(&Driver, &Decision) -> Option<Answer> {
    move |driver, decision| match &decision.kind {
        DecisionKind::Priority(options)
            if decision.player == pid(0) && driver.state().stack().is_empty() =>
        {
            options
                .abilities
                .iter()
                .find(|ability| ability.object == kiki)
                .map(|ability| Answer::Priority(PriorityAction::Activate(*ability)))
        }
        DecisionKind::ChooseTargets(options) => {
            let wanted = if options.source == kiki {
                creature
            } else {
                kiki
            };
            options
                .legal
                .iter()
                .find(|target| matches!(target, Target::Object(object) if object.id == wanted))
                .map(|target| Answer::ChooseTargets(TargetChoice::Targets(vec![*target])))
        }
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(true))),
        // Pestermite untaps.
        DecisionKind::ChooseOption(_) => Some(Answer::ChooseOption(OptionChoice(1))),
        _ => None,
    }
}

fn kiki_table(seats: usize, creature: &str, bolt: Option<usize>) -> (Driver, ObjectId, ObjectId) {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Kiki-Jiki, Mirror Breaker").unwrap()),
            ready(card(creature).unwrap()),
        ],
    );
    if let Some(seat) = bolt {
        scenario = scenario
            .hand(seat, [card("Lightning Bolt").unwrap()])
            .battlefield(seat, [ready(land(Color::Red))]);
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let kiki = driver.id_of("Kiki-Jiki, Mirror Breaker").unwrap();
    let copied = driver.id_of(creature).unwrap();
    (driver, kiki, copied)
}

#[rstest]
fn the_controller_of_kiki_jiki_chooses_how_many_copies(
    #[values(2, 3, 4)] seats: usize,
    #[values("Pestermite", "Zealous Conscripts")] creature: &str,
) -> Result<(), ScriptError> {
    let (mut driver, kiki_id, copied) = kiki_table(seats, creature, None);
    let (asked, options) =
        play(&mut driver, 400, kiki(kiki_id, copied))?.expect("the controller is asked");
    assert_eq!(asked, pid(0));
    assert!(!options.mandatory && !options.bounded && !options.can_draw);
    let made: u32 = options.per_iteration.tokens.iter().map(|t| t.count).sum();
    assert_eq!(made, 1, "one copy an iteration");
    let before = tokens_named(&driver, creature);
    let events = driver.answer(Answer::Loop(LoopAnswer::Repeat(50)))?;
    assert_eq!(summaries(&events).pop().map(|s| s.iterations), Some(50));
    assert_eq!(tokens_named(&driver, creature), before + 50);
    // The batch ends at the start of the loop with priority, where the controller stops.
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.decision()?.player, pid(0));
    // Out of range: no iterations, more than the cap.
    let options = play(&mut driver, 400, kiki(kiki_id, copied))?
        .expect("offered again")
        .1;
    assert_eq!(options.asked, 1);
    for wrong in [0, options.max + 1] {
        assert!(
            driver
                .answer(Answer::Loop(LoopAnswer::Repeat(wrong)))
                .is_err()
        );
    }
    assert!(
        driver.answer(Answer::Loop(LoopAnswer::Draw)).is_err(),
        "a loop with a driver is no draw"
    );
    driver.answer(Answer::Loop(LoopAnswer::Stop))?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_third_player_responds_after_the_batch_and_breaks_the_loop(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let (mut driver, kiki_id, pestermite) = kiki_table(seats, "Pestermite", Some(2));
    let (asked, _) =
        play(&mut driver, 400, kiki(kiki_id, pestermite))?.expect("the controller is asked");
    assert_eq!(
        asked,
        pid(0),
        "the driver is asked, not the player who could respond"
    );
    driver.answer(Answer::Loop(LoopAnswer::Repeat(10)))?;
    let mut policy = kiki(kiki_id, pestermite);
    let mut bolted = false;
    let offer = play(&mut driver, 400, |d, decision| {
        if let DecisionKind::PayMana(_) = &decision.kind
            && decision.player == pid(2)
        {
            return Some(Answer::PayMana(PayChoice::Auto));
        }
        if decision.player == pid(2)
            && !d.state().stack().is_empty()
            && !bolted
            && let DecisionKind::Priority(options) = &decision.kind
        {
            bolted = true;
            return options
                .castable
                .first()
                .map(|spell| Answer::Priority(PriorityAction::CastSpell(*spell)));
        }
        if let DecisionKind::ChooseTargets(_) = &decision.kind
            && decision.player == pid(2)
        {
            return Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                Target::Object(d.state().object(kiki_id).unwrap().reference()),
            ])));
        }
        if d.state().turn().number() > 1 {
            return None;
        }
        policy(d, decision)
    })?;
    assert!(
        offer.is_none(),
        "the loop is broken, so it is not offered again"
    );
    assert!(bolted);
    assert!(
        driver
            .state()
            .object(kiki_id)
            .is_none_or(|kiki| kiki.zone() != Zone::Battlefield)
    );
    Ok(())
}

#[test]
fn a_growing_loop_stops_at_the_work_a_shortcut_may_do() -> Result<(), ScriptError> {
    let (mut driver, kiki_id, pestermite) = kiki_table(2, "Pestermite", None);
    play(&mut driver, 400, kiki(kiki_id, pestermite))?.expect("the controller is asked");
    let before = tokens_named(&driver, "Pestermite");
    let events = driver.answer(Answer::Loop(LoopAnswer::Repeat(1000)))?;
    let ran = summaries(&events).pop().expect("one summary").iterations;
    assert!((200..1000).contains(&ran), "{ran} iterations");
    assert_eq!(tokens_named(&driver, "Pestermite"), before + ran as usize);
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    Ok(())
}
