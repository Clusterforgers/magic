//! Goal: the smoke gate passes Verified corpus cards that the games use, counts how each kind of card is used, and names the failure of a rules file whose effect errors, a card that loops, a card that can never be cast and a card the games never meet.
use std::sync::Arc;
use std::time::Duration;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::support::Support;
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::color::Color;
use mtg_testkit::cards::{creature, gain_life, instant};
use mtg_testkit::corpus::corpus;
use mtg_testkit::harness::{
    FailureKind, GameFailure, GameOutcome, PlayOptions, bots_for, play_game_with_bots,
};
use mtg_testkit::smoke::{DEFAULT_SEEDS, SmokeFailure, SmokeReport, Uses, smoke_card, smoke_setup};
use mtg_testkit::trigger_cards::{artifact_with, creature_with, trigger, when, with_rules};
use rstest::rstest;

fn corpus_card(name: &str) -> Result<Arc<CardDefinition>, String> {
    let corpus = corpus().map_err(|e| e.to_string())?;
    let support = corpus.support(name).map_err(|e| e.to_string())?;
    if support.support != Support::Verified {
        return Err(format!("{name} is {:?}, not Verified", support.support));
    }
    corpus.card(name).map_err(|e| e.to_string())
}

fn smoke(card: &CardDefinition) -> SmokeReport {
    smoke_card(card, &DEFAULT_SEEDS)
}

/// An instant for `{G}` whose only effect names something no step bound: the engine cannot resolve it.
fn erroring_spell() -> Arc<CardDefinition> {
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: vec![Effect::GainLife {
            who: Ref::Bound("nothing".to_owned()),
            amount: Amount::Fixed(1),
        }],
    };
    with_rules(&instant("Erroring Spell", 0, &[Color::Green]), vec![spell])
}

/// A free creature that gains its controller life as it enters and whenever anyone gains life: a loop with no end.
fn looping_creature() -> Arc<CardDefinition> {
    creature_with(
        "Looping Creature",
        vec![
            trigger(when::this_enters(), vec![gain_life(1)]),
            trigger(
                TriggerPattern::LifeGained {
                    who: Ref::each_player(),
                },
                vec![gain_life(1)],
            ),
        ],
    )
}

#[rstest]
#[case::burn("Lightning Bolt")]
#[case::creature("Grizzly Bears")]
#[case::pump("Giant Growth")]
#[case::flyer("Serra Angel")]
#[case::mana_creature("Llanowar Elves")]
#[case::mana_rock("Sol Ring")]
#[case::removal("Swords to Plowshares")]
#[case::sweeper("Wrath of God")]
#[case::ramp("Rampant Growth")]
#[case::cantrip("Ponder")]
#[case::cascade("Bloodbraid Elf")]
#[case::protector("Mother of Runes")]
#[case::basic_land("Forest")]
fn verified_corpus_cards_pass_and_are_used(#[case] name: &str) -> Result<(), String> {
    let report = smoke(corpus_card(name)?.as_ref());
    assert!(report.passed(), "{name}: {:?}", report.failure);
    assert!(report.uses.total() > 0, "{name}: {:?}", report.uses);
    assert_eq!(report.games, 4, "two seeds at two table sizes");
    Ok(())
}

#[rstest]
#[case::spell(corpus_card("Lightning Bolt"), |u: Uses| u.cast > 0 && u.resolved > 0 && u.played == 0)]
#[case::land(corpus_card("Forest"), |u: Uses| u.played > 0 && u.cast == 0)]
#[case::mana_ability(corpus_card("Sol Ring"), |u: Uses| u.cast > 0 && u.activated > 0)]
#[case::enters_trigger(
    Ok(creature_with("Sage", vec![trigger(when::this_enters(), vec![gain_life(1)])])),
    |u: Uses| u.cast > 0 && u.triggered > 0 && u.resolved > u.cast
)]
fn the_counts_follow_what_the_card_does(
    #[case] card: Result<Arc<CardDefinition>, String>,
    #[case] expected: fn(Uses) -> bool,
) -> Result<(), String> {
    let report = smoke(card?.as_ref());
    assert!(report.passed(), "{:?}", report.failure);
    assert!(expected(report.uses), "{:?}", report.uses);
    Ok(())
}

