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
    /// Only a tap that makes mana of this type ("for {C}").
    pub produced: Option<mtg_core::mana::ManaType>,
    /// Only a tap that makes mana of the color the source chose.
    pub chosen_color: bool,
    /// What must hold as the permanent is tapped (an intervening "if").
    pub condition: Option<&'a Condition>,
    /// Who adds the mana: the controller of the ability (`You`) or the player who tapped the permanent (`Triggering`).
    pub who: &'a Ref,
    /// The mana it adds for the player who tapped the permanent.
    pub mana: &'a Produce,
    /// The player who adds it may decline ("its controller may add"), so a tap also makes the mana without it.
    pub optional: bool,
    /// What the mana it adds may be spent on ("Spend this mana only to pay cumulative upkeep costs"); empty means anything.
    pub only: &'a [crate::spend::SpendOn],
}

/// The shape of `ability` as a triggered mana ability, if it has one: an event of a permanent tapped for mana, no
/// targets, and one `AddMana` for the player who tapped it, once, or a `May` of that player around it; it works from the battlefield, or from the command zone on an emblem.
pub fn mana_trigger(ability: &Ability) -> Option<ManaTrigger<'_>> {
    let Ability::Triggered {
        when:
            TriggerPattern::TappedForMana {
                what,
                by,
                produced,
                chosen_color,
            },
        intervening_if,
        limit: None,
        from: ZoneKind::Battlefield | ZoneKind::Command,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    // "Its controller may add" is a `May` of that player around the one `AddMana`.
    let (effects, optional, chooser) = match effects.as_slice() {
        [Effect::May { who, effects }] => (effects.as_slice(), true, Some(who)),
        all => (all, false, None),
    };
    let [
        Effect::AddMana {
            who,
            mana,
            times,
            only,
            lasts,
            when_spent: None,
        },
    ] = effects
    else {
        return None;
    };
    if chooser.is_some_and(|chooser| chooser != who) {
        return None;
    }
    // "You add" is the player who tapped it only when that is the player the event names.
    let taps_for_you = *who == Ref::You && *by == Ref::You;
    let taps_for_them = *who == Ref::Triggering;
    let adds = match mana {
        Produce::Fixed(list) => !list.is_empty(),
        Produce::AnyColor(count) => count.fixed().is_some_and(|count| count > 0),
        Produce::ChosenColor(count) => *count > 0,
        Produce::AnyCombination { count, .. } => count.fixed().is_some_and(|count| count > 0),
        Produce::OfTapped => true,
        Produce::Instead { base, instead, .. } => {
            matches!((&**base, &**instead), (Produce::Fixed(a), Produce::Fixed(b)) if !a.is_empty() && !b.is_empty())
        }
        _ => false,
    };
    (targets.is_empty()
        && times.is_one()
        && lasts.is_step()
        && adds
        && (taps_for_you || taps_for_them))
        .then_some(ManaTrigger {
            what,
            by,
            produced: *produced,
            chosen_color: *chosen_color,
            condition: intervening_if.as_ref(),
            who,
            mana,
            optional,
            only,
        })
}

#[cfg(test)]
mod tests {
    //! Goal: a triggered ability on a permanent tapped for mana is a triggered mana ability only when it adds mana, perhaps restricted or optional, for the player who tapped it with no target and no limit; every other ability stays on the stack.
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
    #[case::restricted(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#
    )]
    #[case::snowfall(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Subtype":"Island"}]}},"effects":[{"May":{"who":"Triggering","effects":[{"AddMana":{"who":"Triggering","mana":{"Instead":{"base":{"Fixed":["Blue"]},"when":{"Matches":{"subject":"Triggering","selector":[{"Supertype":"Snow"}]}},"instead":{"Fixed":["Blue","Blue"]}}},"only":["CumulativeUpkeep"]}}]}}]}}"#
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
    #[case::and_something_else(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}},{"Draw":{}}]}}"#
    )]
    #[case::a_limit(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::another_event(
        r#"{"Triggered":{"when":{"Tapped":{"what":[{"Type":"Land"}]}},"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#
    )]
    #[case::may_of_another_player(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"May":{"effects":[{"AddMana":{"who":"Triggering","mana":{"Fixed":["Green"]}}}]}}]}}"#
    )]
    #[case::damage_instead(
        r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#
    )]
    fn any_other_trigger_on_a_tap_for_mana_stays_on_the_stack(#[case] json: &str) {
        assert!(mana_trigger(&ability(json)).is_none(), "{json}");
    }
}
