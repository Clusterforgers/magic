//! Invariants of what the table was shown or told by hand: a card revealed from a hand is identified to every seat while
//! it is still that object in that hand, a revealed top card is hidden again once its library is shuffled, and a note
//! stays on its card while the card is the same object, leaves with it, and the same text is refused again.

use mtg_core::ids::PlayerId;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualReveal, check};
use mtg_engine::object::ObjectRef;
use mtg_engine::sight::{note_of, revealed_hand};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};

use crate::invariants::Violation;

/// The cards shown and the notes kept by hand so far, as the events say.
#[derive(Debug, Clone, Default)]
pub struct ShownByHand {
    /// Cards revealed from a hand: the seat and the object, shown while it is that object in that hand.
    hand: Vec<(PlayerId, ObjectRef)>,
    /// Top cards revealed: the owner of the library and the object, shown while it is that object in that library.
    top: Vec<(PlayerId, ObjectRef)>,
    /// Revealed library cards whose library has been shuffled since: hidden again.
    hidden: Vec<(PlayerId, ObjectRef)>,
    /// Notes kept: the seat, the object and the trimmed text.
    notes: Vec<(PlayerId, ObjectRef, String)>,
}

fn bad(what: String) -> Violation {
    Violation::Bookkeeping(what)
}

impl ShownByHand {
    /// Takes in `event` before it is folded onto `before`: a card revealed, a note put or cleared, a library shuffled.
    pub fn before(&mut self, before: &GameState, event: &Event) {
        match event {
            Event::ManualAction { player, action } => match &**action {
                ManualAction::Reveal { what } => self.reveal(before, *player, *what),
                ManualAction::Note { object, text } => {
                    self.notes.retain(|(_, held, _)| held.id != *object);
                    if let (false, Some(on)) = (text.trim().is_empty(), before.object(*object)) {
                        self.notes
                            .push((*player, on.reference(), text.trim().to_owned()));
                    }
                }
                _ => {}
            },
            Event::LibraryShuffled { player, .. } => {
                let (shuffled, kept) = std::mem::take(&mut self.top)
                    .into_iter()
                    .partition(|(owner, _)| owner == player);
                self.top = kept;
                self.hidden.extend(shuffled);
            }
            _ => {}
        }
    }

    fn reveal(&mut self, before: &GameState, player: PlayerId, what: ManualReveal) {
        let (card, library) = match what {
            ManualReveal::Hand { object } => (Some(object), false),
            ManualReveal::Top {} => (before.zone(Zone::Library(player)).last().copied(), true),
        };
        let Some(on) = card
            .and_then(|id| before.object(id))
            .map(mtg_engine::object::GameObject::reference)
        else {
            return;
        };
        self.hidden.retain(|(_, held)| *held != on);
        if library {
            self.top.push((player, on));
        } else {
            self.hand.push((player, on));
        }
    }

    /// How many cards and notes are being watched.
    pub fn watched(&self) -> usize {
        self.hand.len() + self.top.len() + self.hidden.len() + self.notes.len()
    }

    /// Checks `state` against what was shown and kept, and lets go of what has left its place.
    pub fn check(&mut self, state: &GameState) -> Result<(), Violation> {
        let seats: Vec<PlayerId> = state
            .players()
            .iter()
            .map(mtg_engine::player::Player::id)
            .collect();
        let here = |held: &ObjectRef| state.object(held.id).filter(|o| o.reference() == *held);
        for (seat, held) in std::mem::take(&mut self.hand) {
            match here(&held).map(mtg_engine::object::GameObject::zone) {
                Some(Zone::Hand(owner)) if owner == seat => {
                    if !revealed_hand(state, owner).contains(&held.id) {
                        return Err(bad(format!(
                            "{:?} was revealed from a hand and is not shown there",
                            held.id
                        )));
                    }
                    if let Some(unseen) = seats
                        .iter()
                        .find(|viewer| !state.is_shown_to(**viewer, held.id))
                    {
                        return Err(bad(format!(
                            "{:?} was revealed from a hand and {unseen:?} cannot identify it",
                            held.id
                        )));
                    }
                    self.hand.push((seat, held));
                }
                _ => {
                    if revealed_hand(state, seat).contains(&held.id) {
                        return Err(bad(format!(
                            "{:?} left the hand it was revealed in and is still shown",
                            held.id
                        )));
                    }
                }
            }
        }
        for (owner, held) in std::mem::take(&mut self.top) {
            if here(&held).is_some_and(|o| o.zone() == Zone::Library(owner)) {
                if let Some(unseen) = seats
                    .iter()
                    .find(|viewer| !state.is_shown_to(**viewer, held.id))
                {
                    return Err(bad(format!(
                        "{:?} was revealed on a library and {unseen:?} cannot identify it",
                        held.id
                    )));
                }
                self.top.push((owner, held));
            }
        }
        for (owner, held) in std::mem::take(&mut self.hidden) {
            let buried = here(&held).is_some_and(|o| o.zone() == Zone::Library(owner));
            let by_rule = state.revealed_top(owner).is_some();
            if buried && !by_rule {
                if let Some(seen) = seats
                    .iter()
                    .find(|viewer| **viewer != owner && state.is_shown_to(**viewer, held.id))
                {
                    return Err(bad(format!(
                        "{:?} was revealed, its library was shuffled and {seen:?} still identifies it",
                        held.id
                    )));
                }
            }
        }
        self.check_notes(state)
    }

    fn check_notes(&mut self, state: &GameState) -> Result<(), Violation> {
        for (seat, held, text) in std::mem::take(&mut self.notes) {
            let same = state.object(held.id).is_some_and(|o| o.reference() == held);
            if !same {
                if note_of(state, held.id).is_some_and(|now| now == text) {
                    return Err(bad(format!(
                        "the note {text:?} stayed on {:?} after the card left",
                        held.id
                    )));
                }
                continue;
            }
            if note_of(state, held.id) != Some(text.as_str()) {
                return Err(bad(format!(
                    "the note {text:?} on {:?} reads {:?}",
                    held.id,
                    note_of(state, held.id)
                )));
            }
            let again = ManualAction::Note {
                object: held.id,
                text: text.clone(),
            };
            match check(state, seat, &again) {
                Err(
                    ManualRefusal::NoChange
                    | ManualRefusal::NotYours
                    | ManualRefusal::WrongZone { .. },
                ) => {}
                other => {
                    return Err(bad(format!(
                        "the note {text:?} on {:?} was not refused again: {other:?}",
                        held.id
                    )));
                }
            }
            self.notes.push((seat, held, text));
        }
        Ok(())
    }
}
