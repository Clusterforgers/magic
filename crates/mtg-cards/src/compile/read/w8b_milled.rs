//! Mill triggers: "whenever a player mills a nonland card", "whenever an opponent mills a nonland card" and "whenever one
//! or more nonland cards are milled", read into `Milled` (a batch for "one or more") and written back the same way.

use super::library::filter_text;
use super::selectors::{Form, phrase_in};
use super::tokens::Reader;
use super::triggers::{article, player_verb};
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

/// "a nonland card" after "mills": the card a player mills.
pub(super) fn w8b_mills(r: &mut Reader<'_>, by: Ref) -> Option<TriggerPattern> {
    (r.word("a") || r.word("an")).then_some(())?;
    let what = w8b_card(phrase_in(r, Form::Subject, true)?)?;
    Some(TriggerPattern::Milled { what, by })
}

/// "nonland cards are milled" after "one or more": the cards milled together, by any player.
pub(super) fn w8b_batch(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let what = w8b_card(phrase_in(r, Form::All, true)?)?;
    r.need("are milled")?;
    Some(TriggerPattern::Batch(Box::new(TriggerPattern::Milled {
        what,
        by: Ref::EachPlayer,
    })))
}

/// "if ~ is in your graveyard," after the event of a trigger: the card's ability works from its graveyard.
pub(super) fn w8b_in_graveyard(r: &mut Reader<'_>) -> Option<()> {
    r.word("if").then_some(())?;
    r.source().then_some(())?;
    r.need("is in your graveyard")?;
    r.punct(',').then_some(())
}

/// "you may return it to your hand." after "if ~ is in your graveyard,": the card may return itself to its owner's hand.
pub(super) fn w8b_return_it(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may return it to your hand")?;
    r.sentence_end().then_some(())?;
    Some(vec![Effect::May {
        who: Ref::You,
        effects: vec![Effect::Move {
            what: Ref::This,
            to: Destination::Hand,
        }],
    }])
}

/// The wordings of a trigger whose card returns itself from its graveyard to the hand that say the card is there after
/// the event: "Whenever an opponent mills a nonland card, if ~ is in your graveyard, you may return it to your hand."
pub(super) fn w8b_in_graveyard_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains("return ~ to its owner's hand"))
        .filter_map(|text| {
            let (event, tail) = text.split_once(", ")?;
            let tail = tail.replace("return ~ to its owner's hand", "return it to your hand");
            Some(format!("{event}, if ~ is in your graveyard, {tail}"))
        })
        .collect()
}

/// The cards of a mill are named by kind alone: no zone, controller or owner (the owner is who mills).
fn w8b_card(what: Selector) -> Option<Selector> {
    let plain = what.0.iter().all(|pred| {
        !matches!(
            pred,
            Pred::This
                | Pred::Other
                | Pred::In(_)
                | Pred::OwnedBy(_)
                | Pred::ControlledBy(_)
                | Pred::Is(_)
        )
    });
    plain.then_some(what)
}

/// "Whenever a player mills a nonland card".
pub(super) fn w8b_text(what: &Selector, by: &Ref) -> Option<String> {
    let card = filter_text(&w8b_card(what.clone())?, false)?;
    Some(format!(
        "Whenever {} {} {card}",
        player_verb(by, "mill")?,
        article(&card)
    ))
}

/// "Whenever one or more nonland cards are milled".
pub(super) fn w8b_batch_text(what: &Selector, by: &Ref) -> Option<String> {
    (*by == Ref::EachPlayer).then_some(())?;
    let cards = filter_text(&w8b_card(what.clone())?, true)?;
    Some(format!("Whenever one or more {cards} are milled"))
}
