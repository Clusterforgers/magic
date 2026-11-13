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
mod aura_additive;
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
mod ce29_arrest;
mod ce29_charm;
mod champion;
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
mod d14_wordings;
mod damage;
mod day_night;
mod defining;
mod dk1a_attached;
mod dk1c5_rooms;
mod dk1d_by_paying;
mod dk27_animate;
mod dk27_batches;
mod dk27_bid;
mod dk27_numbers;
mod dk27_pairs;
mod draws;
mod e10a_targets;
mod e10c_granted;
mod e14b_entering;
mod e14d_sparing;
mod e15a_max_speed;
mod e15b_entering;
mod e15f_words;
mod e15g_licid;
mod e16a_history;
mod e16b_presence;
mod e16c_attacks;
mod e17b_exile_until;
mod e17b_returned;
mod e18a_first;
mod effects;
mod either_way_pairs;
mod ep21_plus;
mod ep22_everywhere;
mod ep22_pairs;
mod ep23_three;
mod ep24_counters;
mod ep24_retarget;
mod ep24_rules;
mod ep26_mana;
mod ep27_flash;
mod ep28_under;
mod ep29_flash;
mod ep30_paid;
mod ep32_discard;
mod ep33_face_down;
mod ep33_spend;
mod equip_options;
mod evasion;
mod event_wordings;
mod exiled_kind;
mod facts;
mod fix16_kicked;
mod fix23_dice;
mod fix32_animate;
mod fix32_pairs;
mod flicker;
mod g10c_facts;
mod g10c_wordings;
mod g10d_spell_costs;
mod g11c_taxes;
mod g12b_copy_kinds;
mod g12b_counters_of;
mod g12f_taxes;
mod g13a_wordings;
mod g13d_attacked;
mod g13g_activation_costs;
mod g8e_attached;
mod g9a_blocks;
mod g9c_blocks;
mod group_copies;
mod group_phrases;
mod group_pump;
mod instead_pairs;
mod joined;
mod keyword_actions;
mod kicked_copies;
mod l290_writers;
mod l291_dice;
mod l291_nth_counter;
mod l292_hand_reads;
mod l292_haunt;
mod l292_twins;
mod l293_events;
mod l294_activations;
mod l294_damage_facts;
mod l31a_events;
pub(crate) mod l31a_performed;
mod l31b_any_amount;
mod l31d_any_number;
mod l31d_x_color;
mod l32a_paid;
mod land_types;
mod lands;
mod leave_combat;
mod levels;
mod library;
mod life_gain;
mod lost_abilities;
mod mana_trigger;
mod manifest;
mod mass_moves;
mod modal;
mod outside;
mod pairs;
mod pay_phrases;
mod payments;
mod player_keywords;
mod prepare;
mod prevention;
mod pronouns;
mod r11w_tallies;
mod r281_unless;
mod r283_atoms;
mod r284_pairs;
mod r284_rules;
mod r285_writers;
mod r287_facts;
mod r289_writers;
mod r300b_writers;
mod r300c_cost_pairs;
mod r300c_writers;
mod r301b_atoms;
mod r301b_writers;
mod r302c_writers;
mod r303_forms;
mod r303b_statics;
mod r303b_writers;
mod r303c_writers;
mod r304_as_enters;
mod r304_damage;
mod r304b_writers;
mod r305b_writers;
mod r305c_copies;
mod r307c_events;
mod r371_copy_except;
mod r371_owner;
mod r372_suspend;
mod r374_champion;
mod r374_counts;
mod r375_events;
mod r375_phase;
mod r376_nearest;
mod r376_seats;
mod r376_secret;
mod r379_atoms;
mod r401_flash;
mod r401_riders;
mod r5a_order;
mod r5a_wordings;
mod r5b_performed;
mod r5c_order;
mod r5g_lines;
mod r6a_effects;
mod r6a_more;
mod r6a_wordings;
mod r6b_pairs;
mod r6b_wordings;
mod r6c_pairs;
mod r6c_wordings;
mod r6d_done;
mod r6d_facts;
mod r6d_heads;
mod r6d_kind;
mod r6f_flash_sacrifice;
mod r7a_more;
mod r7a_wordings;
mod r7b_more;
mod r7b_wordings;
mod r7c_when;
mod r7d_statics;
mod r7e_events;
mod r9w_cant;
mod r9w_players;
mod random_moves;
mod render;
mod repeat;
mod replacements;
mod restrictions;
mod reveal;
mod reveal_until;
mod rmt0_chroma;
mod rmt0_hand_pick;
mod rmt0_kinds;
mod rmt0_x_is;
mod rmt4_untap_limit;
mod rmt4_wordings;
pub(crate) mod rmt7_atoms;
mod rmt8_conditions;
mod selectors;
mod shortened;
mod soulbond;
mod spell_keywords;
mod spend;
mod spent;
mod static_pairs;
mod static_parts;
mod statics;
mod stations;
pub(super) mod subtypes;
mod t15a_alt_costs;
mod t17b_attached;
mod t18_binds;
mod t18_dead;
mod t18_demorgan;
mod t18_words;
mod target_phrases;
mod tc34_control;
#[cfg(test)]
mod tests;
mod token_binds;
mod tokens;
mod transform;
mod triggers;
mod turns;
mod un33_attractions;
mod un34_stickers;
mod untap_rules;
mod us38_quotes;
mod vote;
mod vp20_atoms;
mod w11a_joins;
mod w11b_phase;
mod w11c_clash;
mod w13b_mill_rad;
mod w14_trailing_if;
mod w14a_limits;
mod w14a_wanderer;
mod w14b_or_spell;
mod w14c_alt_costs;
mod w14c_block_capacity;
mod w15_wordings;
mod w15b_time;
mod w15c_clash;
mod w16_wordings;
mod w16b_labels;
mod w16c_lists;
mod w16d_forms;
mod w16e_sequence;
mod w17_forms;
mod w17_wordings;
mod w17b_statics;
mod w17c_cast_only;
mod w18_forms;
mod w19a_flash;
mod w19d_suffixes;
mod w19r_stutter;
mod w20a_look_pick;
mod w20b_reflexive;
mod w20c_counter;
mod w20c_owners;
mod w20c_taken;
mod w21_spells;
mod w21a_atoms;
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
mod wf24_token_bind;
mod wf25_writers;
mod wf26_writers;
mod wf27_writers;
mod where_x;
mod written_memo;
mod zone_moves;

