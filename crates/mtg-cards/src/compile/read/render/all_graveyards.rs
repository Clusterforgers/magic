//! A mass move from every graveyard to the battlefield under the controller: "Put all creature cards from all graveyards
//! onto the battlefield under your control."

use super::{Destination, Effect, Form, Pred, Ref, ZoneKind, phrase_text};

/// "put all creature cards from all graveyards onto the battlefield under your control".
pub(super) fn text(effect: &Effect) -> Option<String> {
    let Effect::Move {
        what: Ref::Each(selector),
        to:
            Destination::Battlefield {
                tapped: false,
                under: Some(Ref::You),
                transformed: false,
                counters,
            },
    } = effect
    else {
        return None;
    };
    let any_graveyard = selector.0.contains(&Pred::In(ZoneKind::Graveyard))
        && !selector
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::OwnedBy(_)));
    (counters.is_empty() && any_graveyard).then_some(())?;
    let phrase = phrase_text(selector, Form::All)?;
    let cards = phrase.strip_suffix(" from a graveyard")?;
    Some(format!(
        "put all {cards} from all graveyards onto the battlefield under your control"
    ))
}
