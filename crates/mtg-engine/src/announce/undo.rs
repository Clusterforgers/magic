//! Goal: `Answer::Undo` backs out of a spell or an activated ability at every question of its announcement (the way to cast it, modes, optional costs, X, targets, cards for a cost, the payment), leaving the log, the state, the hand and every untapped mana source as they were and giving the player priority again, while it is refused where nothing is being announced.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::{Needs, Optional, Pending, What, XCost};
use crate::abilities::AbilityRef;
use crate::choose::{
    CardsChoice, CostsChoice, ModeChoice, NumberChoice, OptionChoice, TargetChoice,
};
use crate::decision::{Answer, DecisionKind, PayChoice};
use crate::engine::Engine;
use crate::error::{EngineError, IllegalAnswer};
use crate::fixtures::{creature, setup};
use crate::ways::Way;
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

fn forest() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(Color::Green)))
}

fn instant(name: &str, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

fn gadget(rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

const DRAW: &str = r#"{"Spell":{"effects":[{"Draw":{}}]}}"#;
const SHOCK: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}}"#;
const CHARM: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":1,"max":1},"modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]},{"effects":[{"GainLife":{"amount":3}}]}]}}]}}"#;
const PINGER: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#;
const ALTAR: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}}]}}"#;

/// The spells and abilities announced, each begun with seat 0 holding four Forests and two Bears and seat 1 a Bear.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// A spell with a kicker and an X.
    KickedX,
    /// A spell with two ways to cast it.
    TwoWays,
    /// A spell with a target.
    Targeted,
    /// A modal spell whose first mode targets.
    Modal,
    /// An ability that costs mana and a tap and targets.
    Pinger,
    /// An ability that costs mana and a sacrifice.
    Sacrifice,
}

fn begun(kind: Kind) -> Engine {
    let (hand, permanent) = match kind {
        Kind::KickedX | Kind::TwoWays => (Some(instant("Spell", DRAW)), None),
        Kind::Targeted => (Some(instant("Shock", SHOCK)), None),
        Kind::Modal => (Some(instant("Charm", CHARM)), None),
        Kind::Pinger => (None, Some(gadget(PINGER))),
        Kind::Sacrifice => (None, Some(gadget(ALTAR))),
    };
    let mut own = vec![forest(); 4];
    own.extend([creature("Bear", 2, 2), creature("Bear", 2, 2)]);
    own.extend(permanent.clone());
    let mut engine = Engine::new(setup(
        vec![hand.into_iter().collect(), vec![]],
        vec![own, vec![creature("Bear", 2, 2)]],
    ))
    .unwrap();
    let what = if permanent.is_some() {
        let source = *engine
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .rev()
            .nth(1)
            .unwrap();
        What::Ability(AbilityRef::printed(source, 0))
    } else {
        What::Spell(engine.state().zone(Zone::Hand(SEAT))[0])
    };
    let pending = match kind {
        Kind::KickedX => Pending::needing(
            SEAT,
            what,
            Needs {
                optional: vec![Optional::mana_only("Kicker {1}", "{1}", false)],
                x: Some(XCost::Mana { symbols: 1 }),
                ..Needs::default()
            },
        ),
        Kind::TwoWays => Pending::needing(
            SEAT,
            what,
            Needs {
                ways: [
                    (None, "Mana cost {1}", "{1}"),
                    (Some(CastWay::Overload), "Overload {2}", "{2}"),
                ]
                .into_iter()
                .map(|(alternative, label, mana)| Way {
                    alternative,
                    label: label.to_owned(),
                    mana: Some(ManaCost::parse(mana).unwrap()),
                    instant: true,
                    variant: 0,
                    parts: Vec::new(),
                })
                .collect(),
                ..Needs::default()
            },
        ),
        _ => Pending::new(SEAT, what),
    };
    engine.begin(pending).unwrap();
    engine
}

fn asked(engine: &Engine) -> &'static str {
    engine.decision().unwrap().kind.name()
}

