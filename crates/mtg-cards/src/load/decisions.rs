//! What a library decided of each card, kept so that a card is decided once per library, printed text and engine.
//!
//! Deciding a card reads its rules back against the printed text, which costs milliseconds; a deck check, a precon
//! index or a game asks the same library about the same cards over and over. A library is never changed after it is
//! built, so a decision stays true for as long as the printed card and the capabilities asked with are the same.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::assemble::Assembled;
use crate::deck::ReadDiffers;
use crate::draft::ReadState;
use crate::support::{Caps, PrintedCard, SupportRecord};

/// What one card was found to be, each part filled the first time it is asked for.
#[derive(Debug, Clone)]
pub(crate) struct Decided {
    printed: PrintedCard,
    /// The support record and whether a draft decides the card.
    pub(crate) record: Option<(SupportRecord, bool)>,
    /// Whether `draft_state` is decided.
    pub(crate) draft_known: bool,
    /// The read state the deck check shows, once `draft_known`.
    pub(crate) draft_state: Option<ReadState>,
    /// The definition the engine plays.
    pub(crate) assembled: Option<Assembled>,
    /// The clauses the card's draft reads back as other text.
    pub(crate) differs: Option<Vec<ReadDiffers>>,
}

impl Decided {
    /// Nothing decided yet of `printed`.
    fn of(printed: &PrintedCard) -> Decided {
        Decided {
            printed: printed.clone(),
            record: None,
            draft_known: false,
            draft_state: None,
            assembled: None,
            differs: None,
        }
    }
}

#[derive(Debug, Default)]
struct Held {
    /// The capabilities every held decision was made under.
    caps: Option<Caps>,
    /// Shared with the libraries that took them over, so taking over copies no decision.
    cards: HashMap<String, Arc<Decided>>,
}

/// The decisions of one library, by oracle id. A clone starts empty: a cloned library is changed before it is used.
#[derive(Default)]
pub(crate) struct Decisions {
    held: Mutex<Held>,
}

impl Clone for Decisions {
    fn clone(&self) -> Decisions {
        Decisions::default()
    }
}

impl fmt::Debug for Decisions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        write!(f, "Decisions({} cards)", held.cards.len())
    }
}

impl Decisions {
    /// The part `pick` takes of the decision on `printed`, computed by `make` and kept with `keep` when there is none.
    /// `make` runs without the lock, so two callers may both compute a card; they compute the same.
    pub(crate) fn get<T: Clone>(
        &self,
        printed: &PrintedCard,
        caps: &Caps,
        pick: impl Fn(&Decided) -> Option<T>,
        make: impl FnOnce() -> T,
        keep: impl FnOnce(&mut Decided, T),
    ) -> T {
        {
            let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
            if held.caps.as_ref() == Some(caps)
                && let Some(decided) = held.cards.get(&printed.oracle_id)
                && decided.printed == *printed
                && let Some(found) = pick(decided)
            {
                return found;
            }
        }
        let answer = make();
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if held.caps.as_ref() != Some(caps) {
            held.caps = Some(caps.clone());
            held.cards.clear();
        }
        let decided = held
            .cards
            .entry(printed.oracle_id.clone())
            .or_insert_with(|| Arc::new(Decided::of(printed)));
        if decided.printed != *printed {
            *decided = Arc::new(Decided::of(printed));
        }
        keep(Arc::make_mut(decided), answer.clone());
        answer
    }

    /// Takes over the decisions of `previous` on the cards `same` says this library decides as `previous` does.
    pub(crate) fn inherit(&self, previous: &Decisions, same: impl Fn(&str) -> bool) {
        let taken = {
            let old = previous.held.lock().unwrap_or_else(PoisonError::into_inner);
            Held {
                caps: old.caps.clone(),
                cards: old
                    .cards
                    .iter()
                    .filter(|(id, _)| same(id))
                    .map(|(id, decided)| (id.clone(), decided.clone()))
                    .collect(),
            }
        };
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if held.cards.is_empty() {
            *held = taken;
        }
    }

    /// How many cards have a decision held.
    pub(crate) fn len(&self) -> usize {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cards
            .len()
    }
}
