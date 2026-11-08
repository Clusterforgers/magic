//! The words of a keyword a static ability gives that the keyword list does not read: a unit keyword ("convoke",
//! "cascade"), one with a number or a cost ("afflict 3", "casualty 2", "outlast {2}", "prowl {2}{R}") and one the table
//! expands ("training", "flanking"), and the quoted keyword abilities ("Ward—Pay 2 life.").

use super::super::super::selectors::{Form, phrase_text};
use crate::cost::{Cost, CostItem};
use crate::keyword::Keyword;
use crate::refs::{Pred, Selector};
use crate::value::Amount;
use mtg_core::card::CardType;

/// How a keyword is said in a list of the card's words: a plain word, or a quoted ability that ends the sentence.
pub(super) struct Words {
    /// The words.
    pub(super) text: String,
    /// The keyword is said as a quoted ability with its own period.
    pub(super) quoted: bool,
}

/// The words of a keyword given with the keyword written as `unit` says it, if the card words it so.
pub(super) fn words(keyword: &Keyword, unit: bool) -> Option<Words> {
    let plain = |text: String| {
        Some(Words {
            text,
            quoted: false,
        })
    };
    let name = keyword.name().to_lowercase();
    match keyword {
        Keyword::Afflict(number) | Keyword::Casualty(number) | Keyword::Ripple(number) => {
            plain(format!("{name} {number}"))
        }
        Keyword::Outlast(cost) | Keyword::Prowl(cost) | Keyword::Replicate(cost) => {
            plain(format!("{name} {}", mana_only(cost)?))
        }
        Keyword::Macro { name, amount: 0 } => plain(name.clone()),
        Keyword::Macro { name, amount } => plain(format!("{name} {amount}")),
        Keyword::Affinity(kind) => plain(format!("affinity for {}", phrase_text(kind, Form::All)?)),
        Keyword::Landwalk(Selector(preds)) => match preds.as_slice() {
            [Pred::Type(CardType::Land), Pred::Type(CardType::Artifact)] => {
                plain("artifact landwalk".to_owned())
            }
            _ => None,
        },
        Keyword::Ward(cost) => Some(Words {
            text: format!("\"Ward—{}.\"", cost_words(cost)?),
            quoted: true,
        }),
        Keyword::CumulativeUpkeep(cost) => {
            let paid = match mana_only(cost) {
                Some(mana) => format!(" {mana}"),
                None => format!("—{}", cost_words(cost)?),
            };
            Some(Words {
                text: format!("\"Cumulative upkeep{paid}.\""),
                quoted: true,
            })
        }
        _ if unit => plain(name),
        _ => None,
    }
}

/// Whether the changes are keywords and pumps only, which a condition's words can follow once ("has flying as long as ...").
pub(super) fn only_keywords(mods: &[crate::modify::Modification]) -> bool {
    use crate::modify::Modification;
    mods.iter().all(|m| {
        matches!(
            m,
            Modification::AddKeyword(_) | Modification::ModifyPT { .. }
        )
    })
}

/// The mana of a cost that is only mana.
fn mana_only(cost: &Cost) -> Option<String> {
    match cost.0.as_slice() {
        [CostItem::Mana(mana)] => Some(mana.to_string()),
        _ => None,
    }
}

/// What a cost asks for: "Pay 2 life", "Sacrifice a permanent".
fn cost_words(cost: &Cost) -> Option<String> {
    match cost.0.as_slice() {
        [CostItem::PayLife(Amount::Fixed(life))] => Some(format!("Pay {life} life")),
        [CostItem::Sacrifice { filter, count }]
            if filter.0.is_empty() && *count == Amount::one() =>
        {
            Some("Sacrifice a permanent".to_owned())
        }
        _ => None,
    }
}
