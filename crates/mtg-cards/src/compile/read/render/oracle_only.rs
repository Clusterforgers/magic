//! Wordings the oracle text uses for an ability that the renderers write another way, each derived from the wordings of
//! [`super::variants`] by the data of the ability. The compiler's own reads stay on `variants`; only the proposal
//! checker accepts these (`written_variants`).

mod af2;
mod all_non;
mod also_create;
mod and_create;
mod and_draw;
mod and_or;
mod any_token_maker;
mod are_and_or;
mod as_it_enters;
mod at_least;
mod attach_it;
mod attach_to_it;
mod attack_player;
mod bare;
mod battalion;
mod blink;
mod bounce_all;
mod c10_cen_land_look;
mod c10_cen_power_damage;
mod c11f_wholes;
mod c11g_wholes;
mod c4_af;
mod c4_attack_token;
mod c4_aura_becomes;
mod c4_chosen_damage;
mod c4_counters_you_put;
mod c4_delayed_prepare;
mod c4_each_of_up_to;
mod c4_exile_copy;
mod c4_group_effects;
mod c4_moves;
mod c4_mr;
mod c4_party_roles;
mod c4_phases;
mod c4_prevent_combat;
mod c4_support;
mod c4_swaps;
mod c4_swaps_sz;
mod c4_sz;
mod c4_those_creatures;
mod c4_token_counts;
mod c4_twins;
mod c5_af;
mod c5_dice_piles;
mod c5_gl_modes;
mod c5_gl_swaps;
mod c5_gl_twins;
mod c5_mr_writers;
mod c5_sz_sentences;
mod c5_sz_wholes;
mod c6_af;
mod c6_eg;
mod c6_gap_wholes;
mod c6_gl_causes;
mod c6_gl_returns;
mod c6_gl_wholes;
mod c6_mr_writers;
mod c6_sz_and_you;
mod c6_sz_bodies;
mod c6_sz_wholes;
mod c9_wr_returns;
mod c9_wr_wholes;
mod cant_countered;
mod cant_unless;
mod cast_condition;
mod cast_free;
mod cast_from_hand;
mod cast_graveyard;
mod channel_discard;
mod choice_keywords;
mod choose_first;
mod choose_targets;
mod chosen_color_cast;
mod color_lists;
mod commander_draw;
mod conditional_rule;
mod conditions;
mod controller_creates;
mod controller_damage;
mod controller_draws;
mod controller_may;
mod converge;
mod copy_quotes;
mod counter_ability;
mod counter_any;
mod counter_pair;
mod counter_sacrifice;
mod counter_you_draw;
mod counters_equal;
mod counters_put;
mod creature_enters;
mod cycle_this;
mod damage_back;
mod damage_each_and;
mod damage_instead;
mod dealt_damage;
mod dealt_damage_sz;
mod defending_mills;
mod delayed_first;
mod destroy_all;
mod destroy_and;
mod destroy_targets;
mod destroy_that;
mod destroy_two;
mod destroyed_this_way;
mod died_each;
mod discard_play;
mod discarded_was;
mod do_this_once;
mod double_counters;
mod double_self;
mod doubled;
mod draw_equal;
mod each_and_each;
mod each_draw;
mod each_draw_sz2;
mod each_of;
mod each_sacrifice;
mod each_sweep;
mod each_to_owner;
mod either_event;
mod energy_amount;
mod energy_cost_words;
mod energy_each;
mod energy_words;
mod entered_under;
mod enters_and_step;
mod entry_counters;
mod entry_sum;
mod except_for;
mod except_lands_tokens;
mod exile_each;
mod exile_play;
mod exiled_from;
mod ferocious;
mod first_main;
mod flash_casts;
mod flash_permission;
mod flash_rule;
mod for_each_card;
mod goaded;
mod graveyard_cards;
mod graveyard_shuffle;
mod group_verbs;
mod half_tokens;
mod have_cards;
mod have_deal;
mod home_pair;
mod itself;
mod joined_steps;
mod kicked;
mod kicked_enters;
mod kicked_trigger;
mod land_subtypes;
mod land_types;
mod leaves_graveyard;
mod life_counters;
mod life_over_starting;
mod look_put;
mod look_split;
mod lowercase_until;
mod may_have_player;
mod minus_amount;
mod no_max_hand;
mod non_lists;
mod nonlegendary;
mod of_their_choice;
mod one_or_more_attack;
mod other_players;
mod other_than;
mod own_permanent;
mod owner_shuffles;
mod owners_graveyards;
mod paid_return;
mod paid_steps;
mod paired_returns;
mod pay_two_costs;
mod per_spell_cast;
mod performed_steps;
mod plus_one_counters;
mod poison_pump;
mod prevent_remove;
mod pump_and_unblockable;
mod pump_untap;
mod put_back_order;
mod put_graveyard;
mod put_into;
mod put_into_graveyard;
mod put_that_card;
mod quote_commas;
mod regen_this_way;
mod repeat_modes;
mod return_damager;
mod return_this_card;
mod sacrifice_it;
mod sacrifice_search;
mod sacrifice_unless;
mod sacrifices;
mod search_put;
mod search_up_to_x;
mod shield_group;
mod sized_condition;
mod spell_controller;
mod spell_value;
mod spells_cast_damage;
mod standin;
mod swaps;
mod t15a_rewordings;
mod t15a_three_events;
mod t17b_conditions;
mod t17b_durations;
mod t17b_each;
mod t17b_protection;
mod tail_sentences;
mod target_and;
mod target_costs;
mod target_damage;
mod that_creature;
mod that_creature_sz2;
mod that_many_tokens;
mod that_player;
mod that_spells;
mod those_cards;
mod trailing_if;
mod trigger_again;
mod triggering_pump;
mod twin_json;
mod uncounterable_group;
mod unless;
mod untap_lands;
mod untapped_cost;
mod w12a_exactly;
mod w12a_is_this;
mod w12a_it_self;
mod w12a_minus_zero;
mod w12a_next_upkeep;
mod w12a_that_target;
mod w12a_their_graveyard;
mod w13a_either_quality;
mod w13a_greater;
mod w13a_itself;
mod w13a_named;
mod w13a_only_one;
mod w13a_sacrifice_all;
mod w13a_shuffle_zone;
mod w13a_their_hand;
mod w13a_those;
mod w13a_was_color;
mod w16a_color_or;
mod w16a_comma_then;
mod w16a_damage_counters;
mod w16a_defending;
mod w16a_enchanted_control;
mod w16a_entry_if;
mod w16a_named_yours;
// dk1b
mod dk1b_attached;
mod w16a_owner;
mod w16a_return_cost;
mod w16a_state_trigger;
mod w16a_static_and;
mod w4a_if;
mod w4a_if_conditions;
mod w4a_if_instead;
mod w4a_if_steps;
mod w4b_becomes;
mod w4b_conditions;
mod w4b_control;
mod w4b_durations;
mod w4b_grants;
mod w4b_pumps;
mod w4b_stand;
mod w4b_untap;
mod w4c_bodies;
mod w4c_may_pay;
mod w4d_each;
mod w4d_later;
mod w4d_parts;
mod w4e_counters;
mod w4e_flash;
mod w4e_if;
mod w4e_modes;
mod w4e_moves;
mod w4e_where_x;
mod w4f_counts;
mod w4f_pronouns;
mod w4f_sentences;
mod w4f_tokens;
mod w4g_amounts;
mod w4g_texts;
mod w5a_combat;
mod w5a_conditions;
mod w5a_heads;
mod w5b_casts;
mod w5b_damage;
mod w5b_mana;
mod w5b_parts;
mod w5b_rules;
mod w5c_later;
mod w5c_player_counters;
mod w6d_wholes;
mod w8c_damage_and;
mod w8c_kind_controller;
mod w8c_on_top;
mod w8c_opponent_graveyard;
mod w8c_other_turns;
mod w8c_prevent_next;
mod w8c_return_both;
mod w8c_snow;
mod w8c_spell_colors;
mod w8c_still_kind;
mod w8c_then_discards;
mod w8c_then_tap;
mod wheres;
mod while_control;
mod without_counter;
mod you_create;

