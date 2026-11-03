//! Goal: the cost of a mode (Spree, Tiered) and the extra cost of each mode beyond the first (Escalate) join the total cost, only the modes the player can pay for are offered and no more than the cheapest of them pay for, the castable list agrees with the cast, an answer that cannot be paid is refused, and the cast pays the total of the modes chosen.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use rstest::rstest;

use crate::choose::{ModeChoice, ModeOptions};
use crate::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use crate::engine::Engine;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

fn mana(text: &str) -> Cost {
    Cost::mana(text.parse().unwrap())
}

fn discard() -> Cost {
    Cost(vec![CostItem::Discard {
        count: Amount::one(),
        filter: None,
    }])
}

/// What a modal instant that costs `{1}` has: the mode costs, the extra cost and how many modes it chooses.
struct Spell {
    costs: Vec<Cost>,
    extra: Option<Cost>,
    min: u32,
    max: Option<u32>,
}

impl Spell {
    /// Three modes that draw a card each, so none of them has a target.
    fn card(&self) -> Arc<CardDefinition> {
        let mode: serde_json::Value = serde_json::from_str(r#"{"effects":[{"Draw":{}}]}"#).unwrap();
        let mut modal = serde_json::json!({
            "choose": { "min": self.min, "max": self.max },
            "modes": vec![mode; 3],
        });
        if !self.costs.is_empty() {
            modal["mode_costs"] = serde_json::to_value(&self.costs).unwrap();
        }
        if let Some(extra) = &self.extra {
            modal["extra_mode_cost"] = serde_json::to_value(extra).unwrap();
        }
        let rules = serde_json::json!({ "Spell": { "effects": [{ "Modal": modal }] } });
        Arc::new(
            CardDefinition::new("Spree Spell")
                .with_type(CardType::Instant)
                .with_mana_cost(ManaCost::parse("{1}").unwrap())
                .with_rules(serde_json::from_value(rules).unwrap()),
        )
    }

    /// A game where seat 0 holds the spell and `others` more cards, with `lands` Forests in play.
    fn engine(&self, lands: usize, others: usize) -> (Engine, ObjectId) {
        let forest = Arc::new(CardDefinition::basic_land(Some(Color::Green)));
        let mut hand = vec![self.card()];
        hand.extend((0..others).map(|n| creature(&format!("Spare {n}"), 1, 1)));
        let engine =
            Engine::new(setup(vec![hand, vec![]], vec![vec![forest; lands], vec![]])).unwrap();
        let card = engine.state().zone(Zone::Hand(SEAT))[0];
        (engine, card)
    }
}

fn castable(engine: &Engine, card: ObjectId) -> bool {
    let Some(DecisionKind::Priority(options)) = engine.decision().map(|d| d.kind.clone()) else {
        panic!("seat 0 has priority");
    };
    options.castable.contains(&card)
}

/// The modes offered when the spell is cast, or `None` when the cast is taken back before it asks.
fn cast(engine: &mut Engine, card: ObjectId) -> Option<ModeOptions> {
    engine
        .apply(Answer::Priority(PriorityAction::CastSpell(card)))
        .unwrap();
    match engine.decision().map(|d| d.kind.clone()) {
        Some(DecisionKind::ChooseMode(options)) => Some(options),
        _ => None,
    }
}

/// The modes offered by index and cost words, and the most to choose.
type Offer = (Vec<(usize, Option<String>)>, u32);

/// `(index, cost words)` of the modes offered and the most that may be chosen.
fn shape(options: &ModeOptions) -> Offer {
    (
        options
            .modes
            .iter()
            .map(|mode| (mode.index, mode.cost.clone()))
            .collect(),
        options.max,
    )
}

fn named(index: usize, cost: Option<&str>) -> (usize, Option<String>) {
    (index, cost.map(str::to_owned))
}

// A spree: the base cost {1} and a cost for each mode, all three modes of choose one or more.
#[rstest]
#[case::one_forest_cannot_pay_the_cheapest(1, None)]
#[case::two_pay_only_the_first_mode(
    2,
    Some((vec![named(0, Some("{G}"))], 1))
)]
#[case::three_pay_any_one_mode_but_not_two(
    3,
    Some((vec![named(0, Some("{G}")), named(1, Some("{1}{G}")), named(2, Some("{2}"))], 1))
)]
#[case::four_pay_the_pair_of_the_cheapest(
    4,
    Some((vec![named(0, Some("{G}")), named(1, Some("{1}{G}")), named(2, Some("{2}"))], 2))
)]
#[case::six_pay_all_three(
    6,
    Some((vec![named(0, Some("{G}")), named(1, Some("{1}{G}")), named(2, Some("{2}"))], 3))
)]
fn a_spree_offers_the_modes_the_player_pays_for_and_the_castable_list_agrees(
    #[case] lands: usize,
    #[case] expected: Option<Offer>,
) {
    let spell = Spell {
        costs: vec![mana("{G}"), mana("{1}{G}"), mana("{2}")],
        extra: None,
        min: 1,
        max: None,
    };
    let (mut engine, card) = spell.engine(lands, 0);
    assert_eq!(castable(&engine, card), expected.is_some());
    if expected.is_some() {
        assert_eq!(cast(&mut engine, card).as_ref().map(shape), expected);
    }
}

