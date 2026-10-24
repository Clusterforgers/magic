//! A player shuffles cards of their graveyard into their library: "target player shuffles any number of target cards
//! from their graveyard into their library".

use super::{Effect, TargetSpec};
use crate::compile::read::amounts::count_word;
use crate::compile::read::selectors::{Form, phrase_text, spec_text};
use crate::effect::{Destination, Place};
use crate::refs::{Bounds, Pred, Ref, Selector, Targetable};
use crate::value::Amount;

/// The sentence for the cards of the second target, which the player of the first target shuffles into their library.
pub(super) fn text(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::Move {
            what: Ref::Target(1),
            to:
                Destination::Library {
                    place: Place::Shuffled,
                    order: None,
                },
        },
    ] = effects
    else {
        return None;
    };
    let [player, cards] = targets else {
        return None;
    };
    (matches!(player.what, Targetable::Player | Targetable::Opponent) && player.bounds.is_one())
        .then_some(())?;
    Some(format!(
        "{} shuffles {} into their library",
        spec_text(player)?,
        their_cards(cards)?
    ))
}

/// "any number of target cards from their graveyard", "up to two target creature cards from their graveyard": the cards
/// of the graveyard of the first target.
fn their_cards(spec: &TargetSpec) -> Option<String> {
    let Targetable::Object(Selector(preds)) = &spec.what else {
        return None;
    };
    let theirs = Pred::OwnedBy(Ref::Target(0));
    let (own, count) = (Pred::OwnedBy(Ref::You), &spec.bounds);
    (preds.contains(&theirs) && preds.contains(&Pred::In(crate::refs::ZoneKind::Graveyard)))
        .then_some(())?;
    let yours = Selector(
        preds
            .iter()
            .map(|pred| {
                if *pred == theirs {
                    own.clone()
                } else {
                    pred.clone()
                }
            })
            .collect(),
    );
    let phrase = phrase_text(&yours, Form::Targets)?.replace("your graveyard", "their graveyard");
    let opening = match (count.fixed_min(), &count.max) {
        (Some(0), None) => "any number of".to_owned(),
        (Some(0), Some(Amount::Fixed(most))) => {
            format!("up to {}", count_word(u32::try_from(*most).ok()?)?)
        }
        _ if *count == Bounds::default() => {
            let one =
                phrase_text(&yours, Form::Target)?.replace("your graveyard", "their graveyard");
            return Some(format!("target {one}"));
        }
        _ => return None,
    };
    Some(format!("{opening} target {phrase}"))
}
