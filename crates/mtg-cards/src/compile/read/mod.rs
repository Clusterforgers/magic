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
//! `damage` (what deals damage to whom and the life gained with it), `copies` (a token that is a copy, populate, entering as a copy), `levels` (the blocks of a level up creature), `chapters` (the lines of a Saga), `class` (the levels of a Class), `stations` (the blocks of a Spacecraft), `copy_spells` (a copy of a spell or an ability, a
//! permanent that becomes a copy), `defining` (the power and toughness a card defines for itself), `life_gain` (replacements and rules about gaining life), `transform`, `draws` (draw and discard wordings), `actions` (the keyword actions of creatures),
//! `pronouns` (the wordings that call the card "it"), `where_x` (an amount named after the sentence: "where X is its power"), `zone_moves` (cards returned from a graveyard, a graveyard exiled, a card put from the hand), `render`, and the writers that no template reads: `replacements`, `batches`, `choices`, `static_parts` and `activations`.

mod abilities;
mod actions;
mod activations;
mod among;
mod amounts;
mod as_long_as;
mod attack_pairs;
mod attack_tax;
mod attackers;
mod aura_forms;
mod batches;
mod becomes;
mod c10_dice_targets;
mod c10_pa_attacks;
mod c10_pa_monstrous;
mod c10_pa_pay_x;
mod c10_uc;
mod c11a_ability_less;
mod c11a_turn_costs;
mod c11b_cast_target;
mod c11b_once;
mod c11c_had_counters;
mod c11h_attached;
mod c11h_prevent;
mod c11h_rules;
mod c4_cast_hand;
mod c4_chosen_pairs;
mod c4_hands;
mod c4_joined_events;
mod c4_phase;
mod c4_rules;
mod c6_au_dead;
mod c6_blk_aura;
mod c6_blk_draws;
mod c7_untap;
mod c8_eg_cast_itself;
mod c8_kwq_abilities;
mod c9_fight_double;
mod card_grants;
mod cast;
mod chance;
mod changes;
mod chapters;
mod choices;
mod class;
mod conditional;
mod copies;
mod copy_spells;
mod cost_conditions;
mod costs;
mod counters;
mod create;
mod damage;
mod day_night;
mod defining;
mod draws;
mod effects;
mod evasion;
mod event_wordings;
mod exiled_kind;
mod facts;
mod flicker;
mod group_copies;
mod group_pump;
mod instead_pairs;
mod joined;
mod kicked_copies;
mod lands;
mod levels;
mod library;
mod life_gain;
mod mana_trigger;
mod manifest;
mod mass_moves;
mod modal;
mod pairs;
mod pay_phrases;
mod payments;
mod player_keywords;
mod prepare;
mod prevention;
mod pronouns;
mod random_moves;
mod render;
mod repeat;
mod replacements;
mod restrictions;
mod reveal;
mod reveal_until;
mod selectors;
mod shortened;
mod spell_keywords;
mod spend;
mod spent;
mod static_pairs;
mod static_parts;
mod statics;
mod stations;
pub(super) mod subtypes;
#[cfg(test)]
mod tests;
mod token_binds;
mod tokens;
mod transform;
mod triggers;
mod turns;
mod untap_rules;
mod vote;
mod w11a_joins;
mod w11b_phase;
mod w11c_clash;
mod w13b_mill_rad;
mod w14a_limits;
mod w14a_wanderer;
mod w14b_or_spell;
mod w14c_alt_costs;
mod w14c_block_capacity;
mod w15b_time;
mod w15c_clash;
mod w20c_counter;
mod w20c_owners;
mod w20c_taken;
mod w2b_you_and;
mod w2c_counts;
mod w2d_villainous;
mod w2e_copied;
mod w2f_any_counters;
mod w2g_next_cast;
mod w5d_replaced;
mod w6a_attacks_you;
mod w6b_emblem;
mod w6d_shares;
mod w6e_control;
mod w7b_player_counters;
mod w7e_phase;
mod w8a_declaration_limits;
mod w8b_milled;
mod w8b_then;
mod w9b_clash;
mod w9c_replaced;
mod w9d_exert;
mod where_x;
mod zone_moves;

pub use chance::{add_row as add_die_row, die_row, ends_in_roll, row_texts as die_row_texts};
pub use chapters::block as chapter_block;
pub use class::block as class_block;
pub use levels::block as level_block;
pub use levels::{Plan as BlockPlan, given as keyword_grant, plan as block_plan, without_header};
pub use render::{
    ability as render_ability, mode as render_mode, variants as render_variants, written_variants,
};
pub use static_pairs::texts as static_pair_texts;
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

