//! Stage 6: the readers. A template is a small function over a clause's tokens that reads the whole
//! clause into rules data or nothing, and the renderer writes what was read back as oracle text.
//!
//! A clause is read only when the template consumes all of it and the rendering of the result is
//! the clause's own text (whitespace, a closing period, "this creature" for `~` and ", then" for a
//! new sentence aside), so a reading that dropped or changed a word stays with the table. One file
//! per part: `tokens` (the words, numbers and symbols of a clause and the reader that walks them),
//! `selectors` (what a clause targets or counts), `subtypes` (the capitalized words that are
//! types, not names), `amounts`, `effects` (a sentence is an effect), `library` (searching, looking
//! at and shuffling a library), `counters` (putting, removing and moving them), `modal` (the modes
//! of a choice), `triggers` (the event of a triggered ability), `costs` (what is paid before the
//! colon), `abilities` (a spell ability, a triggered ability, an activated ability) and `render`.

mod abilities;
mod amounts;
mod costs;
mod counters;
mod create;
mod effects;
mod library;
mod modal;
mod render;
mod selectors;
mod subtypes;
#[cfg(test)]
mod tests;
mod tokens;
mod triggers;

pub use render::{ability as render_ability, variants as render_variants};
pub use tokens::same_text;

use super::{ClauseKind, classify};
use crate::ability::Ability;
use tokens::{Reader, tokenize};

/// Reads a spell, a modal, a triggered or an activated clause into an ability, or `None` when it
/// is not a wording the templates know. The text of a spell may run over several lines, which are
/// its sentences in order; a modal's lines are its bullets. An ability word before a dash
/// ("Landfall — ") is flavor and left out.
/// `x_free` says the card's mana cost has an `{X}`, which an amount may use.
pub fn clause(text: &str, kind: ClauseKind, x_free: bool) -> Option<Ability> {
    let text = match kind {
        ClauseKind::Triggered => {
            let head = text.lines().next().unwrap_or_default();
            text.get(classify::ability_word_len(head).unwrap_or(0)..)?
        }
        _ => text,
    };
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, x_free);
    let ability = match kind {
        ClauseKind::Spell => abilities::spell(&mut reader)?,
        ClauseKind::Modal => abilities::modal_spell(&mut reader)?,
        ClauseKind::Triggered => abilities::triggered(&mut reader)?,
        ClauseKind::Activated => abilities::activated(&mut reader)?,
        _ => return None,
    };
    if !reader.end() {
        return None;
    }
    render::variants(&ability)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(ability)
}
