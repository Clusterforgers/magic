//! Stage 6: the readers. A template is a small function over a clause's tokens that reads the whole
//! clause into rules data or nothing, and the renderer writes what was read back as oracle text.
//!
//! A clause is read only when the template consumes all of it and the rendering of the result is
//! the clause's own text (whitespace, a closing period, "this creature" for `~` and ", then" for a
//! new sentence aside), so a reading that dropped or changed a word stays with the table. One file
//! per part: `tokens` (the words, numbers and symbols of a clause and the reader that walks them),
//! `selectors` (what a clause targets or counts), `subtypes` (the capitalized words that are
//! types, not names), `amounts`, `effects` (a sentence is an effect), `library` (searching, looking
//! at and shuffling a library), `counters` (putting, removing and moving them), `statics` (how a
//! permanent enters, what it can't do, what it gives), `modal` (the modes
//! of a choice), `triggers` (the event of a triggered ability), `costs` (what is paid before the
//! colon), `abilities` (a spell ability, a triggered ability, an activated ability), `conditional`
//! (an effect that depends on a condition), `reveal` (looking at the top card and revealing it),
//! `damage` (what deals damage to whom and the life gained with it), `copies` (a token that is a copy, populate, entering as a copy), `levels` (the blocks of a level up creature), `chapters` (the lines of a Saga), `stations` (the blocks of a Spacecraft), `copy_spells` (a copy of a spell or an ability, a
//! permanent that becomes a copy), `defining` (the power and toughness a card defines for itself), `life_gain` (replacements and rules about gaining life), `transform`, `draws` (draw and discard wordings), `actions` (the keyword actions of creatures),
//! `pronouns` (the wordings that call the card "it"), `where_x` (an amount named after the sentence: "where X is its power") and `render`.

mod abilities;
mod actions;
mod among;
mod amounts;
mod attack_tax;
mod attackers;
mod becomes;
mod cast;
mod chapters;
mod conditional;
mod copies;
mod copy_spells;
mod costs;
mod counters;
mod create;
mod damage;
mod defining;
mod draws;
mod effects;
mod flicker;
mod group_pump;
mod lands;
mod levels;
mod library;
mod life_gain;
mod modal;
mod pairs;
mod payments;
mod prevention;
mod pronouns;
mod render;
mod reveal;
mod selectors;
mod spend;
mod statics;
mod stations;
mod subtypes;
#[cfg(test)]
mod tests;
mod tokens;
mod transform;
mod triggers;
mod where_x;

pub use chapters::block as chapter_block;
pub use levels::block as level_block;
pub use render::{ability as render_ability, mode as render_mode, variants as render_variants};
pub use stations::block as station_block;
pub use tokens::same_text;

use super::{ClauseKind, classify};
use crate::ability::Ability;
use crate::cost::Cost;
use crate::refs::Selector;
use tokens::{Reader, tokenize};

/// Whether a cost modifier counts the targets of its spell ("for each target beyond the first"), so that it needs the
/// spell ability beside it.
pub fn counts_targets(per: &crate::value::Amount) -> bool {
    statics::counts_targets(per)
}

/// The cost a keyword of a spell prints in words ("Sacrifice a creature", "{1}{U}, Pay 3 life"), when all of the text
/// reads as the parts of one cost.
pub fn cost_in_words(text: &str) -> Option<Cost> {
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, false);
    let cost = costs::cost(&mut reader)?;
    reader.end().then_some(cost)
}

/// What an Aura may enchant, from the words after "Enchant": the objects a phrase names ("creature you control",
/// "nonland permanent", "artifact or land", "creature, land, or planeswalker", "Forest"), when all of the text reads.
pub fn enchant_filter(text: &str) -> Option<Selector> {
    // An article in front keeps a subtype at the start from being read as the capital of a sentence.
    let spelled = format!("a {}", text.replace(", or ", " or ").replace(", ", " or "));
    let tokens = tokenize(&spelled);
    let mut reader = Reader::new(&tokens, false);
    reader.word("a").then_some(())?;
    let selector = selectors::phrase(&mut reader, selectors::Form::Target)?;
    reader.end().then_some(selector)
}

/// What an affinity counts, from the words after "Affinity for" ("artifacts", "artifact creatures", "Equipment",
/// "Islands", "tokens"), when all of the text reads.
pub fn affinity_filter(text: &str) -> Option<Selector> {
    let tokens = tokenize(&format!("all {text}"));
    let mut reader = Reader::new(&tokens, false);
    reader.word("all").then_some(())?;
    let selector = selectors::phrase(&mut reader, selectors::Form::All)?;
    reader.end().then_some(selector)
}

/// Reads a spell, a modal, a triggered or an activated clause into an ability, or `None` when it
/// is not a wording the templates know. The text of a spell may run over several lines, which are
/// its sentences in order; a modal's lines are its bullets. An ability word before a dash
/// ("Landfall — ") is flavor and left out.
/// `x_free` says the card's mana cost has an `{X}`, which an amount may use.
#[cfg(test)]
pub fn clause(text: &str, kind: ClauseKind, x_free: bool) -> Option<Ability> {
    clause_with(text, kind, x_free, &Own::default())
}

/// What the readers know of the face a clause is on besides its text.
#[derive(Debug, Clone, Default)]
pub struct Own {
    /// The subtypes on the type line: a word among them is a subtype (in the singular or the plural) even where the
    /// rules' list does not have it.
    pub subtypes: Vec<String>,
    /// The printed power when it is a number.
    pub power: Option<i32>,
    /// The printed toughness when it is a number.
    pub toughness: Option<i32>,
    /// The charge counters at which a Spacecraft with a printed power becomes a creature: its highest station threshold.
    pub creature_at: Option<i32>,
}

/// Reads a clause like `clause`, with what the face itself says (`Own`).
pub fn clause_with(text: &str, kind: ClauseKind, x_free: bool, own: &Own) -> Option<Ability> {
    let text = match kind {
        // An ability word is flavor: "Landfall — ", "Strive — ".
        ClauseKind::Triggered | ClauseKind::Static => {
            let head = text.lines().next().unwrap_or_default();
            text.get(classify::ability_word_len(head).unwrap_or(0)..)?
        }
        _ => text,
    };
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, x_free);
    reader.subtypes.clone_from(&own.subtypes);
    reader.power = own.power;
    reader.toughness = own.toughness;
    let ability = match kind {
        ClauseKind::Spell => abilities::spell(&mut reader)?,
        ClauseKind::Modal => abilities::modal_spell(&mut reader)?,
        ClauseKind::Triggered => abilities::triggered(&mut reader)?,
        ClauseKind::Activated => abilities::activated(&mut reader)?,
        ClauseKind::Static => statics::ability(&mut reader)?,
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
