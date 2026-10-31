//! Rooms (CR 709.5): a split card whose halves are doors. The card as a whole is what a fully unlocked Room permanent
//! is (both names, the sum of the costs, the abilities of both doors); each door is its own face, whose "when you unlock
//! this door" abilities name that face; a Room with both doors locked has only its types.

use std::sync::LazyLock;

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

/// The abilities read from the face `index`, with each "when you unlock this door" naming that face.
pub fn dk1c5_this_door(mut abilities: Vec<Ability>, index: usize) -> Vec<Ability> {
    let face = u8::try_from(index).unwrap_or(u8::MAX);
    for ability in &mut abilities {
        if let Ability::Triggered {
            when: TriggerPattern::DoorUnlocked {
                door: Some(door), ..
            },
            ..
        } = ability
        {
            *door = face;
        }
    }
    abilities
}

/// Completes the whole card of a Room once its faces are set: it has the subtypes and the abilities of both doors.
/// Any other card is left as it is.
pub fn dk1c5_doors(whole: &mut CardDefinition) {
    if !whole.is_room() {
        return;
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

#[cfg(test)]
mod tests {
    //! Goal: each face's "when you unlock this door" names that face, and other abilities are left as they are.
    use super::*;

    #[test]
    fn each_face_names_its_own_door() {
        let unlock: Ability = serde_json::from_str(
            r#"{"Triggered":{"when":{"DoorUnlocked":{"who":"You","door":0}},"effects":[{"Draw":{}}]}}"#,
        )
        .unwrap();
        let other: Ability = serde_json::from_str(r#"{"Keyword":"Flash"}"#).unwrap();
        for face in 0..2u8 {
            let named = dk1c5_this_door(vec![unlock.clone(), other.clone()], usize::from(face));
            let Ability::Triggered {
                when: TriggerPattern::DoorUnlocked { door, .. },
                ..
            } = &named[0]
            else {
                panic!("the trigger stays");
            };
            assert_eq!((*door, &named[1]), (Some(face), &other));
        }
    }
}