/// Reads a spell, a modal, a triggered, an activated or a loyalty clause into an ability, or `None` when it
/// is not a wording the templates know. The text of a spell may run over several lines, which are
/// its sentences in order; a modal's lines are its bullets. An ability word before a dash
/// ("Landfall — ") is flavor and left out.
/// `x_free` says the card's mana cost has an `{X}`, which an amount may use.
#[cfg(test)]
pub fn clause(text: &str, kind: ClauseKind, x_free: bool) -> Option<Ability> {
    clause_with(text, kind, x_free, &Own::default())
}

/// Reads a static clause that is two statics in one sentence after one condition ("As long as you control your
/// commander, this creature gets +2/+2 and creatures you control have vigilance.") into the two abilities in order. An
/// ability word before a dash is flavor and left out.
pub fn static_pair(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    let head = text.lines().next().unwrap_or_default();
    let text = text.get(classify::ability_word_len(head).unwrap_or(0)..)?;
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, x_free);
    reader.subtypes.clone_from(&own.subtypes);
    let found = static_pairs::read(&mut reader)?;
    if !reader.end() {
        return None;
    }
    let [first, second] = found.as_slice() else {
        return None;
    };
    static_pairs::texts(first, second)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// A line of the table of a die roll ("1—9 | Each player sacrifices a permanent of their choice.") as a row: its totals and
/// the spell that follows the bar, which names no target.
pub fn die_row_of(text: &str, x_free: bool, own: &Own) -> Option<crate::effect::DieRow> {
    let (from, to, body) = chance::die_row(text)?;
    let Ability::Spell { targets, effects } = clause_with(body, ClauseKind::Spell, x_free, own)?
    else {
        return None;
    };
    targets
        .is_empty()
        .then_some(crate::effect::DieRow { from, to, effects })
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
    /// Whether the spell must pay an additional cost that takes objects out of their zones ("As an additional cost to
    /// cast this spell, sacrifice a creature."), which its effects read as "the sacrificed creature's power".
    pub pays: bool,
    /// Whether the face has an `{X}` in its cycling cost ("Cycling {X}{1}{U}"), which "when you cycle this card" reads.
    pub w6c_cost_x: bool,
}

/// The words before an activated ability that are rules, not flavor: the ability works from another zone (a hand, an
/// exile) or only after something happened ("Boast \u{2014} {1}: ...").
const MEANINGFUL_WORDS: [&str; 5] = [
    "Channel \u{2014} ",
    "Bloodrush \u{2014} ",
    "Boast \u{2014} ",
    "Exhaust \u{2014} ",
    "Forecast \u{2014} ",
];

/// Reads a clause like `clause`, with what the face itself says (`Own`).
pub fn clause_with(text: &str, kind: ClauseKind, x_free: bool, own: &Own) -> Option<Ability> {
    let text = match kind {
        // An ability word is flavor: "Landfall — ", "Strive — ".
        ClauseKind::Triggered | ClauseKind::Static => {
            let head = text.lines().next().unwrap_or_default();
            text.get(classify::ability_word_len(head).unwrap_or(0)..)?
        }
        // An ability word is flavor ("Ferocious — ", "Devouring Monster — ") except the words that name a zone the
        // ability works from or a limit on it.
        ClauseKind::Activated => {
            let head = text.lines().next().unwrap_or_default();
            let word = (!MEANINGFUL_WORDS.iter().any(|word| head.starts_with(word)))
                .then(|| classify::ability_word_len(head))
                .flatten();
            text.get(word.unwrap_or(0)..)?
        }
        _ => text,
    };
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, x_free);
    reader.subtypes.clone_from(&own.subtypes);
    reader.ability = matches!(
        kind,
        ClauseKind::Triggered | ClauseKind::Activated | ClauseKind::Loyalty
    );
    reader.power = own.power;
    reader.toughness = own.toughness;
    reader.paid = own.pays;
    reader.w6c_cost_x = own.w6c_cost_x;
    let ability = match kind {
        ClauseKind::Spell => abilities::spell(&mut reader)?,
        ClauseKind::Modal => abilities::modal_spell(&mut reader)?,
        ClauseKind::Triggered => abilities::triggered(&mut reader)?,
        ClauseKind::Activated => abilities::activated(&mut reader)?,
        ClauseKind::Loyalty => abilities::loyalty(&mut reader)?,
        ClauseKind::Static => statics::ability(&mut reader)?,
        ClauseKind::Keyword => return None,
    };
    if !reader.end() {
        return None;
    }
    render::variants(&ability)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(ability)
}
