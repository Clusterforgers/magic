//! Goal: "sacrifice it unless {M} was spent to cast it" reads as a sacrifice of the card that happens when that mana was not spent, "if {M} was spent to cast it" reads as an intervening "if" on the mana spent, both write back as the same text for every kind of mana, and a wording that differs (a cost, two symbols, another object, a spell that is not the card) stays with the table.
use super::*;

use crate::effect::Effect;
use crate::value::Condition;
use mtg_core::mana::ManaType;

#[rstest]
#[case::white('W', ManaType::White)]
#[case::blue('U', ManaType::Blue)]
#[case::black('B', ManaType::Black)]
#[case::red('R', ManaType::Red)]
#[case::green('G', ManaType::Green)]
#[case::colorless('C', ManaType::Colorless)]
fn the_card_is_sacrificed_unless_that_mana_was_spent(#[case] symbol: char, #[case] mana: ManaType) {
    let text = format!("When ~ enters, sacrifice it unless {{{symbol}}} was spent to cast it.");
    let read = golden(&text, ClauseKind::Triggered, false);
    let [
        Effect::If {
            condition,
            then,
            otherwise,
        },
    ] = read.effects()
    else {
        panic!("not a condition: {read:?}");
    };
    assert_eq!(
        *condition,
        Condition::Not(Box::new(Condition::ManaSpent(mana)))
    );
    assert!(matches!(then.as_slice(), [Effect::Sacrifice { .. }]));
    assert!(otherwise.is_empty());
}

#[rstest]
#[case::black("When ~ enters, if {B} was spent to cast it, target player discards a card.")]
#[case::red("When ~ enters, if {R} was spent to cast it, ~ gains haste until end of turn.")]
#[case::blue(
    "When ~ enters, if {U} was spent to cast it, return target creature to its owner's hand."
)]
fn the_mana_spent_is_an_intervening_if(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    assert!(
        matches!(
            read,
            Ability::Triggered {
                intervening_if: Some(Condition::ManaSpent(_)),
                ..
            }
        ),
        "{read:?}"
    );
}

#[rstest]
#[case::two_symbols("When ~ enters, sacrifice it unless {G}{G} was spent to cast it.")]
#[case::a_hybrid_symbol("When ~ enters, sacrifice it unless {R/G} was spent to cast it.")]
#[case::another_object("When ~ enters, sacrifice target creature unless {G} was spent to cast it.")]
#[case::no_mana("When ~ enters, sacrifice it unless mana was spent to cast it.")]
#[case::a_count("When ~ enters, sacrifice it unless three {G} was spent to cast it.")]
#[case::paid_with("When ~ enters, sacrifice it unless {G} was paid to cast it.")]
#[case::a_spell_that_is_not_the_card(
    "When ~ enters, sacrifice it unless {G} was spent to cast that spell."
)]
#[case::an_if_without_a_symbol("When ~ enters, if mana was spent to cast it, draw a card.")]
fn a_wording_that_differs_stays_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
