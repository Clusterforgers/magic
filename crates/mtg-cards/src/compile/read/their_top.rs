//! Another player exiling the top of their own library: "defending player exiles the top twenty cards of their library",
//! "each opponent discards a card and exiles the top three cards of their library", and for a target the cards' own
//! wording "exile the top three cards of target opponent's library", each read from its wording and written back the
//! same way.

use super::library::{top_cards, top_text};
use super::tokens::Reader;
use crate::effect::{EXILED, Effect};
use crate::refs::Ref;

/// "exiles the top twenty cards of their library" after the subject `who`, a player other than you and the one of the
/// event (whose wording is "exile the top card of that player's library").
pub(super) fn exiles_top(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    (!matches!(who, Ref::You | Ref::Triggering)).then_some(())?;
    r.need("exiles")?;
    let count = top_cards(r)?;
    r.need("of their library")?;
    Some(vec![Effect::ExileTop {
        who: who.clone(),
        count,
        bind: EXILED.to_owned(),
        sealed: false,
        bottom: false,
    }])
}

/// The verb phrase of such an exile without its subject, "exiles the top three cards of their library"; none for
/// another effect.
pub(super) fn phrase(effect: &Effect) -> Option<String> {
    match effect {
        Effect::ExileTop {
            who,
            count,
            sealed: false,
            bottom: false,
            ..
        } if !matches!(who, Ref::You | Ref::Triggering) => {
            Some(format!("exiles {} of their library", top_text(count)?))
        }
        _ => None,
    }
}

/// Adds "exile the top three cards of target opponent's library" for each text with "target opponent exiles the top three
/// cards of their library": the wording the cards print for a target player.
pub(super) fn possessive_wordings(texts: &mut Vec<String>) {
    let more: Vec<String> = texts.iter().filter_map(|text| possessive(text)).collect();
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The possessive wording of one text; none without a target player exiling the top of their library.
fn possessive(text: &str) -> Option<String> {
    // The subject opens the sentence with a capital, or follows a comma without one.
    let (written, subject, verb) = [
        ("Target opponent", "target opponent", "Exile"),
        ("Target player", "target player", "Exile"),
        ("target opponent", "target opponent", "exile"),
        ("target player", "target player", "exile"),
    ]
    .into_iter()
    .find(|(written, ..)| text.contains(&format!("{written} exiles the top ")))?;
    let at = text.find(&format!("{written} exiles the top "))?;
    let rest = &text[at + written.len() + " exiles ".len()..];
    let end = rest.find(" of their library")?;
    Some(format!(
        "{}{verb} {} of {subject}'s library{}",
        &text[..at],
        &rest[..end],
        &rest[end + " of their library".len()..]
    ))
}
