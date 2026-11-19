//! Replacements over a token's entering (CR 111.1, 614.1c, 614.12): creating a token is its entering, so an effect that
//! modifies how a permanent enters ("Each creature you control enters with an additional +1/+1 counter on it", "As a
//! historic permanent you control enters, it becomes ...", a token's own "enter as a copy") applies to each token.
//!
//! The replacements of the creation itself (`ReplaceWhen::TokenCreation`) apply to the run of tokens first; then each
//! token of the run is a slot of its own, read as an entering. The token does not exist before the batch, so the step
//! reads the batch against a view of the state with its tokens made (`r109_token_view`): what a token is as it enters is
//! what it would be on the battlefield (CR 614.12). The view does not add the tokens' abilities to the effects that
//! apply: those were gathered before, and a token's own enters replacements are read as a card's are.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::ReplaceWhen;
use mtg_core::ids::ObjectId;

use super::{Item, Pass, Slot, Source};
use crate::events::Event;
use crate::state::GameState;

/// How many times the step runs again for tokens a replacement adds (each run adds at least one token to the view).
pub(super) const R109_RERUNS: usize = 8;

/// What splitting a run of tokens did.
pub(super) enum Split {
    /// It is no run, or no token can be replaced as it enters.
    No,
    /// The run is a slot for each token now.
    Done,
    /// Some of its tokens are not in the view the step reads: they come back to be added to it.
    Unseen(Vec<Event>),
}

/// Whether the rule modifies how a permanent enters.
fn r109_modifies_entering(rules: &Ability) -> bool {
    matches!(
        rules,
        Ability::Replacement {
            when: ReplaceWhen::Enters { .. },
            ..
        }
    )
}

/// The state with the tokens the batch creates (and those `added` by replacements on an earlier run) made, and their
/// ids, when an effect over a permanent's entering may apply to one of them: some source holds such a replacement, or a
/// token's own rules do. `None` otherwise, and when a token cannot be folded.
pub(super) fn r109_token_view(
    state: &GameState,
    proposed: &[Event],
    added: &[Event],
    sources: &[Source],
) -> Option<(GameState, Vec<ObjectId>)> {
    let tokens: Vec<&Event> = proposed
        .iter()
        .chain(added)
        .filter(|event| matches!(event, Event::TokenCreated(_)))
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let from_sources = sources
        .iter()
        .any(|source| r109_modifies_entering(&source.ability));
    let own = tokens.iter().any(|event| {
        matches!(event, Event::TokenCreated(token)
            if crate::ability_reader::printed_rules(&token.definition).any(|rules| r109_modifies_entering(rules)))
    });
    if !from_sources && !own {
        return None;
    }
    let mut view = state.clone();
    let mut made = Vec::with_capacity(tokens.len());
    for event in tokens {
        crate::fold::apply_event(&mut view, event).ok()?;
        if let Event::TokenCreated(token) = event {
            made.push(token.object);
        }
    }
    Some((view, made))
}

impl Pass<'_> {
    /// Whether `object` is a token the batch creates, read in the view where it exists.
    pub(super) fn r109_is_entering_token(&self, object: ObjectId) -> bool {
        self.r109_tokens.contains(&object)
    }

    /// Whether `object` is a token the batch creates by the embalm ability resolving now ("if this creature was
    /// embalmed", CR 702.128a): the ability its card's embalm adds.
    pub(super) fn r109_embalmed(&self, object: ObjectId) -> bool {
        if !self.r109_is_entering_token(object) {
            return false;
        }
        let Some(ability) = self
            .state
            .resolving()
            .and_then(|running| self.state.object(running.object))
            .and_then(crate::object::GameObject::ability)
        else {
            return false;
        };
        let address = crate::abilities::AbilityRef {
            object: ability.source.id,
            ability: ability.index,
            source: ability.origin,
        };
        matches!(
            crate::ability_reader::keyword_adding_as_it_was(self.state, address, &ability.snapshot),
            Some(mtg_cards::keyword::Keyword::Embalm(_))
        )
    }

    /// Splits the run of tokens at `at`, once no replacement of the creation applies to it any more, into a slot for
    /// each token, which is then read as that token's entering.
    pub(super) fn r109_enter_tokens(&mut self, at: usize) -> Split {
        if self.r109_tokens.is_empty() {
            return Split::No;
        }
        let Some(Slot {
            item: Item::Tokens(run),
            applied,
            frozen,
        }) = self.slots.get(at).cloned()
        else {
            return Split::No;
        };
        let unseen: Vec<Event> = run
            .iter()
            .filter(|token| !self.r109_tokens.contains(&token.object))
            .cloned()
            .map(Event::TokenCreated)
            .collect();
        if !unseen.is_empty() {
            return Split::Unseen(unseen);
        }
        let parts: Vec<Slot> = run
            .into_iter()
            .map(|token| Slot {
                item: Item::One(Event::TokenCreated(token)),
                applied: applied.clone(),
                frozen,
            })
            .collect();
        self.slots.splice(at..=at, parts);
        Split::Done
    }
}