// deck-auras
mod deck_auras;

// t19c
mod t19c_either;
mod t19c_put_many;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    and_draw::add(ability, texts);
    and_or::add(texts);
    any_token_maker::add(ability, texts);
    are_and_or::add(ability, texts);
    as_it_enters::add(ability, texts);
    blink::add(ability, texts);
    attach_to_it::add(ability, texts);
    chosen_color_cast::add(ability, texts);
    converge::add(ability, texts);
    defending_mills::add(ability, texts);
    discarded_was::add(ability, texts);
    life_over_starting::add(ability, texts);
    no_max_hand::add(ability, texts);
    paid_return::add(ability, texts);
    per_spell_cast::add(ability, texts);
    pump_and_unblockable::add(ability, texts);
    repeat_modes::add(ability, texts);
    search_up_to_x::add(ability, texts);
    shield_group::add(ability, texts);
    triggering_pump::add(ability, texts);
    cast_condition::add(ability, texts);
    conditions::add(ability, texts);
    controller_creates::add(ability, texts);
    controller_damage::add(ability, texts);
    controller_draws::add(ability, texts);
    cant_unless::add(ability, texts);
    counter_pair::add(ability, texts);
    creature_enters::add(ability, texts);
    dealt_damage::add(ability, texts);
    died_each::add(ability, texts);
    destroy_all::add(ability, texts);
    destroy_targets::add(ability, texts);
    doubled::add(ability, texts);
    double_self::add(ability, texts);
    draw_equal::add(ability, texts);
    each_and_each::add(ability, texts);
    destroy_and::add(ability, texts);
    destroyed_this_way::add(ability, texts);
    also_create::add(ability, texts);
    cast_graveyard::add(ability, texts);
    choose_first::add(ability, texts);
    commander_draw::add(ability, texts);
    controller_may::add(ability, texts);
    copy_quotes::add(ability, texts);
    counter_ability::add(ability, texts);
    counter_any::add(ability, texts);
    counter_you_draw::add(ability, texts);
    counters_equal::add(ability, texts);
    counters_put::add(ability, texts);
    add_more(ability, texts);
    add_c4_af(ability, texts);
    add_c4_mr(ability, texts);
    c4_oracle_only(ability, texts);
    add_c4(ability, texts);
    c4_writers(ability, texts);
    add_c5_dice(ability, texts);
    add_c5_af(ability, texts);
    add_c5_mr(ability, texts);
    add_c5_gl(ability, texts);
    add_c5_sz(ability, texts);
    add_c6_eg(ability, texts);
    add_c6_gap(ability, texts);
    add_c6_af(ability, texts);
    add_c6_sz(ability, texts);
    add_c6_gl(ability, texts);
    add_c6_mr(ability, texts);
    add_c9_wr(ability, texts);
    add_c10_cen(ability, texts);
    add_c11g(ability, texts);
    add_c11f(ability, texts);
    add_w4c(ability, texts);
    add_w4e(ability, texts);
    add_w4a(ability, texts);
    add_w4g(ability, texts);
    add_w4f(texts);
    add_w4d(ability, texts);
    add_w4b(ability, texts);
    add_w5b(ability, texts);
    add_w5c(ability, texts);
    add_w5a(ability, texts);
    add_w8c(ability, texts);
    add_w12a(ability, texts);
    add_w13a(ability, texts);
    add_w16a(ability, texts);
    t15a_three_events::add(ability, texts);
    t15a_rewordings::add(texts);
    add_t17b(ability, texts);
    // dk1b
    dk1b_attached::add(ability, texts);
    // deck-auras
    deck_auras::add(ability, texts);
    // t19c
    t19c_put_many::add(ability, texts);
    t19c_either::add(ability, texts);
}