/// The least answer to the question asked, which goes on to the next one.
fn least(kind: &DecisionKind) -> Answer {
    match kind {
        DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(Vec::new())),
        DecisionKind::ChooseNumber(options) => {
            Answer::ChooseNumber(NumberChoice::Number(options.min))
        }
        DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
        DecisionKind::ChooseMode(options) => {
            Answer::ChooseMode(ModeChoice::Modes(vec![options.modes[0].index]))
        }
        DecisionKind::ChooseTargets(options) => {
            let count = usize::try_from(options.min.max(1)).unwrap();
            Answer::ChooseTargets(TargetChoice::Targets(options.legal[..count].to_vec()))
        }
        DecisionKind::ChooseCards(options) => {
            let count = usize::try_from(options.min.max(1)).unwrap();
            Answer::ChooseCards(CardsChoice(options.from[..count].to_vec()))
        }
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        other => panic!("{} is not a question of the announcement", other.name()),
    }
}

#[rstest]
#[case::kicker_and_x(Kind::KickedX, &["choose-costs", "choose-number", "pay-mana"])]
#[case::two_ways(Kind::TwoWays, &["choose-option", "pay-mana"])]
#[case::a_target(Kind::Targeted, &["choose-targets", "pay-mana"])]
#[case::modes_then_a_target(Kind::Modal, &["choose-mode", "choose-targets", "pay-mana"])]
#[case::an_ability_with_a_target(Kind::Pinger, &["choose-targets", "pay-mana"])]
#[case::an_ability_with_a_sacrifice(Kind::Sacrifice, &["choose-cards", "pay-mana"])]
fn every_question_of_the_announcement_offers_undo_and_the_last_puts_it_on_the_stack(
    #[case] kind: Kind,
    #[case] steps: &[&str],
) {
    let mut engine = begun(kind);
    for step in steps {
        assert_eq!(asked(&engine), *step, "{kind:?}");
        let decision = &engine.decision().unwrap().kind;
        assert!(decision.can_undo(), "{kind:?} at {step}");
        let answer = least(decision);
        engine.apply(answer).unwrap();
    }
    assert_eq!(engine.state().stack().len(), 1, "{kind:?}");
}

#[rstest]
#[case::the_kicker(Kind::KickedX, 0)]
#[case::x(Kind::KickedX, 1)]
#[case::the_payment_of_a_kicked_spell(Kind::KickedX, 2)]
#[case::the_way_to_cast(Kind::TwoWays, 0)]
#[case::the_payment_of_a_way(Kind::TwoWays, 1)]
#[case::the_target_of_a_spell(Kind::Targeted, 0)]
#[case::the_payment_of_a_targeted_spell(Kind::Targeted, 1)]
#[case::the_modes(Kind::Modal, 0)]
#[case::the_target_of_a_mode(Kind::Modal, 1)]
#[case::the_payment_of_a_modal_spell(Kind::Modal, 2)]
#[case::the_target_of_an_ability(Kind::Pinger, 0)]
#[case::the_payment_of_a_tap_ability(Kind::Pinger, 1)]
#[case::the_sacrifice(Kind::Sacrifice, 0)]
#[case::the_payment_after_the_sacrifice(Kind::Sacrifice, 1)]
fn undo_at_a_step_leaves_the_game_as_it_was(#[case] kind: Kind, #[case] step: usize) {
    let mut engine = begun(kind);
    let before = (engine.events().len(), engine.state_hash());
    let hand = engine.state().zone(Zone::Hand(SEAT)).to_vec();
    for _ in 0..step {
        let answer = least(&engine.decision().unwrap().kind);
        engine.apply(answer).unwrap();
    }
    let events = engine.apply(Answer::Undo).unwrap();
    assert!(
        events.is_empty(),
        "{kind:?} at {step}: nothing is logged, so nothing hidden is shown"
    );
    assert_eq!(
        (engine.events().len(), engine.state_hash()),
        before,
        "{kind:?} at {step}"
    );
    assert_eq!(
        engine.state().zone(Zone::Hand(SEAT)),
        hand,
        "the card is back in the hand"
    );
    assert_eq!(asked(&engine), "priority", "the player has priority again");
}

#[test]
fn undo_is_refused_where_nothing_is_announced() {
    let mut engine = begun(Kind::Targeted);
    engine.apply(Answer::Undo).unwrap();
    let before = engine.state_hash();
    assert_eq!(
        engine.apply(Answer::Undo).unwrap_err(),
        EngineError::Illegal(IllegalAnswer::NotOffered("undo"))
    );
    assert_eq!(engine.state_hash(), before);
}
