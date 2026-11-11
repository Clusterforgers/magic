//! A player reveals their hand and you pick the card they discard: "Target player reveals their hand. You choose a
//! nonland card from it with mana value 2 or less. That player discards that card." Read as the reveal of the cards in
//! that hand and a discard of one card of the kind, chosen by you (`Discard { by: You }`); written back the same way.

use super::selectors::{Form, phrase_in, phrase_text, spec_text, suffix, suffix_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, TargetSpec, ZoneKind};
use crate::value::Amount;

/// The cards in the hand of `who`.
fn hand_of(who: &Ref) -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(who.clone()),
    ]))
}

/// "Target player reveals their hand. You choose a nonland card from it[ with mana value 2 or less]. That player
/// discards that card": the reveal and the discard you choose; the last period is left to the caller.
pub(super) fn rmt0_hand_pick(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = super::effects::player(r)?;
    matches!(who, Ref::Target(_)).then_some(())?;
    r.need("reveals their hand")?;
    r.punct('.').then_some(())?;
    r.need("you choose")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let Selector(mut preds) = phrase_in(r, Form::Count, true)?;
    r.need("from it")?;
    if let Some(more) = r.attempt(|r| suffix(r, Form::Count)) {
        preds.push(more);
    }
    r.punct('.').then_some(())?;
    r.need("that player discards that card")?;
    Some(vec![
        Effect::Reveal {
            what: hand_of(&who),
        },
        Effect::Discard {
            who,
            count: Amount::Fixed(1),
            random: false,
            filter: (!preds.is_empty()).then_some(Selector(preds)),
            by: Some(Ref::You),
            optional: false,
            bind: None,
        },
    ])
}

/// The three sentences the reveal and the chosen discard at the front of `effects` are, and how many effects they take.
pub(super) fn rmt0_sequence(effects: &[Effect], targets: &[TargetSpec]) -> Option<(String, usize)> {
    let [
        Effect::Reveal { what },
        Effect::Discard {
            who: who @ Ref::Target(index),
            count: Amount::Fixed(1),
            random: false,
            filter,
            by: Some(Ref::You),
            optional: false,
            bind: None,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    (*what == hand_of(who)).then_some(())?;
    let player = spec_text(targets.get(usize::from(*index))?)?;
    let preds = filter
        .as_ref()
        .map_or(&[][..], |filter| filter.0.as_slice());
    // A comparison of a characteristic is said after "from it".
    let (after, kind): (Vec<&Pred>, Vec<&Pred>) = preds
        .iter()
        .partition(|pred| matches!(pred, Pred::Compare { .. }));
    let kind: Vec<Pred> = kind.into_iter().cloned().collect();
    // The cards of a hand are cards: "nonland card", not "nonland permanent".
    let noun = phrase_text(&Selector(kind), Form::Count)?;
    let noun = noun.strip_suffix(" permanent").unwrap_or(&noun);
    let noun = format!("{} card", noun.strip_suffix(" card").unwrap_or(noun));
    let mut tail = String::new();
    for pred in after {
        tail.push(' ');
        tail.push_str(&suffix_text(pred, Form::Count)?);
    }
    let article = super::triggers::article(&noun);
    Some((
        format!(
            "{player} reveals their hand. You choose {article} {noun} from it{tail}. That player discards that card"
        ),
        2,
    ))
}
