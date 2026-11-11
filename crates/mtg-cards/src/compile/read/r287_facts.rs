//! Facts of the turn an intervening "if" asks (r28-7): "if one or more cards were put into exile this turn", "if a
//! permanent was put into your hand from the battlefield this turn", "if three
//! or more cards were put into your graveyard from anywhere other than the battlefield this turn".

use super::facts::quantity_texts;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, TurnTally};

/// The wordings of the tally compared with the number, or nothing when no row has it.
pub(super) fn texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    let Amount::Tallied { who, what } = left else {
        return Vec::new();
    };
    if op != Cmp::Ge || number == 0 {
        return Vec::new();
    }
    let many = |pattern: &dyn Fn(&str) -> String| -> Vec<String> {
        quantity_texts(number)
            .iter()
            .map(|words| pattern(words))
            .collect()
    };
    match (who, what, number) {
        (Ref::EachPlayer, TurnTally::CardsExiled, 1) => vec![
            "one or more cards were put into exile this turn".to_owned(),
            "a card was put into exile this turn".to_owned(),
        ],
        (Ref::You, TurnTally::PermanentsToHand, 1) => {
            vec!["a permanent was put into your hand from the battlefield this turn".to_owned()]
        }
        (Ref::You, TurnTally::CardsGravedNotFromBattlefield, _) => many(&|words| {
            format!(
                "{words} or more cards were put into your graveyard from anywhere other than the battlefield this turn"
            )
        }),
        _ => Vec::new(),
    }
}