/// Adds the wordings of continuous effects and static grants the stored local-model proposals hit (t17b): durations of
/// a pump other than the turn, several targets that each change, protection from a group, and the conditions of a static
/// ability about a count, an Equipment or an Aura attached, or the cards drawn.
fn add_t17b(ability: &Ability, texts: &mut Vec<String>) {
    t17b_durations::add(ability, texts);
    t17b_each::add(ability, texts);
    t17b_protection::add(ability, texts);
    t17b_conditions::add(ability, texts);
}

/// Adds the wordings of the shapes the stored local-model proposals hit (r4d-d, r4b-d): an entry with counters under a
/// condition, the defending player who acts, the card returned to its owner's hand as a cost, a group of one of two
/// colors, damage turned into counters, creatures you control with a name, the owner of a target, control of the
/// enchanted permanent, a state trigger, two parts of a static ability joined by "and" and a later step joined by ", then".
fn add_w16a(ability: &Ability, texts: &mut Vec<String>) {
    w16a_entry_if::add(ability, texts);
    w16a_defending::add(ability, texts);
    w16a_return_cost::add(ability, texts);
    w16a_color_or::add(ability, texts);
    w16a_damage_counters::add(ability, texts);
    w16a_named_yours::add(ability, texts);
    w16a_owner::add(ability, texts);
    w16a_enchanted_control::add(ability, texts);
    w16a_state_trigger::add(ability, texts);
    w16a_static_and::add(ability, texts);
    w16a_comma_then::add(texts);
}