pub use champion::champion_line;
pub use chance::{add_row as add_die_row, die_row, ends_in_roll, row_texts as die_row_texts};
pub use chapters::block as chapter_block;
pub use class::block as class_block;
pub use costs::text as render_cost;
pub use e15f_words::block as word_block;
pub use either_way_pairs::texts as either_way_pair_texts;
pub use ep23_three::ep23_texts as ep23_three_texts;
pub use equip_options::equip_line;
pub use group_phrases::{all_groups, subject_group};
pub use levels::block as level_block;
pub use levels::{Plan as BlockPlan, given as keyword_grant, plan as block_plan, without_header};
pub use r5g_lines::{escalate_cost, escalate_reads, forecast_lines, reinforce_line};
pub use r303_forms::form_text;
pub use r304b_writers::r304b_triple_texts;
pub use render::{
    ability as render_ability, mode as render_mode, variants as render_variants, written_variants,
};
pub use static_pairs::texts as static_pair_texts;
pub use stations::block as station_block;
pub use target_phrases::target_phrases;
pub use tokens::{same_text, same_written_text};
pub use un33_attractions::{is_visit_line, un33_visit_block as visit_block};

use super::{ClauseKind, classify};
use crate::ability::Ability;
use crate::cost::Cost;
use crate::refs::{Pred, Selector, ZoneKind};
use tokens::{Reader, tokenize};

/// Whether a cost modifier counts the targets of its spell ("for each target beyond the first"), so that it needs the
/// spell ability beside it.
pub fn counts_targets(per: &crate::value::Amount) -> bool {
    statics::counts_targets(per)
}

/// The event of a triggered clause the table plays by hand ("Whenever a creature dies, ..."), when the words before its
/// first comma read as one; `names` are the face's own names, which the clause reads as `~`. A clause of several lines
/// waits for no one event.
pub fn by_hand_event(text: &str, names: &[String]) -> Option<crate::trigger::TriggerPattern> {
    let normalized = super::normalize::normalize(text, names);
    let lines = normalized.lines();
    let [line] = lines.as_slice() else {
        return None;
    };
    let line = normalized.slice(line.clone());
    if classify::classify(&line, false) != ClauseKind::Triggered {
        return None;
    }
    let line = line.get(classify::ability_word_len(&line).unwrap_or(0)..)?;
    let tokens = tokenize(line);
    let mut reader = Reader::new(&tokens, false);
    let when = triggers::pattern(&mut reader)?;
    reader.punct(',').then_some(when)
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
    // "Enchant creature card in a graveyard" (Animate Dead): the kind of card, in a graveyard.
    if let Some(kind) = text.strip_suffix(" card in a graveyard") {
        let mut selector =
            enchant_filter(kind).filter(|found| matches!(found.0.as_slice(), [Pred::Type(_)]))?;
        selector.0.push(Pred::In(ZoneKind::Graveyard));
        return Some(selector);
    }
    // An article in front keeps a subtype at the start from being read as the capital of a sentence.
    let spelled = format!("a {}", text.replace(", or ", " or ").replace(", ", " or "));
    let tokens = tokenize(&spelled);
    let mut reader = Reader::new(&tokens, false);
    reader.word("a").then_some(())?;
    let selector = selectors::phrase(&mut reader, selectors::Form::Target);
    match selector {
        Some(selector) if reader.end() => Some(selector),
        _ => wf26_enchant_either(text),
    }
}

