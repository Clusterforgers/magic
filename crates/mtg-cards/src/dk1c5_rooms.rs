//! Rooms (CR 709.5): a split card whose halves are doors. The card as a whole is what a fully unlocked Room permanent
//! is (both names, the sum of the costs, the abilities of both doors); each door is its own face, whose "when you unlock
//! this door" abilities name that face; a Room with both doors locked has only its types.

use std::sync::{Arc, LazyLock};

use mtg_core::card::{CardType, Subtype};

use crate::ability::Ability;
use crate::definition::CardDefinition;
use crate::trigger::TriggerPattern;

/// What a Room permanent with both doors locked is: an Enchantment — Room with no name, cost or abilities.
pub static DK1C5_LOCKED: LazyLock<CardDefinition> = LazyLock::new(|| {
    let mut locked = CardDefinition::new("");
    locked.types.insert(CardType::Enchantment);
    locked.subtypes.push(Subtype("Room".to_owned()));
    locked
});

/// Completes the whole card of a Room once its faces are set: each door's unlock triggers name their face, and the
/// whole card has the subtypes and the abilities of both doors. Any other card is left as it is.
pub fn dk1c5_doors(whole: &mut CardDefinition) {
    if !whole.is_room() {
        return;
    }
    for (index, face) in whole.faces.iter_mut().enumerate() {
        let door = u8::try_from(index).unwrap_or(u8::MAX);
        for rule in &mut face.rules {
            if let Ability::Triggered {
                when:
                    TriggerPattern::DoorUnlocked {
                        door: Some(own), ..
                    },
                ..
            } = &**rule
                && *own != door
            {
                let mut named = (**rule).clone();
                if let Ability::Triggered {
                    when: TriggerPattern::DoorUnlocked { door: own, .. },
                    ..
                } = &mut named
                {
                    *own = Some(door);
                }
                *rule = Arc::new(named);
            }
        }
    }
    whole.subtypes.clear();
    for face in &whole.faces {
        for subtype in &face.subtypes {
            if !whole.subtypes.contains(subtype) {
                whole.subtypes.push(subtype.clone());
            }
        }
    }
    whole.rules = whole
        .faces
        .iter()
        .flat_map(|face| face.rules.clone())
        .collect();
    whole.abilities = whole
        .faces
        .iter()
        .flat_map(|face| face.abilities.clone())
        .collect();
}
