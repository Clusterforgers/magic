//! Goal: a spend restriction reads where a spell is cast from (exile, anywhere but its caster's hand), whether it is
//! colorless and whether it has devoid or flashback, from the spell as it is being cast.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::spend::{SpendFact, SpendFilter, SpendOn};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use crate::events::{Event, ZoneCause};
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::spend::Spend;
use crate::state::GameState;
use crate::zones::Zone;

/// An instant of the colours and keywords.
fn instant(colors: &[Color], keywords: Vec<Keyword>) -> Arc<CardDefinition> {
    let cost = if colors.is_empty() { "{2}" } else { "{1}{R}" };
    let mut card = CardDefinition::new("Spell")
        .with_type(CardType::Instant)
        .with_mana_cost(ManaCost::parse(cost).unwrap());
    card.color_indicator = Some(colors.iter().copied().collect());
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword));
    }
    Arc::new(card)
}

/// Seat 0 with the spell in hand, moved to `zone` first unless that is the hand.
fn spell_in(card: Arc<CardDefinition>, zone: Option<Zone>) -> (GameState, Spend) {
    let mut game = state(vec![vec![card], Vec::new()], vec![Vec::new(), Vec::new()]);
    let id = game
        .objects()
        .find(|object| object.card().name == "Spell")
        .map(crate::object::GameObject::id)
        .unwrap();
    if let Some(to) = zone {
        let moved = Event::ZoneChanged {
            object: id,
            from: Zone::Hand(PlayerId(0)),
            to,
            cause: ZoneCause::Moved,
        };
        assert!(apply_event(&mut game, &moved).is_ok());
    }
    let id = game
        .objects()
        .find(|object| object.card().name == "Spell")
        .map(crate::object::GameObject::id)
        .unwrap();
    let spend = Spend::for_spell(&game, id, PlayerId(0), false);
    (game, spend)
}

fn only(fact: SpendFact) -> [SpendOn; 1] {
    [SpendOn::Spell(SpendFilter {
        facts: vec![fact],
        ..SpendFilter::default()
    })]
}

#[rstest]
#[case::exile_from_exile(SpendFact::FromExile, Some(Zone::Exile), true)]
#[case::exile_not_from_the_hand(SpendFact::FromExile, None, false)]
#[case::exile_not_from_a_graveyard(SpendFact::FromExile, Some(Zone::Graveyard(PlayerId(0))), false)]
#[case::not_the_hand_from_a_graveyard(
    SpendFact::NotFromHand,
    Some(Zone::Graveyard(PlayerId(0))),
    true
)]
#[case::not_the_hand_from_exile(SpendFact::NotFromHand, Some(Zone::Exile), true)]
#[case::not_the_hand_from_the_library(
    SpendFact::NotFromHand,
    Some(Zone::Library(PlayerId(0))),
    true
)]
#[case::not_the_hand_from_the_hand(SpendFact::NotFromHand, None, false)]
fn where_a_spell_is_cast_from_is_read_while_it_is_still_there(
    #[case] fact: SpendFact,
    #[case] zone: Option<Zone>,
    #[case] allowed: bool,
) {
    let (_, spend) = spell_in(instant(&[Color::Red], Vec::new()), zone);
    assert_eq!(spend.allows(&only(fact)), allowed);
}

#[rstest]
#[case::colorless_spell(SpendFact::Colorless, &[], Vec::new(), true)]
#[case::red_is_not_colorless(SpendFact::Colorless, &[Color::Red], Vec::new(), false)]
#[case::devoid_is_colorless(SpendFact::Colorless, &[Color::Red], vec![Keyword::Devoid], true)]
#[case::devoid(SpendFact::Devoid, &[Color::Red], vec![Keyword::Devoid], true)]
#[case::colorless_without_devoid(SpendFact::Devoid, &[], Vec::new(), false)]
#[case::flashback(SpendFact::Flashback, &[Color::Red], vec![Keyword::Flashback(Cost::mana("{R}".parse().unwrap()))], true)]
#[case::no_flashback(SpendFact::Flashback, &[Color::Red], Vec::new(), false)]
fn what_a_spell_is_is_read_from_its_characteristics(
    #[case] fact: SpendFact,
    #[case] colors: &[Color],
    #[case] keywords: Vec<Keyword>,
    #[case] allowed: bool,
) {
    let (_, spend) = spell_in(instant(colors, keywords), None);
    assert_eq!(spend.allows(&only(fact)), allowed);
}

#[rstest]
#[case::from_the_hand(None, false, false)]
#[case::from_exile(Some(Zone::Exile), true, true)]
#[case::from_a_graveyard(Some(Zone::Graveyard(PlayerId(0))), true, false)]
fn a_spell_on_the_stack_remembers_where_it_was_cast_from(
    #[case] zone: Option<Zone>,
    #[case] not_from_hand: bool,
    #[case] from_exile: bool,
) {
    let (mut game, _) = spell_in(instant(&[Color::Red], Vec::new()), zone);
    let id = game
        .objects()
        .find(|object| object.card().name == "Spell")
        .map(crate::object::GameObject::id)
        .unwrap();
    let cast = Event::SpellCast {
        player: PlayerId(0),
        object: id,
        choices: crate::choose::Choices::default(),
    };
    assert!(apply_event(&mut game, &cast).is_ok());
    let on_stack = game
        .objects()
        .find(|object| object.card().name == "Spell" && object.zone() == Zone::Stack)
        .map(crate::object::GameObject::id);
    assert!(on_stack.is_some(), "the spell is on the stack");
    let spend = Spend::for_spell(&game, on_stack.unwrap(), PlayerId(0), false);
    assert_eq!(spend.allows(&only(SpendFact::NotFromHand)), not_from_hand);
    assert_eq!(spend.allows(&only(SpendFact::FromExile)), from_exile);
}
