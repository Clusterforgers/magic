//! Words of the fourth sample of the names S to Z that the renderers write another way: the card that cycles itself
//! "may have it deal", "is put into a graveyard from the battlefield" for "dies", "that player loses life" for the
//! player of an event, a commander "you own", all permanents "except for artifacts and lands" and an activation
//! "during your upkeep". Each entry applies only to an ability of the shape its guard names.

use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// What an ability must have for a swap to apply.
type Guard = fn(&Ability) -> bool;

/// The swaps: where they apply, the renderers' words and the card's.
const SWAPS: [(Guard, &str, &str); 7] = [
    (
        land_enters_tapped,
        "Whenever a tapped land you control enters,",
        "Whenever a land you control enters tapped,",
    ),
    (
        cycles_to_damage,
        ", you may have ~ deal ",
        ", you may have it deal ",
    ),
    (
        permanent_dies,
        " dies, ",
        " is put into a graveyard from the battlefield, ",
    ),
    (
        player_loses_unless,
        ", they lose life equal to the number of creatures that player controls unless",
        ", that player loses life equal to the number of creatures they control unless",
    ),
    (
        returns_own_commander,
        " target your commander to its owner's hand",
        " target commander you own from the battlefield to your hand",
    ),
    (
        destroys_all_but,
        "Destroy each nonartifact, nonland permanent.",
        "Destroy all permanents except for artifacts and lands.",
    ),
    (
        upkeep_only,
        "during your turn and only during any upkeep step",
        "during your upkeep",
    ),
];

/// Adds the wordings with the swaps that apply, each swap also on the wordings the swaps before it made.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut found: Vec<String> = Vec::new();
    for (applies, from, to) in SWAPS {
        if applies(ability) {
            let swapped: Vec<String> = texts
                .iter()
                .chain(&found)
                .filter(|text| text.contains(from))
                .map(|text| text.replacen(from, to, 1))
                .collect();
            found.extend(swapped);
        }
    }
    texts.extend(found);
}

/// The rules as JSON, which the guards search for the shape they want.
fn json(ability: &Ability) -> String {
    serde_json::to_string(ability).unwrap_or_default()
}

fn cycles_to_damage(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::Cycled { .. },
            ..
        }
    ) && json(ability).contains(r#""May":{"effects":[{"DealDamage""#)
}

/// A permanent that is neither the card itself nor a creature goes from the battlefield to a graveyard: only a creature
/// dies.
fn permanent_dies(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
                what,
            },
            ..
        } if !what.0.contains(&Pred::This) && !what.0.contains(&Pred::Type(CardType::Creature))
    )
}

fn land_enters_tapped(ability: &Ability) -> bool {
    json(ability).contains(r#""ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"},"Tapped"],"to":"Battlefield"}"#)
}

fn player_loses_unless(ability: &Ability) -> bool {
    json(ability).contains(r#""unpaid":[{"LoseLife":{"who":"Triggering""#)
}

fn returns_own_commander(ability: &Ability) -> bool {
    let text = json(ability);
    text.contains(r#"{"Object":["Commander",{"OwnedBy":"You"}]}"#)
        && text.contains(r#""to":"Hand""#)
}

fn destroys_all_but(ability: &Ability) -> bool {
    json(ability).contains(
        r#""Destroy":{"what":{"Each":[{"Not":{"Type":"Artifact"}},{"Not":{"Type":"Land"}}]}}"#,
    )
}

fn upkeep_only(ability: &Ability) -> bool {
    json(ability).contains(r#""condition":{"All":[{"Step":"Upkeep"},"YourTurn"]}"#)
}
