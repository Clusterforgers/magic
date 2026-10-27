//! Wordings of the fifth sample of the names A to F that only the checker accepts: twins of an ability the renderers
//! write with the card's words put back, a look at the top cards that keeps some, a return "instead", a draw "equal to
//! the difference", a win at a number of counters, an Aura's creature returned and a table of phrase swaps.

mod c5_af_difference;
mod c5_af_instead;
mod c5_af_look_pick;
mod c5_af_returns;
mod c5_af_swaps;
mod c5_af_twins;
mod c5_af_win;

use crate::ability::Ability;

/// Adds the wordings of this sample to `texts`; the swaps run last because they edit the wordings found before them.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    c5_af_twins::c5_af_add(ability, texts);
    c5_af_look_pick::c5_af_add(ability, texts);
    c5_af_instead::c5_af_add(ability, texts);
    c5_af_difference::c5_af_add(ability, texts);
    c5_af_win::c5_af_add(ability, texts);
    c5_af_returns::c5_af_add(ability, texts);
    c5_af_swaps::c5_af_add(ability, texts);
}

/// The ability as JSON, which the guards search for the shape they want.
fn c5_af_json(ability: &Ability) -> String {
    serde_json::to_string(ability).unwrap_or_default()
}

/// The ability read back from JSON after an edit; `None` when the edit made it no ability.
fn c5_af_parse(json: &str) -> Option<Ability> {
    serde_json::from_str(json).ok()
}

/// The wordings the renderers give a twin of an ability.
fn c5_af_variants(twin: &Ability) -> Vec<String> {
    super::super::variants(twin)
}
