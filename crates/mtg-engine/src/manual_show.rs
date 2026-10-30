//! The manual actions that show something to the table: a card of the seat's hand or the top card of its library to
//! every seat, and a short note on a card every seat sees.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::events::Event;
use crate::library::{Revealed, top_cards};
use crate::manual::{MAX_NOTE_CHARS, ManualAction, ManualRefusal, ManualReveal, own};
use crate::state::GameState;
use crate::zones::Zone;

/// The card a reveal names: a card of the seat's own hand, or the top card of its library.
fn shown_card(
    state: &GameState,
    player: PlayerId,
    what: ManualReveal,
) -> Result<ObjectId, ManualRefusal> {
    match what {
        ManualReveal::Hand { object } => {
            let card = own(state, player, object)?;
            if card.zone() == Zone::Hand(player) {
                Ok(object)
            } else {
                Err(ManualRefusal::WrongZone {
                    object,
                    zone: card.zone(),
                })
            }
        }
        ManualReveal::Top {} => top_cards(state, player, 1)
            .first()
            .copied()
            .ok_or(ManualRefusal::NotEnough("cards in the library")),
    }
}

/// The seat shows a card to every seat: a `Revealed` that names it, and from then on every view identifies it.
pub(crate) fn reveal(
    state: &GameState,
    player: PlayerId,
    what: ManualReveal,
) -> Result<Vec<Event>, ManualRefusal> {
    let card = shown_card(state, player, what)?;
    let seen_by_all = (0..state.players().len())
        .map(PlayerId::from_index)
        .all(|seat| state.is_shown_to(seat, card));
    if seen_by_all {
        return Err(ManualRefusal::NoChange);
    }
    Ok(vec![Event::Revealed(Revealed {
        player,
        cards: vec![card],
    })])
}

/// The note `text` as it is kept: trimmed, on one line and up to [`MAX_NOTE_CHARS`] characters.
fn clean_note(text: &str) -> Result<&str, ManualRefusal> {
    let text = text.trim();
    if text.chars().count() > MAX_NOTE_CHARS || text.chars().any(char::is_control) {
        return Err(ManualRefusal::OutOfRange("the note"));
    }
    Ok(text)
}

/// The seat puts a note on a card it owns or controls in a zone every seat sees into, or clears it with an empty text.
/// The note itself is kept when the action is folded (`apply`), so the action has no events of its own.
pub(crate) fn note(
    state: &GameState,
    player: PlayerId,
    object: ObjectId,
    text: &str,
) -> Result<Vec<Event>, ManualRefusal> {
    let card = own(state, player, object)?;
    if matches!(
        card.zone(),
        Zone::Hand(_) | Zone::Library(_) | Zone::PhasedOut
    ) {
        return Err(ManualRefusal::WrongZone {
            object,
            zone: card.zone(),
        });
    }
    let text = clean_note(text)?;
    if crate::sight::note_of(state, object).unwrap_or("") == text {
        return Err(ManualRefusal::NoChange);
    }
    Ok(Vec::new())
}

/// What folding a manual action `action` of `player` records: the card shown to every seat, the note on a card.
pub(crate) fn apply(state: &mut GameState, player: PlayerId, action: &ManualAction) {
    match action {
        ManualAction::Reveal { what } => {
            if let Ok(card) = shown_card(state, player, *what) {
                crate::sight::reveal_by_hand(state, card);
            }
        }
        ManualAction::Note { object, text } => {
            crate::sight::note_by_hand(state, *object, text.trim());
        }
        _ => {}
    }
}
