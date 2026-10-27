//! Wordings of the fourth sample of the names M to R that only the checker accepts, each derived from the wordings of
//! [`super::super::variants`] by the data of the ability.

mod command_zone;
mod converge_draw;
mod copy_it;
mod copy_returned;
mod counter_check;
mod devotion_if;
mod explores_it;
mod foretold;
mod free_cast;
mod graveyard_limit;
mod graveyard_return;
mod hand_size_draw;
mod last_known;
mod poison_loss;
mod power_plus_toughness;
mod return_self;
mod sacrificed_stats;
mod swaps;
mod switch_each;
mod twice_x;
mod until_first;

use crate::ability::Ability;

/// Adds to `texts` the oracle wordings of the ability that the renderers do not write.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    command_zone::add(ability, texts);
    converge_draw::add(ability, texts);
    copy_it::add(ability, texts);
    copy_returned::add(ability, texts);
    devotion_if::add(ability, texts);
    explores_it::add(ability, texts);
    foretold::add(ability, texts);
    free_cast::add(ability, texts);
    hand_size_draw::add(ability, texts);
    last_known::add(ability, texts);
    poison_loss::add(ability, texts);
    power_plus_toughness::add(ability, texts);
    return_self::add(ability, texts);
    sacrificed_stats::add(ability, texts);
    switch_each::add(ability, texts);
    twice_x::add(ability, texts);
    graveyard_limit::add(ability, texts);
    // These edit the wordings found by the modules above.
    counter_check::add(ability, texts);
    graveyard_return::add(ability, texts);
    until_first::add(ability, texts);
    swaps::add(ability, texts);
}

/// The ability with every `from` in its JSON changed to `to`, a plain twin whose wordings a writer edits; `None` when
/// `from` is not in the ability or the result is no ability.
fn restyled(ability: &Ability, from: &str, to: &str) -> Option<Ability> {
    let json = serde_json::to_string(ability).ok()?;
    json.contains(from).then_some(())?;
    serde_json::from_str(&json.replace(from, to)).ok()
}

/// The wordings of the plain twin of the ability (see [`restyled`]); none when there is no twin.
fn twin_variants(ability: &Ability, from: &str, to: &str) -> Vec<String> {
    restyled(ability, from, to)
        .map(|plain| super::super::variants(&plain))
        .unwrap_or_default()
}