#[test]
fn an_activated_ability_is_counted_when_the_bots_use_it() {
    let ability = Ability::Activated {
        cost: mtg_cards::cost::Cost(vec![mtg_cards::cost::CostItem::Tap]),
        timing: mtg_cards::ability::Timing::default(),
        condition: None,
        limit: None,
        from: mtg_cards::refs::ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![gain_life(1)],
    };
    let report = smoke(&artifact_with("Spring", vec![ability]));
    assert!(report.passed(), "{:?}", report.failure);
    assert!(report.uses.activated > 0 && report.uses.resolved > 0);
}

#[test]
fn a_rules_file_whose_effect_errors_fails_with_the_engine_error() {
    let report = smoke(&erroring_spell());
    assert!(
        matches!(&report.failure, Some(SmokeFailure::Engine { detail, .. }) if detail.contains("nothing")),
        "{:?}",
        report.failure
    );
    assert!(!report.passed());
    assert!(report.uses.cast == 0, "the failed game is not counted");
}

#[test]
fn the_gate_sees_the_error_of_a_card_that_already_runs_on_a_read() {
    let mut read = (*erroring_spell()).clone();
    read.read = true;
    let report = smoke(&read);
    assert!(
        matches!(&report.failure, Some(SmokeFailure::Engine { detail, .. }) if detail.contains("nothing")),
        "fail-soft resolution must not hide the error from the gate: {:?}",
        report.failure
    );
}

#[test]
fn a_card_that_loops_fails_as_a_stuck_game() {
    let report = smoke(&looping_creature());
    assert!(
        matches!(&report.failure, Some(SmokeFailure::Stuck { .. })),
        "{:?}",
        report.failure
    );
    assert_eq!(report.games, 1, "the first failure ends the run");
}

/// The game of the looping creature at two seats, with the bots that prefer it, played under `options`.
fn loop_game(options: PlayOptions) -> Result<GameOutcome, GameFailure> {
    let card = looping_creature();
    let bots = bots_for(2, 1, options)
        .into_iter()
        .map(|bot| bot.favouring(card.name.clone()))
        .collect();
    play_game_with_bots(smoke_setup(&card, 2, 1), 1, options, bots)
}

#[test]
fn the_watchdog_ends_a_runaway_game_as_a_failure_when_asked_to() {
    let options = PlayOptions {
        runaway_fails: true,
        ..PlayOptions::default()
    };
    let failure = loop_game(options).expect_err("a loop cannot end");
    let FailureKind::Runaway(report) = &failure.kind else {
        panic!("a runaway game expected: {}", failure.kind);
    };
    assert!(
        report.starts_with("watchdog: game of seed 1 runs away"),
        "{report}"
    );
    assert!(report.contains("Looping Creature"), "{report}");
}

#[test]
fn a_game_that_uses_up_its_time_is_a_runaway() {
    let options = PlayOptions {
        runaway_fails: true,
        time_limit: Some(Duration::ZERO),
        ..PlayOptions::default()
    };
    let card = creature("Bear", 1, &[Color::Green], 2, 2);
    let bots = bots_for(2, 1, options);
    let failure = play_game_with_bots(smoke_setup(&card, 2, 1), 1, options, bots)
        .expect_err("no time at all");
    let FailureKind::Runaway(report) = &failure.kind else {
        panic!("a runaway game expected: {}", failure.kind);
    };
    assert!(report.contains("s of play, 0.0 s allowed"), "{report}");
}

#[test]
#[should_panic(expected = "watchdog: game of seed 1 runs away")]
fn the_watchdog_panics_with_its_report_unless_asked_otherwise() {
    let _ = loop_game(PlayOptions::default());
}

#[rstest]
#[case::too_dear(creature("Too Dear", 30, &[], 3, 3))]
#[case::a_colour_no_land_makes(creature("Colourless Only", 0, &[], 3, 3).as_ref().clone().with_mana_cost(
    mtg_core::mana::ManaCost::from_symbols([mtg_core::mana::ManaSymbol::Colorless; 3])
).into())]
fn a_card_that_can_never_be_cast_is_never_seen(#[case] card: Arc<CardDefinition>) {
    let report = smoke(&card);
    assert_eq!(
        report.failure,
        Some(SmokeFailure::NeverSeen { games: 4 }),
        "{:?}",
        report.uses
    );
    assert_eq!(report.uses, Uses::default());
}

#[test]
fn a_report_repeats_for_the_same_seeds() -> Result<(), String> {
    let card = corpus_card("Serra Angel")?;
    assert_eq!(smoke(&card), smoke(&card));
    Ok(())
}