/// Adds the wordings of the shapes the stored local-model proposals hit (r4d-b, r4b-b): a count greater than a number,
/// a later step on "those creatures", a count of a target player's hand, a target with one of two qualities, a target
/// that deals damage to itself, a zone shuffled into the library, a target that was one of two colors, a group of
/// exactly one, a sacrifice of a whole group and creatures with a name.
fn add_w13a(ability: &Ability, texts: &mut Vec<String>) {
    w13a_greater::add(ability, texts);
    w13a_those::add(ability, texts);
    w13a_their_hand::add(ability, texts);
    w13a_either_quality::add(ability, texts);
    w13a_itself::add(ability, texts);
    w13a_shuffle_zone::add(ability, texts);
    w13a_was_color::add(ability, texts);
    w13a_only_one::add(ability, texts);
    w13a_sacrifice_all::add(ability, texts);
    w13a_named::add(ability, texts);
}

/// Adds the wordings of the shapes the stored local-model proposals hit (r4d): the next turn's upkeep, a target said
/// again as "it" or "that creature", the source said "it" after its own trigger, the source named through a selector,
/// a target player's exile from their graveyard, a count of exactly a number and a penalty "-N/-0".
fn add_w12a(ability: &Ability, texts: &mut Vec<String>) {
    w12a_is_this::add(ability, texts);
    w12a_that_target::add(ability, texts);
    w12a_their_graveyard::add(ability, texts);
    w12a_exactly::add(ability, texts);
    w12a_next_upkeep::add(ability, texts);
    w12a_it_self::add(ability, texts);
    w12a_minus_zero::add(texts);
}

/// Adds the wordings of the shapes the stored local-model proposals hit (r4b): a tap after the effects on its target,
/// the snow supertype, two amounts of damage in one sentence, a shield of the next damage, the controller of a target
/// named by its type, a card put into an opponent's graveyard, the top of a library, a group that is still its type,
/// two returns in one sentence, the turns of others, a discard by the owner and a spell of several colors.
fn add_w8c(ability: &Ability, texts: &mut Vec<String>) {
    w8c_then_tap::add(ability, texts);
    w8c_snow::add(ability, texts);
    w8c_damage_and::add(ability, texts);
    w8c_prevent_next::add(ability, texts);
    w8c_kind_controller::add(ability, texts);
    w8c_opponent_graveyard::add(ability, texts);
    w8c_on_top::add(ability, texts);
    w8c_still_kind::add(ability, texts);
    w8c_return_both::add(ability, texts);
    w8c_other_turns::add(ability, texts);
    w8c_then_discards::add(ability, texts);
    w8c_spell_colors::add(ability, texts);
}

/// Adds the wordings of the spells built of several steps and of the draw, choice, rule and mana steps (r3w9).
fn add_w5b(ability: &Ability, texts: &mut Vec<String>) {
    w5b_casts::add(ability, texts);
    w5b_rules::add(ability, texts);
    w5b_mana::add(ability, texts);
    w5b_damage::add(ability, texts);
}

/// Adds the trigger heads the writers before do not word (r3w8): a discard of a kind of card, an attack on an opponent
/// or their planeswalker, a target of a spell, a batch of events, several events in one head.
fn add_w5a(ability: &Ability, texts: &mut Vec<String>) {
    w5a_heads::add(ability, texts);
    w5a_combat::add(ability, texts);
    w5a_conditions::add(ability, texts);
}

