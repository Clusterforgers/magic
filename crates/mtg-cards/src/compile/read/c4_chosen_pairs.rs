//! Two replacements of damage in one sentence about the chosen player: "If a source would deal damage to the chosen
//! player or a permanent they control, it deals double that damage instead." is one replacement for the player and one
//! for the permanents they control.

use crate::ability::Ability;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The wordings of the two replacements read as one sentence, with a capital and a period; empty for any other two
/// abilities.
pub(super) fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Replacement {
            when:
                ReplaceWhen::Damage {
                    to: Some(Ref::Chosen),
                    source: from_player,
                    combat: None,
                },
            by: ReplaceBy::Multiply(2),
            condition: None,
            from: ZoneKind::Battlefield,
        },
        Ability::Replacement {
            when:
                ReplaceWhen::Damage {
                    to: Some(Ref::Each(group)),
                    source: from_permanents,
                    combat: None,
                },
            by: ReplaceBy::Multiply(2),
            condition: None,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if !from_player.is_empty()
        || !from_permanents.is_empty()
        || group.0 != [Pred::ControlledBy(Ref::Chosen)]
    {
        return Vec::new();
    }
    ["it deals", "that source deals"]
        .iter()
        .map(|deals| {
            format!(
                "If a source would deal damage to the chosen player or a permanent they control, {deals} double that damage instead."
            )
        })
        .collect()
}
