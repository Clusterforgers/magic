//! Goal: round 11 pack g11c reads the block tax (`CantBlockUnless`), an attack tax counted by an `SVar`, and draw
//! triggers of the first card of a draw step (`FirstCardInDrawStep`), and leaves other costs a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a creature with the given script lines and Oracle text, as compact JSON, and its gaps.
fn card(lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Bear\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::its_own(
    "S:Mode$ CantBlockUnless | ValidCard$ Card.Self | Cost$ 2",
    "Subject can't block unless you pay {2}.",
    r#"{"BlockTax":{"blockers":["This"],"mana":"{2}"}}"#
)]
#[case::of_big_attackers(
    "S:Mode$ CantBlockUnless | ValidCard$ Card.Self | Attacker$ Creature.powerGE3 | Cost$ 1",
    "Subject can't block creatures with power 3 or greater unless you pay {1}.",
    r#"{"BlockTax":{"attackers":[{"Type":"Creature"},{"Compare":{"op":"Ge","stat":"Power","to":3}}],"blockers":["This"],"mana":"{1}"}}"#
)]
#[case::of_the_attached_attacker(
    "S:Mode$ CantBlockUnless | Attacker$ Creature.AttachedBy | Cost$ 3",
    "Enchanted creature can't be blocked unless defending player pays {3} for each creature they control that's blocking it.",
    r#"{"BlockTax":{"attackers":[{"Is":"Attached"}],"mana":"{3}"}}"#
)]
#[case::in_life(
    "S:Mode$ CantBlockUnless | ValidCard$ Creature.nonBlue | Cost$ PayLife<1>",
    "Nonblue creatures can't block unless their controller pays 1 life for each blocking creature they control.",
    r#"{"BlockTax":{"blockers":[{"Type":"Creature"},{"Not":{"Color":"Blue"}}],"life":1}}"#
)]
#[case::counted(
    "S:Mode$ CantBlockUnless | ValidCard$ Card.Self | Cost$ Y\nSVar:Y:Count$CardCounters.P1P1",
    "Subject can't block unless you pay {1} for each +1/+1 counter on it.",
    r#"{"BlockTax":{"blockers":["This"],"mana":"{1}","per":{"Counters":{"counter":"+1/+1","on":"This"}}}}"#
)]
#[case::attack_tax_counted(
    "S:Mode$ CantAttackUnless | ValidCard$ Creature | Target$ You | Cost$ X\nSVar:X:Count$Valid Enchantment.YouCtrl",
    "Creatures can't attack you unless their controller pays {X} for each creature they control that's attacking you, where X is the number of enchantments you control.",
    r#"{"AttackTax":{"attacked":"Player","mana":"{1}","per":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}}"#
)]
fn combat_taxes_read_as_their_rule(#[case] lines: &str, #[case] oracle: &str, #[case] rule: &str) {
    let (json, found) = card(lines, oracle);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""rule":{rule}"#)), "{json}");
}

#[rstest]
#[case::tapping_a_creature(
    "S:Mode$ CantBlockUnless | ValidCard$ Card.Self | Cost$ tapXType<1/Creature.!blocking>",
    "cost:Static.CantBlockUnless.tapXType<1/Creature.!blocking>"
)]
#[case::sacrificing_a_land(
    "S:Mode$ CantBlockUnless | ValidCard$ Card.Self | Cost$ Sac<1/Land>",
    "cost:Static.CantBlockUnless.Sac<1/Land>"
)]
fn other_block_costs_stay_gaps(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = card(lines, "Subject can't block unless you pay.");
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

#[rstest]
#[case::except_the_first(
    "Card.OppOwn",
    "False",
    r#"{"Drew":{"first_in_draw_step":false,"who":"Opponents"}}"#
)]
#[case::the_first(
    "Card.YouOwn",
    "True",
    r#"{"Drew":{"first_in_draw_step":true,"who":"You"}}"#
)]
fn draws_of_a_draw_step_read_as_the_pattern(
    #[case] card_owned: &str,
    #[case] first: &str,
    #[case] when: &str,
) {
    let (json, found) = card(
        &format!(
            "T:Mode$ Drawn | ValidCard$ {card_owned} | FirstCardInDrawStep$ {first} | TriggerZones$ Battlefield | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1"
        ),
        "Whenever a player draws, you gain 1 life.",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(&format!(r#""when":{when}"#)), "{json}");
}
