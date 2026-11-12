//! "the number of times it's been cast from the command zone this game": the casts of each named commander, alone.

use mtg_cards::refs::Ref;

use super::{ResolveError, Scope};
use crate::choose::Target;

impl Scope<'_> {
    /// How many times the commanders `what` names were cast from the command zone this game (CR 903.8), each counted
    /// on its own; an object that is no commander counts nothing (`Amount::CastsFromCommand`).
    pub(crate) fn r300b_casts_from_command(&self, what: &Ref) -> Result<i64, ResolveError> {
        Ok(self
            .refs(what)?
            .into_iter()
            .filter_map(|named| match named {
                Target::Object(object) => Some(i64::from(self.state.commander_casts(object.id))),
                Target::Player(_) => None,
            })
            .sum())
    }
}