/// Adds the wordings of the step leftovers of the writer wave (r3wa): a delayed trigger said after its effects and
/// the counters a player gets.
fn add_w5c(ability: &Ability, texts: &mut Vec<String>) {
    w5c_later::add(ability, texts);
    w5c_player_counters::add(ability, texts);
}

/// Adds the wordings of the choices and payments around steps (r3w3): "you may", "you may pay", "When you do", "unless".
fn add_w4c(ability: &Ability, texts: &mut Vec<String>) {
    w4c_may_pay::add(ability, texts);
}

/// Adds the wordings of continuous effects and static grants (r3w2): conditions, grants, pumps, durations, a land
/// that becomes a creature, permanents kept tapped and a card given to an opponent.
fn add_w4b(ability: &Ability, texts: &mut Vec<String>) {
    w4b_conditions::add(ability, texts);
    w4b_grants::add(ability, texts);
    w4b_pumps::add(ability, texts);
    w4b_becomes::add(ability, texts);
    w4b_durations::add(ability, texts);
    w4b_untap::add(ability, texts);
    w4b_control::add(ability, texts);
}

/// Adds the wordings of delayed triggers and of steps repeated for each player that close an ability (r3w4).
fn add_w4d(ability: &Ability, texts: &mut Vec<String>) {
    w4d_later::add(ability, texts);
    w4d_each::add(ability, texts);
}

/// Adds the rewordings of the Mismatch proposals of the names A to L (r3w6): sentences joined or ordered as the oracle
/// text does, counts by "for each", a token named before its ability in quotes and the pronouns of a person, last.
fn add_w4f(texts: &mut Vec<String>) {
    w4f_sentences::add(texts);
    w4f_counts::add(texts);
    w4f_tokens::add(texts);
    w4f_pronouns::add(texts);
}

/// Adds the other printed wordings of the Mismatch proposals of the names M to Z (r3w7): amounts, then texts.
fn add_w4g(ability: &Ability, texts: &mut Vec<String>) {
    w4g_amounts::w4g_add_amounts(ability, texts);
    w4g_texts::w4g_add_texts(ability, texts);
}

/// Adds the wordings of the conditional steps (r3w1): "If X, Y.", "Y if X.", "Otherwise, Z.", "If you can't, Z.".
fn add_w4a(ability: &Ability, texts: &mut Vec<String>) {
    w4a_if::add(ability, texts);
}

/// Adds the wordings of the zone moves, counters and modes (r3w5) and of their intervening "if".
fn add_w4e(ability: &Ability, texts: &mut Vec<String>) {
    texts.extend(w4e_bodies(ability));
    texts.extend(w4e_if::wordings(ability, |bare| {
        let mut found = super::variants(bare);
        found.extend(w4e_bodies(bare));
        found
    }));
}

/// The wordings of the steps, the counts of counters and the modes these writers word.
fn w4e_bodies(ability: &Ability) -> Vec<String> {
    let mut found = w4e_steps(ability);
    found.extend(w4e_where_x::wordings(ability, |replaced| {
        let mut found = super::variants(replaced);
        found.extend(w4e_steps(replaced));
        found
    }));
    found.extend(w4e_modes::wordings(ability));
    found.extend(w4e_flash::wordings(ability));
    found
}

/// The wordings of the counter and move steps, each sentence put in the place of the steps.
fn w4e_steps(ability: &Ability) -> Vec<String> {
    let (Ability::Spell { targets, effects }
    | Ability::Activated {
        targets, effects, ..
    }
    | Ability::Triggered {
        targets, effects, ..
    }) = ability
    else {
        return Vec::new();
    };
    let mut sentences = w4e_counters::sentences(targets, effects);
    sentences.extend(w4e_moves::sentences(targets, effects));
    let mut found: Vec<String> = sentences
        .iter()
        .flat_map(|sentence| w4e_with_effects(ability, sentence))
        .collect();
    found.extend(w4e_counters::subtype_choice(ability));
    found.extend(w4e_moves::search_discard(ability));
    found
}

