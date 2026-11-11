//! A card that an opponent makes its owner discard: "If a spell or ability an opponent controls causes you to discard
//! ~, put it onto the battlefield [with two +1/+1 counters on it] instead of putting it into your graveyard." is a
//! replacement of the card's own discard, from the hand, caused by an opponent (`ReplaceWhen::Discarded` with `by`).

use super::counters::{counters, text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// The words before the card and between the card and the counters.
const OPENING: &str = "if a spell or ability an opponent controls causes you to discard";

/// "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield instead of putting it
/// into your graveyard.": the card enters from the hand in place of the discard.
pub(super) fn ep32_discarded_by_opponent(r: &mut Reader<'_>) -> Option<Ability> {
    r.need(OPENING)?;
    r.source().then_some(())?;
    r.punct(',').then_some(())?;
    r.need("put it onto the battlefield")?;
    let counters = r
        .attempt(|r| {
            r.need("with")?;
            let found = counters(r)?;
            r.need("on it")?;
            Some(found)
        })
        .map_or_else(Vec::new, |found| vec![found]);
    r.need("instead of putting it into your graveyard")?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Discarded {
            what: Selector::this(),
            by: Some(Ref::Opponents),
        },
        by: ReplaceBy::Instead(vec![Effect::Move {
            what: Ref::This,
            to: Destination::Battlefield {
                tapped: false,
                under: None,
                transformed: false,
                counters,
            },
            bind: None,
        }]),
        condition: None,
        from: ZoneKind::Hand,
    })
}

/// The sentence of [`ep32_discarded_by_opponent`], without the period; empty for any other ability.
pub(super) fn ep32_discarded_texts(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::Discarded {
                what,
                by: Some(Ref::Opponents),
            },
        by: ReplaceBy::Instead(effects),
        condition: None,
        from: ZoneKind::Hand,
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::Move {
            what: Ref::This,
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters,
                },
            bind: None,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if *what != Selector::this() {
        return Vec::new();
    }
    let with = match counters.as_slice() {
        [] => String::new(),
        [(counter, number)] => {
            let Some(count) = i32::try_from(*number).ok() else {
                return Vec::new();
            };
            match text(counter, &Amount::Fixed(count)) {
                Some(words) => format!(" with {words} on it"),
                None => return Vec::new(),
            }
        }
        _ => return Vec::new(),
    };
    vec![format!(
        "{OPENING} ~, put it onto the battlefield{with} instead of putting it into your graveyard"
    )]
}
