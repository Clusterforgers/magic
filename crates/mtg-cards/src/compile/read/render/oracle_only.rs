//! Wordings the oracle text uses for an ability that the renderers write another way, each derived from the wordings of
//! [`super::variants`] by the data of the ability. The compiler's own reads stay on `variants`; only the proposal
//! checker accepts these (`written_variants`).

mod also_create;
mod bare;
mod cast_graveyard;
mod conditions;
mod copy_quotes;
mod counter_any;
mod discard_play;
mod each_of;
mod either_event;
mod energy_amount;
mod entry_counters;
mod exile_each;
mod exile_play;
mod land_types;
mod look_split;
mod non_lists;
mod one_or_more_attack;
mod put_into_graveyard;
mod unless;
mod wheres;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    also_create::add(ability, texts);
    cast_graveyard::add(ability, texts);
    conditions::add(ability, texts);
    copy_quotes::add(ability, texts);
    counter_any::add(ability, texts);
    discard_play::add(ability, texts);
    each_of::add(ability, texts);
    either_event::add(ability, texts);
    energy_amount::add(ability, texts);
    entry_counters::add(ability, texts);
    exile_each::add(ability, texts);
    exile_play::add(ability, texts);
    land_types::add(ability, texts);
    look_split::add(ability, texts);
    non_lists::add(ability, texts);
    one_or_more_attack::add(ability, texts);
    put_into_graveyard::add(ability, texts);
    unless::add(ability, texts);
    wheres::add(ability, texts);
}