/// The wordings of the ability with `sentence` for its effects, also when a restriction follows them ("Activate only as
/// a sorcery.").
fn w4e_with_effects(ability: &Ability, sentence: &str) -> Vec<String> {
    let found = standin::with_effects(ability, sentence);
    if !found.is_empty() {
        return found;
    }
    let Ability::Activated { .. } = ability else {
        return found;
    };
    let mut stand = ability.clone();
    if let Ability::Activated {
        targets, effects, ..
    } = &mut stand
    {
        targets.clear();
        *effects = vec![crate::effect::Effect::Draw {
            who: crate::refs::Ref::You,
            count: crate::value::Amount::one(),
        }];
    }
    let said = format!("{}.", super::capitalize(sentence));
    super::variants(&stand)
        .iter()
        .filter(|text| text.contains(": Draw a card. "))
        .map(|text| text.replacen(": Draw a card. ", &format!(": {said} "), 1))
        .collect()
}

/// Adds the wordings of the M to Z tail of the stored proposals (r2dge): whole abilities matched as data.
fn add_c11g(ability: &Ability, texts: &mut Vec<String>) {
    c11g_wholes::c11g_add_wholes(ability, texts);
}

/// Adds the wordings of the single-wording tail of the stored proposals of the names A to L (r2dge): whole abilities.
fn add_c11f(ability: &Ability, texts: &mut Vec<String>) {
    c11f_wholes::c11f_add_wholes(ability, texts);
}

/// Adds the wordings of the third census (r2dg): the land put from the top of the library and damage equal to its power.
fn add_c10_cen(ability: &Ability, texts: &mut Vec<String>) {
    c10_cen_land_look::add(ability, texts);
    c10_cen_power_damage::add(ability, texts);
}

/// Adds the wordings of the third samples' writers (r2bfb, r2bgb, r2bhb): the returns, then the whole abilities.
fn add_c9_wr(ability: &Ability, texts: &mut Vec<String>) {
    c9_wr_returns::add(ability, texts);
    c9_wr_wholes::add(ability, texts);
    w6d_wholes::w6d_add_wholes(ability, texts);
}

/// The wordings of the sixth sample of the names A to F; it runs last because it rewrites the texts of the others.
fn add_c6_af(ability: &Ability, texts: &mut Vec<String>) {
    c6_af::c6_af_add(ability, texts);
}

/// Adds the wordings of the sixth sample of the names M to R; it runs last and edits what the others found.
fn add_c6_mr(ability: &Ability, texts: &mut Vec<String>) {
    c6_mr_writers::add(ability, texts);
}

/// The wordings of the dice, coins, votes and piles of the (r1at) pass.
fn add_c5_dice(ability: &Ability, texts: &mut Vec<String>) {
    c5_dice_piles::add(ability, texts);
}

/// The wordings of the fifth sample of the names A to F; it runs last because it rewrites the texts of the others.
fn add_c5_af(ability: &Ability, texts: &mut Vec<String>) {
    c5_af::c5_af_add(ability, texts);
}

/// The wordings of the sixth sample of the names G to L (r2dc).
fn add_c6_gl(ability: &Ability, texts: &mut Vec<String>) {
    c6_gl_wholes::c6_gl_add_wholes(ability, texts);
    c6_gl_returns::c6_gl_add_returns(ability, texts);
    c6_gl_causes::c6_gl_add_causes(ability, texts);
}

/// The wordings of the engine-gap pass (r2bf, r2bg, r2bh).
fn add_c6_eg(ability: &Ability, texts: &mut Vec<String>) {
    c6_eg::add(ability, texts);
}

/// The wordings of the fourth sample of the names A to F; it runs after the others because it rewrites their texts.
fn add_c4_af(ability: &Ability, texts: &mut Vec<String>) {
    c4_af::add(ability, texts);
}

/// The wordings of the fourth sample of the names G to L.
fn add_c4_gl(ability: &Ability, texts: &mut Vec<String>) {
    c4_attack_token::add(ability, texts);
    c4_aura_becomes::add(ability, texts);
    c4_counters_you_put::add(ability, texts);
    c4_prevent_combat::add(ability, texts);
    c4_support::add(ability, texts);
    c4_swaps::add(ability, texts);
}

/// The wordings of the added phases, dice, coins and votes of the (r1as) pass.
fn c4_oracle_only(ability: &Ability, texts: &mut Vec<String>) {
    c4_phases::add(ability, texts);
}

/// The wordings of the gaps of the model-assisted reading that (r2bi) closes.
fn add_c4(ability: &Ability, texts: &mut Vec<String>) {
    c4_group_effects::add(ability, texts);
    c4_moves::add(ability, texts);
    c4_token_counts::add(ability, texts);
}

