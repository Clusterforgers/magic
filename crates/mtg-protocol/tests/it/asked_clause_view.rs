//! Goal: a decision of a spell or ability carries the spell's own words and marks the clause the engine asks at, taken
//! from its place in the effects: the modes as they are chosen, the mode that runs, a number, an option and a choice of
//! cards from a library or a graveyard, each in a spell with other clauses around it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::DecisionKind;
use mtg_engine::choose::{ModeChoice, Target};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_protocol::AskingView;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// "You gain 3 life." or a creature tutor, the one chosen.
const CHARM: &str = r#"{"Spell":{"effects":[{"Modal":{"modes":[
    {"effects":[{"GainLife":{"amount":3}}]},
    {"effects":[{"Search":{"filter":[{"Type":"Creature"}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}
]}}]}}"#;

/// A draw, then up to two counters on the target.
const GROW: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[
    {"Draw":{}},
    {"ChooseValue":{"kind":{"Number":[0,2]},"as":"up_to"}},
    {"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"ChosenAs":"up_to"}}}
]}}"#;

/// A draw, then each creature of the caster becomes the creature type chosen.
const PAINT: &str = r#"{"Spell":{"effects":[
    {"Draw":{}},
    {"ChooseValue":{"kind":"CreatureType","as":"chosen"}},
    {"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"SetCreatureTypesChosen":"chosen"}],"duration":"UntilEndOfTurn"}}
]}}"#;

/// A mill, then a creature card returned from the graveyard.
const DIG: &str = r#"{"Spell":{"effects":[
    {"Mill":{"count":3}},
    {"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]},"as":"returned"}},
    {"Move":{"what":{"Bound":"returned"},"to":"Hand"}}
]}}"#;

/// A free instant named `name` with `rules`.
fn sorcery(name: &str, rules: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// Seat 0 holds the instant, a Bear on the battlefield and creatures in its library.
fn table(rules: &str) -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(free_creature("Bear", 2, 2))])
            .hand(0, [sorcery("Spell", rules)])
            .library(
                0,
                [
                    free_creature("Elk", 3, 3),
                    forest(),
                    free_creature("Owl", 1, 1),
                    forest(),
                ],
            ),
    )
    .unwrap()
}

/// What asks the pending decision, as its seat sees it.
fn asking(driver: &Driver) -> AskingView {
    let decision = driver.decision().unwrap();
    AskingView::of(driver.state(), decision.player, decision).unwrap()
}

/// The words of the text the clause marks.
fn marked(asking: &AskingView) -> Option<String> {
    let (start, end) = asking.clause?;
    let units: Vec<u16> = asking.text.as_deref()?.encode_utf16().collect();
    String::from_utf16(&units[start as usize..end as usize]).ok()
}

/// Passes priority until a decision that is not priority waits, and names its kind.
fn to_question(driver: &mut Driver) -> &'static str {
    for _ in 0..12 {
        let kind = &driver.decision().unwrap().kind;
        if !matches!(kind, DecisionKind::Priority(_)) {
            return kind.name();
        }
        driver.pass().unwrap();
    }
    panic!("no question came");
}

#[derive(Debug, Clone, Copy)]
enum Play {
    /// The charm as its modes are chosen.
    Modes,
    /// The charm's tutor mode as it resolves.
    Tutor,
    /// The number of counters.
    Number,
    /// The creature type.
    Option,
    /// The card returned after the mill.
    Graveyard,
}

#[rstest]
#[case::the_modes_as_cast(
    Play::Modes,
    "choose-mode",
    "Choose one —\n• You gain 3 life.\n• Search your library for a creature card, reveal it, put it into your hand, then shuffle."
)]
#[case::the_mode_that_runs(
    Play::Tutor,
    "choose-cards",
    "Search your library for a creature card, reveal it, put it into your hand, then shuffle."
)]
#[case::a_number(
    Play::Number,
    "choose-number",
    "Put up to two +1/+1 counters on target creature."
)]
#[case::an_option(
    Play::Option,
    "choose-option",
    "Each creature you control becomes the creature type of your choice until end of turn."
)]
#[case::a_card_from_the_graveyard(
    Play::Graveyard,
    "choose-cards",
    "return a creature card from your graveyard to your hand."
)]
fn the_clause_asked_is_the_place_the_engine_asks_at(
    #[case] play: Play,
    #[case] kind: &str,
    #[case] clause: &str,
) {
    let rules = match play {
        Play::Modes | Play::Tutor => CHARM,
        Play::Number => GROW,
        Play::Option => PAINT,
        Play::Graveyard => DIG,
    };
    let mut driver = table(rules);
    let bear = driver.target_named("Bear").unwrap();
    match play {
        Play::Modes => driver.begin_cast("Spell").unwrap(),
        Play::Tutor => {
            driver.begin_cast("Spell").unwrap();
            driver
                .answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))
                .unwrap();
            driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
        }
        Play::Number => {
            driver.cast_at("Spell", &[bear]).unwrap();
        }
        Play::Option | Play::Graveyard => {
            driver.cast("Spell").unwrap();
        }
    }
    assert_eq!(to_question(&mut driver), kind);
    let asking = asking(&driver);
    assert_eq!(asking.name, "Spell");
    assert!(asking.text.is_some(), "a spell carries its own words");
    assert_eq!(
        marked(&asking).as_deref(),
        Some(clause),
        "{:?}",
        asking.text
    );
}

#[test]
fn the_words_around_the_clause_are_the_spells_own() {
    let mut driver = table(GROW);
    let bear: Target = driver.target_named("Bear").unwrap();
    driver.cast_at("Spell", &[bear]).unwrap();
    to_question(&mut driver);
    assert_eq!(
        asking(&driver).text.as_deref(),
        Some("Draw a card. Put up to two +1/+1 counters on target creature.")
    );
}