/// "Enchant creature or Vehicle": two kinds the phrase reader does not join, each read alone (a type and a subtype).
fn wf26_enchant_either(text: &str) -> Option<Selector> {
    let (first, second) = text.split_once(" or ")?;
    let mut either = Vec::new();
    for part in [first, second] {
        if part.contains([',', ' ']) {
            return None;
        }
        let tokens = tokenize(&format!("a {part}"));
        let mut reader = Reader::new(&tokens, false);
        reader.word("a").then_some(())?;
        let Selector(preds) = selectors::phrase(&mut reader, selectors::Form::Target)?;
        let [pred] = <[Pred; 1]>::try_from(preds).ok()?;
        reader.end().then_some(())?;
        either.push(pred);
    }
    Some(Selector(vec![Pred::AnyOf(either)]))
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
    let found = if let Some(found) = dk27_pairs::dk27_any_number_less(text) {
        found
    } else if let Some(found) = ce29_arrest::ce29_arrest(text, x_free, own) {
        found
    } else {
        let found = static_pairs::read(&mut reader)?;
        if !reader.end() {
            return None;
        }
        found
    };
    if let [first, second, third] = found.as_slice() {
        return r371_owner::r371_three_texts(first, second, third)
            .iter()
            .any(|written| same_text(written, text))
            .then_some(found);
    }
    let [first, second] = found.as_slice() else {
        return None;
    };
    let said = ce29_arrest::ce29_said(text);
    static_pairs::texts(first, second)
        .iter()
        .any(|written| same_text(written, &said))
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
#[allow(clippy::too_many_lines)]
pub fn clause_with(text: &str, kind: ClauseKind, x_free: bool, own: &Own) -> Option<Ability> {
    if kind == ClauseKind::Spell
        && let Some(joined) = rmt0_x_is::rmt0_joined(text)
    {
        let ability = clause_with(&joined, kind, x_free, own)?;
        return render::variants(&ability)
            .iter()
            .any(|written| same_text(written, text))
            .then_some(ability);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) = dk27_animate::dk27_animate_read(text)
    {
        return Some(found);
    }
    if let Some(found) = l31d_any_number::l31d_any_number_read(text, kind, |twin, kind, x_free| {
        clause_with(twin, kind, x_free, own)
    }) && written_variants(&found)
        .iter()
        .any(|written| same_text(written, text))
    {
        return Some(found);
    }
    if let Some(found) =
        l31d_x_color::l31d_x_color_read(text, kind, |head| clause_with(head, kind, x_free, own))
        && written_variants(&found)
            .iter()
            .any(|written| same_text(written, text))
    {
        return Some(found);
    }
    if kind == ClauseKind::Static
        && let Some(found) = r375_phase::r375_no_phase_in_read(text)
    {
        return Some(found);
    }
    if let Some(found) =
        r375_phase::r375_held_read(text, |twin| clause_with(twin, kind, x_free, own))
        && written_variants(&found)
            .iter()
            .any(|written| same_text(written, text))
    {
        return Some(found);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) =
            r375_events::r375_crew_read(text, |twin| clause_with(twin, kind, x_free, own))
        && written_variants(&found)
            .iter()
            .any(|written| same_text(written, text))
    {
        return Some(found);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) =
            l32a_paid::l32a_times_read(text, |twin| clause_with(twin, kind, x_free, own))
                .or_else(|| {
                    l32a_paid::l32a_still_read(text, |twin| clause_with(twin, kind, x_free, own))
                })
                .or_else(|| {
                    l32a_paid::l32a_reattack_read(text, |twin| clause_with(twin, kind, x_free, own))
                })
                .or_else(|| {
                    l32a_paid::l32a_exchange_read(text, |twin| clause_with(twin, kind, x_free, own))
                })
        && written_variants(&found)
            .iter()
            .any(|written| same_text(written, text))
    {
        return Some(found);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) = r372_suspend::r372_not_from_exile_read(text, |twin| {
            clause_with(twin, kind, x_free, own)
        })
    {
        return Some(found);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) = r304b_writers::r304b_tapped_read(text)
    {
        return Some(found);
    }
    if kind == ClauseKind::Triggered
        && let Some(found) =
            l293_events::l293_won_read(text, |twin| clause_with(twin, kind, x_free, own))
        && written_variants(&found)
            .iter()
            .any(|written| same_text(written, text))
    {
        return Some(found);
    }
    if let Some(granted) = text.strip_prefix("Max speed \u{2014} ") {
        let inner = classify::classify(granted, false);
        if kind != ClauseKind::Static {
            return None;
        }
        return clause_with(granted, inner, x_free, own)
            .or_else(|| {
                e15a_max_speed::e15a_read_body(granted, |body| {
                    clause_with(body, inner, x_free, own)
                })
            })
            .map(e15a_max_speed::e15a_wrap);
    }
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