/// The wordings of the fifth sample of the names G to L; the swaps run last because they edit the wordings before them.
fn add_c5_gl(ability: &Ability, texts: &mut Vec<String>) {
    c5_gl_twins::c5_gl_add(ability, texts);
    c5_gl_modes::c5_gl_add(ability, texts);
    c5_gl_swaps::c5_gl_add(ability, texts);
}

/// The wordings of the party, chosen-player and prepare cards (c14a), in a function of their own.
fn c4_writers(ability: &Ability, texts: &mut Vec<String>) {
    c4_party_roles::add(texts);
    c4_those_creatures::add(ability, texts);
    c4_chosen_damage::add(ability, texts);
    c4_delayed_prepare::add(ability, texts);
    c4_each_of_up_to::add(ability, texts);
    c4_exile_copy::add(ability, texts);
    c4_exile_copy::add_and_or(texts);
}

/// The wordings of the modules after `counters_put`, in a function of their own to keep each under the line limit.
fn add_more(ability: &Ability, texts: &mut Vec<String>) {
    discard_play::add(ability, texts);
    double_counters::add(ability, texts);
    each_of::add(ability, texts);
    each_to_owner::add(ability, texts);
    each_sweep::add(ability, texts);
    either_event::add(ability, texts);
    energy_amount::add(ability, texts);
    energy_each::add(ability, texts);
    energy_words::add(ability, texts);
    entry_counters::add(ability, texts);
    except_for::add(ability, texts);
    except_lands_tokens::add(ability, texts);
    exile_each::add(ability, texts);
    exile_play::add(ability, texts);
    af2::add(ability, texts);
    for_each_card::add(ability, texts);
    goaded::add(ability, texts);
    have_deal::add(texts);
    kicked_trigger::add(ability, texts);
    land_subtypes::add(ability, texts);
    performed_steps::add(ability, texts);
    first_main::add(ability, texts);
    sacrifice_search::add(ability, texts);
    itself::add(ability, texts);
    land_types::add(ability, texts);
    look_put::add(ability, texts);
    life_counters::add(ability, texts);
    look_split::add(ability, texts);
    attack_player::add(ability, texts);
    counter_sacrifice::add(ability, texts);
    cycle_this::add(ability, texts);
    destroy_two::add(ability, texts);
    do_this_once::add(ability, texts);
    each_draw::add(ability, texts);
    entered_under::add(ability, texts);
    group_verbs::add(ability, texts);
    kicked::add(ability, texts);
    spell_value::add(ability, texts);
    that_creature::add(ability, texts);
    untapped_cost::add(ability, texts);
    minus_amount::add(ability, texts);
    non_lists::add(ability, texts);
    of_their_choice::add(ability, texts);
    other_than::add(ability, texts);
    pump_untap::add(ability, texts);
    put_back_order::add(ability, texts);
    put_graveyard::add(ability, texts);
    search_put::add(texts);
    sized_condition::add(ability, texts);
    target_damage::add(ability, texts);
    that_player::add(ability, texts);
    that_spells::add(ability, texts);
    untap_lands::add(ability, texts);
    paid_steps::add(ability, texts);
    sacrifice_it::add(ability, texts);
    sacrifices::add(ability, texts);
    target_and::add(ability, texts);
    trailing_if::add(ability, texts);
    one_or_more_attack::add(ability, texts);
    put_into_graveyard::add(ability, texts);
    regen_this_way::add(ability, texts);
    return_damager::add(ability, texts);
    unless::add(ability, texts);
    other_players::add(ability, texts);
    put_into::add(ability, texts);
    joined_steps::add(ability, texts);
    wheres::add(ability, texts);
    without_counter::add(ability, texts);
    you_create::add(ability, texts);
    third_sample_of_m_to_r(ability, texts);
    add_c4_gl(ability, texts);
}

