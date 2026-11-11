//! Chroma counts (CR 702 ability word): the mana symbols of one color in mana costs, written as the cards word them: "the
//! number of black mana symbols in the mana costs of cards in your graveyard", "for each white mana symbol in its mana
//! cost", "the number of red mana symbols in the sacrificed creature's mana cost".

use super::defining::counted_text;
use super::selectors::color_name;
use crate::refs::Ref;
use crate::value::Amount;

/// "the number of ..." of a count of mana symbols; `None` for another amount.
pub(super) fn counting_text(amount: &Amount) -> Option<String> {
    let Amount::ManaSymbols { color, of } = amount else {
        return None;
    };
    let color = color_name(*color);
    Some(match of {
        Ref::Each(selector) => format!(
            "the number of {color} mana symbols in the mana costs of {}",
            counted_text(selector)?
        ),
        Ref::Paid => {
            format!("the number of {color} mana symbols in the sacrificed creature's mana cost")
        }
        Ref::Affected => format!("the number of {color} mana symbols in its mana cost"),
        _ => return None,
    })
}

/// What "for each" counts of a count of mana symbols: "white mana symbol in its mana cost".
pub(super) fn each_text(amount: &Amount) -> Option<String> {
    let Amount::ManaSymbols { color, of } = amount else {
        return None;
    };
    let color = color_name(*color);
    Some(match of {
        Ref::Affected => format!("{color} mana symbol in its mana cost"),
        Ref::Paid => format!("{color} mana symbol in the sacrificed creature's mana cost"),
        Ref::Each(selector) => format!(
            "{color} mana symbol in the mana costs of {}",
            counted_text(selector)?
        ),
        _ => return None,
    })
}
