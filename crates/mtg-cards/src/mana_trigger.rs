//! Triggered mana abilities (CR 605.1b): "Whenever you tap a land for mana, add one mana of any type that land
//! produced", "Whenever enchanted land is tapped for mana, its controller adds an additional {G}".
//!
//! Such an ability adds mana as the mana ability that triggered it resolves and never uses the stack (CR 605.4a), so
//! the engine folds its mana into the options of the land it triggers on instead of detecting it as a trigger.

use crate::ability::Ability;
use crate::effect::{Effect, Produce};
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// The shape of a triggered mana ability the engine folds into the mana of a tap.
#[derive(Debug, Clone, PartialEq)]
pub struct ManaTrigger<'a> {
    /// The permanents whose tap for mana triggers it.
    pub what: &'a Selector,
    /// Who taps them.
    pub by: &'a Ref,
    /// What must hold as the permanent is tapped (an intervening "if").
    pub condition: Option<&'a Condition>,
    /// Who adds the mana: the controller of the ability (`You`) or the player who tapped the permanent (`Triggering`).
    pub who: &'a Ref,
    /// The mana it adds for the player who tapped the permanent.
    pub mana: &'a Produce,
}

/// The shape of `ability` as a triggered mana ability, if it has one: an event of a permanent tapped for mana, no
/// targets, and one `AddMana` for the player who tapped it, once, with no restriction on spending it.
pub fn mana_trigger(ability: &Ability) -> Option<ManaTrigger<'_>> {
    let Ability::Triggered {
        when: TriggerPattern::TappedForMana { what, by },
        intervening_if,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let [
        Effect::AddMana {
            who,
            mana,
            times,
            only,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    // "You add" is the player who tapped it only when that is the player the event names.
    let taps_for_you = *who == Ref::You && *by == Ref::You;
    let taps_for_them = *who == Ref::Triggering;
    let adds = match mana {
        Produce::Fixed(list) => !list.is_empty(),
        Produce::AnyColor(count) => count.fixed().is_some_and(|count| count > 0),
        Produce::ChosenColor(count) => *count > 0,
        Produce::AnyCombination { count, .. } => count.fixed().is_some_and(|count| count > 0),
        Produce::OfTapped => true,
        _ => false,
    };
    (targets.is_empty()
        && times.is_one()
        && only.is_empty()
        && adds
        && (taps_for_you || taps_for_them))
        .then_some(ManaTrigger {
            what,
            by,
            condition: intervening_if.as_ref(),
            who,
            mana,
        })
}

#[cfg(test)]
mod tests {
    //! Goal: a triggered ability on a permanent tapped for mana is a triggered mana ability only when it adds mana for the player who tapped it with no target, no limit and no restriction; every other ability stays on the stack.
    use rstest::rstest;

    use super::*;

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::wild_growth(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::fertile_ground(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"AnyColor":1}}}]}}"#
    )]
    #[case::mirari_wake(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":"OfTapped"}}]}}"#
    )]
    #[case::mana_flare(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":"OfTapped"}}]}}"#
    )]
    #[case::crypt_ghast(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Subtype":"Swamp"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}}]}}"#
    )]
    fn a_trigger_that_adds_mana_for_the_player_who_tapped_is_a_mana_ability(#[case] json: &str) {
        assert!(mana_trigger(&ability(json)).is_some(), "{json}");
    }

    #[rstest]
    #[case::you_add_for_a_tap_by_anyone(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::mana_for_an_opponent(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"who":"Opponents","mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::restricted(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#
    )]
    #[case::and_something_else(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}},{"Draw":{}}]}}"#
    )]
    #[case::a_limit(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::another_event(
        r#"{"Triggered":{"when":{"Tapped":{"what":[{"Type":"Land"}]}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::damage_instead(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#
    )]
    fn any_other_trigger_on_a_tap_for_mana_stays_on_the_stack(#[case] json: &str) {
        assert!(mana_trigger(&ability(json)).is_none(), "{json}");
    }
}
