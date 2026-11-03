//! Goal: bargain and conspire are offered as optional additional costs only when the player controls what they ask for (an artifact, enchantment or token; two untapped creatures that share a color with the spell), the cast pays them by picking those permanents, and conspire puts a copy of the spell on the stack with its cast trigger.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use rstest::rstest;

use crate::choose::{CardsChoice, CostsChoice};
use crate::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use crate::engine::Engine;
use crate::events::Event;
use crate::fixtures::setup;
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

/// A red instant that costs `{R}` and draws a card, with `keyword`.
fn spell(keyword: Keyword) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Spell")
        .with_type(CardType::Instant)
        .with_mana_cost(ManaCost::parse("{R}").unwrap())
        .with_rules(serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap())
        .with_rules(Ability::Keyword(keyword));
    Arc::new(card)
}

fn mountain() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(Color::Red)))
}

/// A creature of `color`, or colorless.
fn bear(color: Option<Color>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Bear")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    match color {
        Some(Color::Red) => card = card.with_mana_cost(ManaCost::parse("{R}").unwrap()),
        Some(_) => card = card.with_mana_cost(ManaCost::parse("{G}").unwrap()),
        None => {}
    }
    Arc::new(card)
}

fn artifact() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Trinket").with_type(CardType::Artifact))
}

/// Seat 0 holds the spell and controls two Mountains and `others`.
fn engine(keyword: Keyword, others: Vec<Arc<CardDefinition>>) -> (Engine, ObjectId) {
    let mut field = vec![mountain(), mountain()];
    field.extend(others);
    let engine = Engine::new(setup(vec![vec![spell(keyword)], vec![]], vec![field, vec![]])).unwrap();
    let card = engine.state().zone(Zone::Hand(SEAT))[0];
    (engine, card)
}

/// Casts the spell and answers every question until the payment: the optional cost is paid when `pay`, and the first
/// permanents offered are picked for it. Returns the costs offered and the events of the payment.
fn cast(engine: &mut Engine, card: ObjectId, pay: bool) -> (Vec<usize>, Vec<Event>) {
    engine
        .apply(Answer::Priority(PriorityAction::CastSpell(card)))
        .unwrap();
    let mut offered = Vec::new();
    for _ in 0..8 {
        let Some(decision) = engine.decision() else {
            break;
        };
        match decision.kind.clone() {
            DecisionKind::ChooseCosts(options) => {
                offered = options.optional.iter().map(|cost| cost.index).collect();
                let costs = if pay { offered.clone() } else { Vec::new() };
                engine
                    .apply(Answer::ChooseCosts(CostsChoice::Costs(costs)))
                    .unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                let picked = options.from[..options.max as usize].to_vec();
                engine
                    .apply(Answer::ChooseCards(CardsChoice(picked)))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                let events = engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
                return (offered, events);
            }
            other => panic!("{} was not expected", other.name()),
        }
    }
    (offered, Vec::new())
}

#[rstest]
#[case::nothing_to_sacrifice(Keyword::Bargain, vec![], false)]
#[case::an_artifact(Keyword::Bargain, vec![artifact()], true)]
#[case::only_a_creature_that_is_none_of_them(Keyword::Bargain, vec![bear(None)], false)]
#[case::one_creature_of_the_color(Keyword::Conspire, vec![bear(Some(Color::Red))], false)]
#[case::two_creatures_of_the_color(Keyword::Conspire, vec![bear(Some(Color::Red)), bear(Some(Color::Red))], true)]
#[case::two_creatures_of_another_color(Keyword::Conspire, vec![bear(Some(Color::Green)), bear(Some(Color::Green))], false)]
#[case::two_colorless_creatures(Keyword::Conspire, vec![bear(None), bear(None)], false)]
fn the_cost_is_offered_only_when_the_player_has_what_it_asks_for(
    #[case] keyword: Keyword,
    #[case] others: Vec<Arc<CardDefinition>>,
    #[case] offered: bool,
) {
    let (mut engine, card) = engine(keyword, others);
    let (costs, _) = cast(&mut engine, card, false);
    assert_eq!(costs, if offered { vec![0] } else { Vec::new() });
}

#[test]
fn a_bargained_cast_sacrifices_the_permanent_it_picks() {
    let (mut engine, card) = engine(Keyword::Bargain, vec![artifact()]);
    let (costs, events) = cast(&mut engine, card, true);
    assert_eq!(costs, vec![0]);
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    assert_eq!(engine.state().zone(Zone::Graveyard(SEAT)).len(), 1, "the artifact was sacrificed");
}

#[test]
fn a_conspired_cast_taps_two_creatures_and_triggers_a_copy() {
    let (mut engine, card) = engine(
        Keyword::Conspire,
        vec![bear(Some(Color::Red)), bear(Some(Color::Red))],
    );
    let (_, events) = cast(&mut engine, card, true);
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    let tapped = events
        .iter()
        .filter(|e| matches!(e, Event::PermanentTapped { .. }))
        .count();
    // Two Mountains pay {R}: one of them taps for it; the two creatures are the conspire cost.
    assert!(tapped >= 3, "{tapped} permanents tapped");
    assert_eq!(
        engine.state().stack().len(),
        2,
        "the spell and its conspire trigger"
    );
}
