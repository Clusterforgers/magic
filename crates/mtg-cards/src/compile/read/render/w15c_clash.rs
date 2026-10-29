//! The sentence of the clash wording of `w15c_clash`: a clash with the defending player.

use super::super::w15c_clash::{W15C_DEFENDING, w15c_defending_clash};
use super::Effect;

/// The clash with the defending player that opens the effects, as a sentence, and the one effect it covers.
pub(super) fn w15c_clash(effects: &[Effect]) -> Option<(Vec<String>, usize)> {
    (*effects.first()? == w15c_defending_clash()).then(|| (vec![W15C_DEFENDING.to_owned()], 1))
}
