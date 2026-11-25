//! Goal: the engine names where a spell asks each decision, in the card's own terms: the modes as it is cast, the effect
//! its resolution stands at for a choice of cards from a library or a graveyard, a number and an option, an effect inside
//! the mode chosen (by its place on the card, whichever modes were chosen before it), and nothing for its payment.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::DecisionKind;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::resolve::{AskedPart, asked_at};
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Life, a creature tutor, or a draw: modes 0, 1 and 2.
const CHARM: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":1,"max":2},"modes":[
    {"effects":[{"GainLife":{"amount":3}}]},
    {"effects":[{"Search":{"filter":[{"Type":"Creature"}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]},
    {"effects":[{"Draw":{}}]}
]}}]}}"#;

/// A draw, then up to two counters on the Bear.
const GROW: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[
    {"Draw":{}},
    {"ChooseValue":{"kind":{"Number":[0,2]},"as":"up_to"}},
    {"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"ChosenAs":"up_to"}}}
]}}"#;

/// A draw, then a creature type chosen.
const NAME_TYPE: &str = r#"{"Spell":{"effects":[
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

/// Seat 0 holds a free instant with `rules`, a Bear on the battlefield and creatures in its library.
fn table(rules: &str) -> Driver {
    let rules: Ability = serde_json::from_str(rules).expect("rules");
    let spell = Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    );
    Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(free_creature("Bear", 2, 2))])
            .hand(0, [spell])
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
    .expect("table")
}

/// Passes priority until another decision waits.
fn to_question(driver: &mut Driver) {
    for _ in 0..12 {
        if !matches!(
            driver.decision().expect("decision").kind,
            DecisionKind::Priority(_)
        ) {
            return;
        }
        driver.pass().expect("pass");
    }
    panic!("no question came");
}

/// Where the spell asks the pending decision, with the decision's name.
fn asked(driver: &Driver) -> (&'static str, Option<AskedPart>) {
    let decision = driver.decision().expect("decision");
    // The spell on the stack, else the card being cast.
    let spell = match &decision.kind {
        DecisionKind::ChooseMode(options) => options.source,
        DecisionKind::PayMana(options) => options.spell,
        _ => *driver
            .state()
            .stack()
            .last()
            .expect("the spell on the stack"),
    };
    (
        decision.kind.name(),
        asked_at(driver.state(), spell, &decision.kind).map(|at| at.part),
    )
}

#[derive(Debug, Clone, Copy)]
enum Play {
    Pay,
    Modes,
    Mode(&'static [usize]),
    Number,
    Option,
    Graveyard,
}

#[rstest]
#[case::the_payment_is_asked_at_no_effect(Play::Pay, "pay-mana", None)]
#[case::the_modes_as_cast(Play::Modes, "choose-mode", Some(AskedPart::Modes(vec![0])))]
#[case::the_tutor_mode(Play::Mode(&[1]), "choose-cards", Some(AskedPart::Effect(vec![0, 1, 0])))]
#[case::the_tutor_after_another_mode(Play::Mode(&[0, 1]), "choose-cards", Some(AskedPart::Effect(vec![0, 1, 0])))]
#[case::a_number(Play::Number, "choose-number", Some(AskedPart::Effect(vec![1])))]
#[case::an_option(Play::Option, "choose-option", Some(AskedPart::Effect(vec![1])))]
#[case::a_card_from_the_graveyard(Play::Graveyard, "choose-cards", Some(AskedPart::Effect(vec![1])))]
fn a_decision_is_asked_at_its_place_in_the_effects(
    #[case] play: Play,
    #[case] kind: &str,
    #[case] expected: Option<AskedPart>,
) {
    let rules = match play {
        Play::Pay | Play::Modes | Play::Mode(_) => CHARM,
        Play::Number => GROW,
        Play::Option => NAME_TYPE,
        Play::Graveyard => DIG,
    };
    let mut driver = table(rules);
    match play {
        Play::Pay => {
            driver.begin_cast("Spell").expect("cast");
            driver
                .answer(Answer::ChooseMode(ModeChoice::Modes(vec![2])))
                .expect("mode");
        }
        Play::Modes => driver.begin_cast("Spell").expect("cast"),
        Play::Mode(modes) => {
            driver.begin_cast("Spell").expect("cast");
            driver
                .answer(Answer::ChooseMode(ModeChoice::Modes(modes.to_vec())))
                .expect("modes");
            driver
                .answer(Answer::PayMana(PayChoice::Auto))
                .expect("pay");
            to_question(&mut driver);
        }
        Play::Number => {
            let bear = driver.target_named("Bear").expect("bear");
            driver.cast_at("Spell", &[bear]).expect("cast");
            to_question(&mut driver);
        }
        Play::Option | Play::Graveyard => {
            driver.cast("Spell").expect("cast");
            to_question(&mut driver);
        }
    }
    assert_eq!(asked(&driver), (kind, expected));
}
