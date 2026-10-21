//! Goal: the engine asks the optional costs and X of a cast one after the other, refuses an answer outside the options and asks again, keeps the answers in the `SpellCast` it logs and pays the total they make, and a cast backed out of at either question leaves the log and the state as they were.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::{Needs, Optional, Pending, What, XCost};
use crate::choose::{CostsChoice, NumberChoice};
use crate::decision::{Answer, DecisionKind, PayChoice};
use crate::engine::Engine;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

fn spell() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap()),
    )
}

/// Seat 0 holds the spell with four Forests; its cast has been begun with a kicker of `{1}` and one `{X}`.
fn begun() -> Engine {
    let forest = Arc::new(CardDefinition::basic_land(Some(Color::Green)));
    let mut engine = Engine::new(setup(
        vec![vec![spell()], vec![]],
        vec![vec![forest; 4], vec![creature("Bear", 2, 2)]],
    ))
    .unwrap();
    let card = engine.state().zone(Zone::Hand(SEAT))[0];
    let needs = Needs {
        ways: Vec::new(),
        optional: vec![Optional {
            label: "Kicker {1}".to_owned(),
            mana: ManaCost::parse("{1}").unwrap(),
            repeatable: false,
        }],
        x: Some(XCost::Mana { symbols: 1 }),
    };
    engine
        .begin(Pending::needing(SEAT, What::Spell(card), needs))
        .unwrap();
    engine
}

fn asked(engine: &Engine) -> &'static str {
    engine.decision().unwrap().kind.name()
}

#[test]
fn the_cast_asks_the_optional_costs_then_x_then_the_payment_and_logs_what_was_chosen() {
    let mut engine = begun();
    let logged = engine.events().len();
    assert_eq!(asked(&engine), "choose-costs");
    engine
        .apply(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))
        .unwrap();
    let DecisionKind::ChooseNumber(options) = &engine.decision().unwrap().kind else {
        panic!("X comes after the costs");
    };
    // Four Forests pay for {1}, the kicker and two more.
    assert_eq!(
        (options.min, options.max, options.can_cancel),
        (0, Some(2), true)
    );
    assert_eq!(options.text, "Choose a value for X.");
    assert_eq!(
        engine.events().len(),
        logged,
        "nothing is logged until it is paid"
    );
    engine
        .apply(Answer::ChooseNumber(NumberChoice::Number(2)))
        .unwrap();
    let DecisionKind::PayMana(pay) = &engine.decision().unwrap().kind else {
        panic!("the payment comes last");
    };
    assert_eq!(pay.cost, ManaCost::parse("{4}").unwrap());
    let events = engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
    let Some(Event::SpellCast { choices, .. }) =
        events.iter().find(|e| matches!(e, Event::SpellCast { .. }))
    else {
        panic!("the spell was cast");
    };
    assert_eq!(
        (choices.x, choices.optional_costs.clone()),
        (Some(2), vec![0])
    );
    assert_eq!(engine.state().stack().len(), 1);
}

#[rstest]
#[case::a_number_above_the_most(NumberChoice::Number(4))]
#[case::a_number_far_above_it(NumberChoice::Number(u32::MAX))]
fn an_answer_outside_the_options_is_refused_and_the_question_stays(#[case] number: NumberChoice) {
    let mut engine = begun();
    engine
        .apply(Answer::ChooseCosts(CostsChoice::Costs(Vec::new())))
        .unwrap();
    let before = (engine.events().len(), engine.state_hash());
    let refused = engine.apply(Answer::ChooseNumber(number)).unwrap_err();
    assert_eq!(
        refused,
        EngineError::Illegal(IllegalAnswer::NotOffered("number"))
    );
    assert_eq!((engine.events().len(), engine.state_hash()), before);
    assert_eq!(asked(&engine), "choose-number");
}

#[rstest]
#[case::at_the_costs(0)]
#[case::at_x(1)]
fn backing_out_leaves_the_log_and_the_state_as_they_were(#[case] after: usize) {
    let mut engine = begun();
    let before = (engine.events().len(), engine.state_hash());
    if after == 1 {
        engine
            .apply(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))
            .unwrap();
    }
    let events = if after == 0 {
        engine.apply(Answer::ChooseCosts(CostsChoice::Cancel))
    } else {
        engine.apply(Answer::ChooseNumber(NumberChoice::Cancel))
    }
    .unwrap();
    assert_eq!(events, Vec::<Event>::new());
    assert_eq!((engine.events().len(), engine.state_hash()), before);
    assert_eq!(asked(&engine), "priority", "the caster has priority again");
}
