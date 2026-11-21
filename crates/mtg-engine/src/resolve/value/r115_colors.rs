//! "The chosen color is the most common color among nontoken permanents the chosen player controls but isn't tied for
//! most common" (`Condition::R115MostCommonColor`, CR 105.1): each matched object counts once for each of its colours.

use mtg_cards::refs::Selector;
use mtg_core::color::Color;

use super::{ResolveError, Scope};
use crate::object::Chosen;

impl Scope<'_> {
    /// Whether the colour the source chose is strictly the most common among the objects the selector matches.
    pub(super) fn r115_most_common_color(&self, among: &Selector) -> Result<bool, ResolveError> {
        let chosen = match self.entered_choice() {
            Some(Chosen::Color(color) | Chosen::R115ColorAndPlayer(color, _)) => *color,
            _ => return Ok(false),
        };
        let mut counts = [0usize; Color::ALL.len()];
        for object in self.matching(among)? {
            let Some(seen) = self.seen(object) else {
                continue;
            };
            for (at, color) in Color::ALL.iter().enumerate() {
                if seen.chars.colors.contains(*color)
                    && let Some(count) = counts.get_mut(at)
                {
                    *count += 1;
                }
            }
        }
        let mine = Color::ALL
            .iter()
            .position(|color| *color == chosen)
            .and_then(|at| counts.get(at).copied())
            .unwrap_or(0);
        Ok(Color::ALL
            .iter()
            .zip(counts)
            .all(|(color, count)| *color == chosen || count < mine))
    }
}

impl Scope<'_> {
    /// The player who caused the events a `Caused` pattern matches (`Ref::R115Causer`): the player of the cause as the
    /// pattern is matched, the one the trigger kept as it resolves.
    pub(super) fn r115_causer(&self) -> Vec<crate::choose::Target> {
        self.caused
            .map(|caused| caused.player)
            .or_else(|| self.cause.and_then(|cause| cause.r115_causer))
            .map(crate::choose::Target::Player)
            .into_iter()
            .collect()
    }
}