// Escalate: three free modes and {1} for each beyond the first, or a discarded card.
#[rstest]
#[case::mana_one_forest_one_mode(Some(mana("{1}")), 1, 0, 1)]
#[case::mana_two_forests_two_modes(Some(mana("{1}")), 2, 0, 2)]
#[case::mana_five_forests_all_three(Some(mana("{1}")), 5, 0, 3)]
#[case::a_discard_needs_a_card_in_hand(Some(discard()), 9, 0, 1)]
#[case::a_discard_with_one_spare_card(Some(discard()), 9, 1, 2)]
#[case::a_discard_with_two_spare_cards(Some(discard()), 9, 2, 3)]
fn escalate_charges_each_mode_beyond_the_first(
    #[case] extra: Option<Cost>,
    #[case] lands: usize,
    #[case] spare: usize,
    #[case] most: u32,
) {
    let spell = Spell {
        costs: Vec::new(),
        extra,
        min: 1,
        max: None,
    };
    let (mut engine, card) = spell.engine(lands, spare);
    assert!(castable(&engine, card));
    let options = cast(&mut engine, card).expect("three modes to choose");
    assert_eq!((options.modes.len(), options.max), (3, most));
}

#[rstest]
#[case::the_first_tier(vec![0], "{1}", None)]
#[case::the_second_tier(vec![1], "{3}", None)]
#[case::a_tier_the_forests_do_not_pay(vec![2], "{5}", Some(IllegalAnswer::NotOffered("mode")))]
fn a_tier_is_paid_with_the_cost_of_the_mode_chosen(
    #[case] chosen: Vec<usize>,
    #[case] added: &str,
    #[case] refused: Option<IllegalAnswer>,
) {
    let spell = Spell {
        costs: vec![mana("{1}"), mana("{3}"), mana("{5}")],
        extra: None,
        min: 1,
        max: Some(1),
    };
    // Five Forests pay the {1} of the spell and the second tier but not the third.
    let (mut engine, card) = spell.engine(5, 0);
    let options = cast(&mut engine, card).expect("tiers to choose");
    assert_eq!(options.modes.len(), 2);
    let answered = engine.apply(Answer::ChooseMode(ModeChoice::Modes(chosen)));
    if let Some(why) = refused {
        assert_eq!(answered.unwrap_err(), EngineError::Illegal(why));
        return;
    }
    answered.unwrap();
    let DecisionKind::PayMana(pay) = engine.decision().unwrap().kind.clone() else {
        panic!("the payment comes after the mode");
    };
    let total = 1 + ManaCost::parse(added).unwrap().mana_value();
    assert_eq!(pay.cost.mana_value(), total);
    let events = engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
}

#[test]
fn an_answer_whose_modes_together_cost_too_much_is_refused_and_the_question_stays() {
    let spell = Spell {
        costs: vec![mana("{G}"), mana("{1}{G}"), mana("{2}")],
        extra: Some(mana("{2}")),
        min: 1,
        max: None,
    };
    // Four Forests pay {1} and the first mode ({G}), then the second ({1}{G} and the extra {2}) is one too many.
    let (mut engine, card) = spell.engine(4, 0);
    let options = cast(&mut engine, card).expect("modes to choose");
    assert_eq!(options.max, 1);
    let before = (engine.events().len(), engine.state_hash());
    let refused = engine
        .apply(Answer::ChooseMode(ModeChoice::Modes(vec![0, 1])))
        .unwrap_err();
    assert!(matches!(refused, EngineError::Illegal(_)));
    assert_eq!((engine.events().len(), engine.state_hash()), before);
    assert_eq!(engine.decision().unwrap().kind.name(), "choose-mode");
}

#[test]
fn a_cast_with_modes_that_cost_nothing_extra_asks_no_more_than_before() {
    let spell = Spell {
        costs: Vec::new(),
        extra: None,
        min: 1,
        max: None,
    };
    let (mut engine, card) = spell.engine(1, 0);
    let options = cast(&mut engine, card).expect("modes to choose");
    assert_eq!(
        shape(&options),
        (vec![named(0, None), named(1, None), named(2, None)], 3)
    );
}
