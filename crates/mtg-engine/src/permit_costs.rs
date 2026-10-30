//! The costs a cast permission adds to the cast it allows (CR 601.2b, 118.8): "You may cast this card from your graveyard by
//! paying 3 life and discarding a card in addition to paying its other costs."
//!
//! They join the way the card is cast through the permission: the mana adds to its mana, the life and the objects the
//! caster picks are parts of it, so the cast pays them with the other parts (`announce/parts.rs`) and a card whose caster
//! cannot give them is not offered (`ways::castable_now`).

use mtg_core::mana::ManaCost;

use crate::announce::required::words_of;
use crate::parts::Part;
use crate::rules::Cast;
use crate::ways::{Way, cast_cost};

/// What the permissions that allow one cast add to its cost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Added {
    /// The mana the cast pays besides the mana of its way.
    mana: ManaCost,
    /// The life and the picked objects the cast pays.
    parts: Vec<Part>,
}

/// What the permissions that allow a cast add to its cost: nothing when one adds none (the caster uses that one), else the
/// cost of the first whose parts the announcement pays; `None` when no permission can be used.
pub(crate) fn added<'a>(permissions: impl IntoIterator<Item = &'a Cast>) -> Option<Added> {
    let mut found = None;
    for permission in permissions {
        match &permission.additional {
            None => return Some(Added::default()),
            Some(cost) if found.is_none() => {
                found = cast_cost(cost).map(|(mana, parts)| Added { mana, parts });
            }
            Some(_) => {}
        }
    }
    found
}

impl Added {
    /// The way with the added cost paid too; `printed` is the mana cost of the card, which a way with no mana of its own pays.
    pub(crate) fn onto(self, mut way: Way, printed: Option<&ManaCost>) -> Way {
        if self.mana == ManaCost::ZERO && self.parts.is_empty() {
            return way;
        }
        let words = words_of(&self.mana, &self.parts);
        if self.mana != ManaCost::ZERO {
            let base = way.cost(printed).unwrap_or(ManaCost::ZERO);
            way.mana = Some(
                self.mana
                    .symbols()
                    .iter()
                    .copied()
                    .fold(base, ManaCost::with),
            );
        }
        way.parts.extend(self.parts);
        way.label = format!("{}, {words}", way.label);
        way
    }
}
