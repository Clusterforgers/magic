//! Goal: the sentences of a hideaway card read into the free cast of the card exiled with the source with their conditions (a count of cards, a total, what a player controls, what happened this turn), in a trigger and in an activation, are written back as the same text in every wording of a number, and a condition or a wording the readers do not model is left to the table.
use super::*;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, StatKind, TurnTally};
use mtg_core::card::CardType;

fn exiled() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::ExiledWith(Ref::This),
    ]))
}

fn cast(play: bool) -> Effect {
    Effect::CastFromZone {
        who: Ref::You,
        what: exiled(),
        free: true,
        play,
        way: None,
        otherwise: Vec::new(),
    }
}

fn at_least(left: Amount, number: i32) -> Condition {
    Condition::Compare {
        left,
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    }
}

fn effects_of(text: &str, kind: ClauseKind) -> Vec<Effect> {
    match golden(text, kind, false) {
        Ability::Activated { effects, .. } | Ability::Triggered { effects, .. } => effects,
        other => panic!("{text}: {other:?}"),
    }
}

#[rstest]
#[case::play(
    "{T}: You may play the exiled card without paying its mana cost.",
    true
)]
#[case::cast(
    "{T}: You may cast the exiled card without paying its mana cost.",
    false
)]
fn the_exiled_card_is_played_or_cast_for_nothing(#[case] text: &str, #[case] play: bool) {
    assert_eq!(effects_of(text, ClauseKind::Activated), vec![cast(play)]);
}

#[rstest]
#[case::attacked(
    "{W}, {T}: You may play the exiled card without paying its mana cost if you attacked with three or more creatures this turn.",
    at_least(Amount::Tallied { who: Ref::You, what: TurnTally::Attackers }, 3)
)]
#[case::attacked_digits(
    "{W}, {T}: You may play the exiled card without paying its mana cost if you attacked with 4 or more creatures this turn.",
    at_least(Amount::Tallied { who: Ref::You, what: TurnTally::Attackers }, 4)
)]
#[case::damage(
    "{R}, {T}: You may play the exiled card without paying its mana cost if an opponent was dealt 7 or more damage this turn.",
    at_least(Amount::Tallied { who: Ref::Opponents, what: TurnTally::DamageTaken }, 7)
)]
#[case::damage_in_words(
    "{R}, {T}: You may play the exiled card without paying its mana cost if an opponent was dealt seven or more damage this turn.",
    at_least(Amount::Tallied { who: Ref::Opponents, what: TurnTally::DamageTaken }, 7)
)]
#[case::total_power(
    "{G}, {T}: You may play the exiled card without paying its mana cost if creatures you control have total power 10 or greater.",
    at_least(
        Amount::Total {
            of: Selector(vec![Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)]),
            stat: StatKind::Power,
        },
        10
    )
)]
#[case::hands(
    "{B}, {T}: You may play the exiled card without paying its mana cost if each player has no cards in hand.",
    Condition::Compare {
        left: Amount::Count(Selector(vec![Pred::In(ZoneKind::Hand)])),
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    }
)]
fn a_condition_after_the_cast_reads_as_an_if(#[case] text: &str, #[case] condition: Condition) {
    assert_eq!(
        effects_of(text, ClauseKind::Activated),
        vec![Effect::If {
            condition,
            then: vec![cast(true)],
            otherwise: Vec::new()
        }]
    );
}

#[rstest]
#[case::controls(
    "Then if you control a creature with power 7 or greater, you may play the exiled card without paying its mana cost."
)]
#[case::graveyard(
    "Then if there are twenty or more cards in your graveyard, you may play the exiled card without paying its mana cost."
)]
#[case::hand(
    "Then if you have nine or more cards in hand, you may play the exiled card without paying its mana cost."
)]
#[case::creatures(
    "Then if you control ten or more creatures, you may play the exiled card without paying its mana cost."
)]
fn a_condition_before_the_cast_after_another_sentence_reads(#[case] sentence: &str) {
    let text = format!("At the beginning of your upkeep, draw a card. {sentence}");
    let effects = effects_of(&text, ClauseKind::Triggered);
    assert!(
        matches!(&effects[..], [Effect::Draw { .. }, Effect::If { then, .. }] if *then == vec![cast(true)])
    );
}

#[test]
fn one_of_the_exiled_cards_is_chosen_first_when_there_are_several() {
    let text = "Whenever ~ deals combat damage to a player, if there are cards exiled with it, you may play one of them without paying its mana cost.";
    let Ability::Triggered {
        intervening_if,
        effects,
        ..
    } = golden(text, ClauseKind::Triggered, false)
    else {
        panic!("a trigger");
    };
    assert_eq!(
        intervening_if,
        Some(at_least(
            Amount::Count(Selector(vec![
                Pred::In(ZoneKind::Exile),
                Pred::ExiledWith(Ref::This)
            ])),
            1
        ))
    );
    assert!(
        matches!(&effects[..], [Effect::Choose { bind, .. }, Effect::CastFromZone { what: Ref::Bound(played), play: true, .. }] if bind == played)
    );
}

#[test]
fn the_card_returns_to_its_owners_hand_when_the_cast_was_made() {
    let text = "Whenever ~ deals combat damage to a player, you may cast the exiled card without paying its mana cost. If you do, return ~ to its owner's hand.";
    let effects = effects_of(text, ClauseKind::Triggered);
    assert_eq!(
        effects,
        vec![
            cast(false),
            Effect::If {
                condition: Condition::Performed,
                then: vec![Effect::Move {
                    what: Ref::This,
                    to: crate::effect::Destination::Hand
                }],
                otherwise: Vec::new()
            }
        ]
    );
}

#[rstest]
#[case::another_verb("{T}: You may exile the exiled card without paying its mana cost.")]
#[case::with_a_cost("{T}: You may play the exiled card.")]
#[case::three_words(
    "{W}, {T}: You may play the exiled card without paying its mana cost if you attacked with three creatures this turn."
)]
#[case::one_creature(
    "{W}, {T}: You may play the exiled card without paying its mana cost if you attacked with one or more creatures this turn."
)]
#[case::a_player(
    "{R}, {T}: You may play the exiled card without paying its mana cost if a player was dealt 7 or more damage this turn."
)]
#[case::less_power(
    "{G}, {T}: You may play the exiled card without paying its mana cost if creatures you control have total power 10 or less."
)]
#[case::a_library(
    "{U}, {T}: You may play the exiled card without paying its mana cost if a library has twenty or fewer cards in it."
)]
#[case::no_cast(
    "{T}: You may play the exiled card without paying its mana cost if you attacked with three or more creatures."
)]
fn a_wording_the_readers_do_not_model_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Activated, false), None, "{text}");
}

#[rstest]
#[case::a_state_opens_a_trigger(
    "Whenever ~ attacks, if an opponent controls more lands than you, draw a card."
)]
#[case::a_state_in_a_spell_is_the_tables_after_the_first_sentence(
    "Draw a card. If you control a creature, draw a card."
)]
fn a_state_of_the_game_is_a_condition_of_the_cast_or_after_then_only(#[case] text: &str) {
    let kind = if text.starts_with("Whenever") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[test]
fn returning_the_card_by_its_name_is_an_ability_and_not_a_spell() {
    assert_eq!(spell("Return ~ to its owner's hand."), None);
}
