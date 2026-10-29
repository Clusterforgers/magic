//! Goal: a card a batch shows and then hides (a revealed hand that stays a hand, a looked-at card put on the bottom, a tutored card revealed and put into a hand) reaches every viewer it was shown to in `EventBatch.shown` as it was seen, at 2, 3 and 4 seats and for a spectator; a viewer it was not shown to finds no trace of it, and a revealed card that stays visible is in `objects`, not `shown`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::compile::compile;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_protocol::{EventBatch, project_events};
use mtg_testkit::cards::{cost, forest, free_creature, island, revealer};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A sorcery for `{G}` with "Reveal each creature on the battlefield."
fn field_revealer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Field Show")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[mtg_core::color::Color::Green]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::Reveal {
                    what: Ref::Each(Selector(vec![
                        Pred::In(ZoneKind::Battlefield),
                        Pred::Type(CardType::Creature),
                    ])),
                }],
            }),
    )
}

/// The tutor as the compiler reads its oracle text.
fn tutor() -> Arc<CardDefinition> {
    let text =
        "Search your library for a creature card, reveal it, put it into your hand, then shuffle.";
    let printed = PrintedCard {
        oracle_id: "oracle-tutor".to_owned(),
        name: "Tutor".to_owned(),
        faces: vec![PrintedFace {
            name: "Tutor".to_owned(),
            mana_cost: "{G}".to_owned(),
            type_line: "Sorcery".to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let rules = compile(&printed).card;
    Arc::new(mtg_cards::assemble(&printed, rules.as_ref(), &Caps::everything()).definition)
}

/// What seat 0 casts.
#[derive(Debug, Clone, Copy)]
enum Spell {
    /// Reveals the hand (`Hand Secret`) and puts the top card (`Top Secret`) on the bottom unseen by the others.
    RevealHandLookTop,
    /// Reveals the creature on the battlefield (`Field Secret`), which stays there.
    RevealField,
    /// Finds `Library Secret`, reveals it and puts it into the hand.
    Tutor,
}

/// Who should be shown each secret card, by seat 0 (the caster) and the other viewers.
const ALL: (bool, bool) = (false, true);
const CASTER: (bool, bool) = (true, false);
const NOBODY: (bool, bool) = (false, false);

/// Seat 0 casts the spell and it resolves; the events of the game are projected as one batch for every seat and a
/// spectator.
fn play(spell: Spell, seats: usize) -> Vec<EventBatch> {
    let card = match spell {
        Spell::RevealHandLookTop => revealer("Show"),
        Spell::RevealField => field_revealer(),
        Spell::Tutor => tutor(),
    };
    let name = card.name.clone();
    let mut field = vec![ready(forest()), ready(island())];
    field.push(ready(free_creature("Field Secret", 2, 2)));
    let mut scenario = Scenario::new(seats)
        .hand(0, [card, free_creature("Hand Secret", 1, 1)])
        .library(
            0,
            vec![
                free_creature("Top Secret", 4, 4),
                forest(),
                forest(),
                free_creature("Library Secret", 3, 3),
            ],
        )
        .battlefield(0, field);
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 4]);
    }
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(&name).unwrap();
    for _ in 0..100 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        let found = match &driver.decision().unwrap().kind {
            DecisionKind::ChooseCards(options) => options
                .from
                .iter()
                .copied()
                .find(|id| driver.state().object(*id).unwrap().card().name == "Library Secret"),
            _ => None,
        };
        match found {
            Some(id) => driver
                .answer(Answer::ChooseCards(CardsChoice(vec![id])))
                .unwrap(),
            None => driver.answer_passively().unwrap(),
        };
    }
    if matches!(spell, Spell::Tutor) {
        let hand = driver.state().zone(Zone::Hand(PlayerId(0)));
        let named = |id: &_| driver.state().object(*id).unwrap().card().name == "Library Secret";
        assert!(hand.iter().any(named), "the tutor found the only creature");
    }
    let viewers: Vec<PlayerId> = (0..=seats).map(PlayerId::from_index).collect();
    project_events(&before, driver.engine().events(), &viewers, 1).unwrap()
}

#[rstest]
#[case::revealed_hand(Spell::RevealHandLookTop, "Hand Secret", ALL)]
#[case::looked_at_top(Spell::RevealHandLookTop, "Top Secret", CASTER)]
#[case::unrevealed_library(Spell::RevealHandLookTop, "Library Secret", NOBODY)]
#[case::reveal_stays_visible(Spell::RevealField, "Field Secret", NOBODY)]
#[case::field_spell_hides_hand(Spell::RevealField, "Hand Secret", NOBODY)]
#[case::tutored(Spell::Tutor, "Library Secret", ALL)]
#[case::tutor_leaves_the_rest(Spell::Tutor, "Top Secret", NOBODY)]
fn a_card_shown_then_hidden_reaches_the_viewers_it_was_shown_to(
    #[case] spell: Spell,
    #[case] card: &str,
    #[case] shown: (bool, bool),
    #[values(2, 3, 4)] seats: usize,
) {
    let batches = play(spell, seats);
    for (index, batch) in batches.iter().enumerate() {
        let caster = index == 0;
        let wanted = if caster { shown.0 } else { shown.1 };
        let sighted = batch.shown.iter().find(|object| object.name == card);
        assert_eq!(sighted.is_some(), wanted, "viewer {index}: {card}");
        if let Some(object) = sighted {
            assert_eq!(object.owner, PlayerId(0));
            assert!(
                !batch.objects.iter().any(|o| o.id == object.id),
                "a sighting is never in objects"
            );
        }
        let json = serde_json::to_string(batch).unwrap();
        // The caster's own hand and the battlefield name their cards anyway; a hidden card is named only when shown.
        let visible = batch.objects.iter().any(|object| object.name == card);
        assert_eq!(
            json.contains(card),
            wanted || visible,
            "viewer {index}: {card} in the JSON"
        );
    }
}

#[rstest]
fn a_batch_that_shows_nothing_leaves_the_field_out(#[values(2, 4)] seats: usize) {
    for batch in play(Spell::RevealField, seats) {
        assert_eq!(batch.shown, Vec::new());
        let json = serde_json::to_value(&batch).unwrap();
        assert!(json.get("shown").is_none());
    }
}
