//! Cards taken at random from a graveyard: "Return an instant or sorcery card at random from your graveyard to your
//! hand." and "Return X cards at random from your graveyard to your hand.", each read from its wording and written back
//! the same way. The pick is a `ChooseAtRandom` of the game's random source, bound as `card`, and the move of it.

use super::amounts::{count_word, number_word};
use super::library::{filter, filter_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// The name the card picked at random is bound to.
pub const PICKED: &str = "card";

/// The zone and owner a pick at random reads: your graveyard.
fn yours() -> [Pred; 2] {
    [Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]
}

/// "Return an instant or sorcery card at random from your graveyard to your hand.", "Return two creature cards at
/// random from your graveyard to the battlefield.": the cards come back one at a time from a set that only the
/// random source orders.
pub fn from_graveyard(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("return")?;
    let (count, plural) = if r.word("a") || r.word("an") {
        (Amount::one(), false)
    } else if r.x() {
        (Amount::X, true)
    } else {
        let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
        (Amount::Fixed(i32::try_from(number).ok()?), true)
    };
    let found = filter(r, plural)?;
    r.need("at random from your graveyard")?;
    let to = if r.words("to your hand") {
        Destination::Hand
    } else {
        r.need("to the battlefield")?;
        Destination::Battlefield {
            tapped: false,
            under: None,
            transformed: false,
            counters: Vec::new(),
        }
    };
    let mut preds = yours().to_vec();
    preds.extend(found.0);
    Some(vec![
        Effect::ChooseAtRandom {
            from: Ref::Each(Selector(preds)),
            count,
            bind: PICKED.to_owned(),
        },
        Effect::Move {
            what: Ref::Bound(PICKED.to_owned()),
            to,
        },
    ])
}

/// The sentence for a pick at random from your graveyard and the move of what was picked.
pub(super) fn text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::ChooseAtRandom {
            from: Ref::Each(from),
            count,
            bind,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to,
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != PICKED || name != PICKED || !from.0.starts_with(&yours()) {
        return None;
    }
    let found = Selector(from.0[yours().len()..].to_vec());
    let to = match to {
        Destination::Hand => "to your hand",
        Destination::Battlefield {
            tapped: false,
            under: None,
            transformed: false,
            counters,
        } if counters.is_empty() => "to the battlefield",
        _ => return None,
    };
    let cards = match count {
        Amount::Fixed(1) => {
            let text = filter_text(&found, false)?;
            format!("{} {text}", article(&text))
        }
        Amount::X => format!("X {}", filter_text(&found, true)?),
        Amount::Fixed(number) => format!(
            "{} {}",
            count_word(u32::try_from(*number).ok().filter(|number| *number > 1)?)?,
            filter_text(&found, true)?
        ),
        _ => return None,
    };
    Some(format!("return {cards} at random from your graveyard {to}"))
}
