//! The rules of r28-4 written as the card words them: 2 life paid as a spell of a kind is cast for one mana symbol of a
//! color less ("As an additional cost to cast green permanent spells, you may pay 2 life. Those spells cost {G} less to
//! cast if you paid life this way. This effect reduces only the amount of green mana you pay.").

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;

use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector};

/// The types of a permanent card (CR 110.4), as "permanent spells" selects them.
const PERMANENT_TYPES: [CardType; 6] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Land,
    CardType::Planeswalker,
];

/// The color word of "green permanent spells".
fn color_word(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Blue => "blue",
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
    }
}

/// "green permanent" for spells of one color and a permanent type.
fn spells_words(spells: &Selector) -> Option<String> {
    let [Pred::Color(color), Pred::AnyOf(kinds)] = spells.0.as_slice() else {
        return None;
    };
    let types: Vec<CardType> = kinds
        .iter()
        .map(|kind| match kind {
            Pred::Type(found) => Some(*found),
            _ => None,
        })
        .collect::<Option<_>>()?;
    (types == PERMANENT_TYPES).then(|| format!("{} permanent", color_word(*color)))
}

/// The sentences of a rule of r28-4 without the last period; empty for any other rule.
pub(super) fn r284_rule_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::PayLifeForOneSymbol {
        who: Ref::You,
        color,
        spells,
    } = rule
    else {
        return Vec::new();
    };
    let Some(kind) = spells_words(spells) else {
        return Vec::new();
    };
    let symbol = ManaType::from(*color).symbol();
    vec![format!(
        "As an additional cost to cast {kind} spells, you may pay 2 life. Those spells cost {{{symbol}}} less to cast if \
         you paid life this way. This effect reduces only the amount of {} mana you pay",
        color_word(*color)
    )]
}
