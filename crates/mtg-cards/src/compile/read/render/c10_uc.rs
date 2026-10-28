//! Taps that hold permanents down, as one pair of sentences that no joint splits: while you control the source, and a
//! target player's creatures through that player's next untap step.

use super::Writer;
use crate::compile::read::c10_uc::{C10_UC_CONTROL_TAILS, C10_UC_SLEEP, c10_uc_players_creatures};
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

impl Writer<'_> {
    /// The tap and the rule that holds the same permanents down, and the number of effects it covers.
    pub(super) fn c10_uc_held(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            tap @ Effect::Tap { what },
            Effect::Continuous {
                affected,
                mods,
                duration,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if affected != what || mods.as_slice() != [Modification::DoesntUntap] {
            return None;
        }
        match duration {
            Duration::WhileYouControlSource if *what == Ref::Target(0) => {
                let tail = C10_UC_CONTROL_TAILS[usize::from(!self.at_most_one(what))];
                let tapped = self.effect(tap)?;
                Some((format!("{tapped}. {tail}"), 2))
            }
            Duration::NextUntapStep if *what == c10_uc_players_creatures() => {
                // The group names the target player.
                self.named = true;
                Some((C10_UC_SLEEP.to_owned(), 2))
            }
            _ => None,
        }
    }
}