/// The wordings of the third sample of the names M to R.
fn third_sample_of_m_to_r(ability: &Ability, texts: &mut Vec<String>) {
    all_non::add(ability, texts);
    bounce_all::add(ability, texts);
    channel_discard::add(ability, texts);
    damage_each_and::add(ability, texts);
    delayed_first::add(ability, texts);
    energy_cost_words::add(ability, texts);
    flash_permission::add(ability, texts);
    graveyard_shuffle::add(ability, texts);
    have_cards::add(ability, texts);
    home_pair::add(ability, texts);
    may_have_player::add(ability, texts);
    owner_shuffles::add(ability, texts);
    owners_graveyards::add(ability, texts);
    pay_two_costs::add(ability, texts);
    poison_pump::add(ability, texts);
    swaps::add(ability, texts);
    tail_sentences::add(ability, texts);
    those_cards::add(ability, texts);
    uncounterable_group::add(ability, texts);
    while_control::add(ability, texts);
    add_sz3(ability, texts);
}

/// Adds the wordings of the third sample of S to Z; it runs last because some of them change the wordings found so far.
fn add_sz3(ability: &Ability, texts: &mut Vec<String>) {
    at_least::add(ability, texts);
    attach_it::add(ability, texts);
    battalion::add(ability, texts);
    cast_free::add(ability, texts);
    cast_from_hand::add(ability, texts);
    choice_keywords::add(ability, texts);
    choose_targets::add(ability, texts);
    color_lists::add(texts);
    conditional_rule::add(ability, texts);
    damage_back::add(ability, texts);
    dealt_damage_sz::add(ability, texts);
    destroy_that::add(ability, texts);
    enters_and_step::add(ability, texts);
    entry_sum::add(ability, texts);
    exiled_from::add(ability, texts);
    flash_casts::add(ability, texts);
    graveyard_cards::add(ability, texts);
    half_tokens::add(ability, texts);
    kicked_enters::add(ability, texts);
    lowercase_until::add(texts);
    nonlegendary::add(texts);
    own_permanent::add(ability, texts);
    paired_returns::add(ability, texts);
    plus_one_counters::add(ability, texts);
    prevent_remove::add(ability, texts);
    put_that_card::add(ability, texts);
    quote_commas::add(ability, texts);
    return_this_card::add(ability, texts);
    sacrifice_unless::add(ability, texts);
    spells_cast_damage::add(ability, texts);
    target_costs::add(ability, texts);
    that_many_tokens::add(ability, texts);
    trigger_again::add(ability, texts);
    each_draw_sz2::add(ability, texts);
    each_sacrifice::add(ability, texts);
    you_create::add(ability, texts);
    and_create::add(ability, texts);
    damage_instead::add(ability, texts);
    cant_countered::add(ability, texts);
    ferocious::add(ability, texts);
    flash_rule::add(ability, texts);
    leaves_graveyard::add(ability, texts);
    spell_controller::add(ability, texts);
    that_creature_sz2::add(ability, texts);
    add_c4_sz(ability, texts);
}

/// Adds the wordings of the fourth sample of S to Z; it runs after the others because some of them change what was found.
fn add_c4_sz(ability: &Ability, texts: &mut Vec<String>) {
    c4_sz::add(ability, texts);
    c4_twins::add(ability, texts);
    c4_swaps_sz::add(ability, texts);
}

/// Adds the wordings of the fourth sample of the names M to R; it runs after the others and edits what they found.
fn add_c4_mr(ability: &Ability, texts: &mut Vec<String>) {
    c4_mr::add(ability, texts);
}

/// Adds the wordings of the fifth sample of the names M to R; it runs last and edits what the others found.
fn add_c5_mr(ability: &Ability, texts: &mut Vec<String>) {
    c5_mr_writers::add(ability, texts);
}

/// Adds the wordings of the fifth sample of the names S to Z.
fn add_c5_sz(ability: &Ability, texts: &mut Vec<String>) {
    c5_sz_sentences::add(ability, texts);
    c5_sz_wholes::add(ability, texts);
}

/// Adds the wordings of the gap constructs.
fn add_c6_gap(ability: &Ability, texts: &mut Vec<String>) {
    c6_gap_wholes::add(ability, texts);
}

/// Adds the wordings of the sixth sample of the names S to Z; the join runs last because it rewrites the others.
fn add_c6_sz(ability: &Ability, texts: &mut Vec<String>) {
    c6_sz_bodies::add(ability, texts);
    c6_sz_wholes::add(ability, texts);
    c6_sz_and_you::add(texts);
}
