//! Wordings the oracle text uses for an ability that the renderers write another way, each derived from the wordings of
//! [`super::variants`] by the data of the ability. The compiler's own reads stay on `variants`; only the proposal
//! checker accepts these (`written_variants`).

mod and_draw;
mod and_or;
mod cast_condition;
mod conditions;
mod controller_damage;
mod controller_draws;
mod creature_enters;
mod destroy_all;
mod doubled;
mod draw_equal;
mod each_and_each;
mod each_of;
mod each_to_owner;
mod either_event;
mod entry_counters;
mod except_for;
mod exile_play;
mod for_each_card;
mod goaded;
mod have_deal;
mod land_subtypes;
mod land_types;
mod look_put;
mod look_split;
mod minus_amount;
mod non_lists;
mod of_their_choice;
mod other_than;
mod put_graveyard;
mod search_put;
mod sized_condition;
mod target_damage;
mod that_spells;
mod untap_lands;
mod wheres;
mod you_create;
mod your_graveyard;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    and_draw::add(ability, texts);
    and_or::add(texts);
    cast_condition::add(ability, texts);
    conditions::add(ability, texts);
    controller_damage::add(ability, texts);
    controller_draws::add(ability, texts);
    creature_enters::add(ability, texts);
    destroy_all::add(ability, texts);
    doubled::add(ability, texts);
    draw_equal::add(ability, texts);
    each_and_each::add(ability, texts);
    each_of::add(ability, texts);
    each_to_owner::add(ability, texts);
    either_event::add(ability, texts);
    entry_counters::add(ability, texts);
    except_for::add(ability, texts);
    exile_play::add(ability, texts);
    for_each_card::add(ability, texts);
    goaded::add(ability, texts);
    have_deal::add(texts);
    land_subtypes::add(ability, texts);
    land_types::add(ability, texts);
    look_put::add(ability, texts);
    look_split::add(ability, texts);
    minus_amount::add(ability, texts);
    non_lists::add(ability, texts);
    of_their_choice::add(ability, texts);
    other_than::add(ability, texts);
    put_graveyard::add(ability, texts);
    search_put::add(texts);
    sized_condition::add(ability, texts);
    target_damage::add(ability, texts);
    that_spells::add(ability, texts);
    untap_lands::add(ability, texts);
    wheres::add(ability, texts);
    you_create::add(ability, texts);
    your_graveyard::add(ability, texts);
}
