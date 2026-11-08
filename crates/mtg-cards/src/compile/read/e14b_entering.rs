//! The wordings of a permanent that enters only if a cost is paid as it would enter, and otherwise goes to its owner's
//! graveyard (Mox Diamond, Lotus Vale, Kjeldoran Outpost).

use crate::cost::Cost;

/// "If ~ would enter, sacrifice two untapped lands instead. If you do, put ~ onto the battlefield. If you don't, put it
/// into its owner's graveyard", with "you may" before the cost for one that may be declined.
pub(super) fn pay_or_graveyard_texts(cost: &Cost, optional: bool) -> Vec<String> {
    let Some(paid) = super::costs::text(cost) else {
        return Vec::new();
    };
    let mut letters = paid.chars();
    let Some(first) = letters.next() else {
        return Vec::new();
    };
    let paid: String = first.to_lowercase().chain(letters).collect();
    let may = if optional { "you may " } else { "" };
    vec![format!(
        "If ~ would enter, {may}{paid} instead. If you do, put ~ onto the battlefield. If you don't, put it into its owner's graveyard"
    )]
}
