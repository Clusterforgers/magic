//! Goal: round 13 pack g13a reads the object facts several open cards shared: a third or an even share of an amount and
//! the difference of two, a search for one card of each of several kinds, fateful hour, a spell that targets a player,
//! mana from a kind of source spent on a spell, and a creature attacking alone or attacking a planeswalker; the shapes
//! beside them that mean something else stay gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// Asserts that the card reads every part of `expected` with no gap, or stays the gap `gap` when one is given.
fn reads(types: &str, lines: &str, expected: &[&str], gap: &str) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    let found = gaps(&text);
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    if gap.is_empty() {
        assert!(found.is_empty(), "{found:?} {json}");
        for part in expected {
            assert!(json.contains(part), "{part} in {json}");
        }
    } else {
        assert!(found.iter().any(|g| g.contains(gap)), "{found:?} {json}");
    }
}

const DRAW_X: &str = "A:SP$ Draw | NumCards$ X";

#[rstest]
#[case::a_third_rounded_up(
    "SVar:X:Count$YourLifeTotal/ThirdUp",
    r#"{"Quotient":{"by":3,"of":{"Life":"You"},"up":true}}"#,
    ""
)]
#[case::a_third_rounded_down(
    "SVar:X:Count$YourLifeTotal/ThirdDown",
    r#"{"Quotient":{"by":3,"of":{"Life":"You"},"up":false}}"#,
    ""
)]
#[case::divided_by_a_number(
    "SVar:X:Count$xPaid/DivideEvenlyDown.7",
    r#"{"Quotient":{"by":7,"of":"X","up":false}}"#,
    ""
)]
#[case::divided_by_a_count(
    "SVar:X:Count$xPaid/DivideEvenlyUp.Y\nSVar:Y:Count$Valid Creature.YouCtrl",
    r#"{"Quotient":{"by":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"of":"X","up":true}}"#,
    ""
)]
#[case::difference_of_two_parts(
    "SVar:X:SVar$Y/Abs\nSVar:Y:Count$YourLifeTotal/Minus.Z\nSVar:Z:Count$xPaid",
    r#"{"Difference":[{"Life":"You"},"X"]}"#,
    ""
)]
#[case::difference_from_a_number(
    "SVar:X:Count$YourLifeTotal/Minus.3/Abs",
    r#"{"Difference":[{"Life":"You"},3]}"#,
    ""
)]
#[case::absolute_value_of_no_subtraction("SVar:X:Count$YourLifeTotal/Abs", "", "amount:Count/Abs")]
#[case::divided_by_nothing_read("SVar:X:Count$xPaid/DivideEvenlyDown.Bogus", "", "amount")]
fn g13a_a_share_or_a_difference(#[case] svars: &str, #[case] part: &str, #[case] gap: &str) {
    reads("Sorcery", &format!("{DRAW_X}\n{svars}"), &[part], gap);
}

#[rstest]
#[case::double(
    "Double",
    r#"{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"Power"}}"#
)]
#[case::triple(
    "Triple",
    r#"{"ModifyPT":{"power":{"Times":{"by":2,"of":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}"#
)]
fn g13a_a_pump_doubles_or_triples_its_target(#[case] word: &str, #[case] part: &str) {
    let line = format!("A:SP$ Pump | ValidTgts$ Creature | NumAtt$ {word} | NumDef$ {word}");
    reads("Instant", &line, &[part], "");
}

const SEARCH: &str = "A:SP$ ChangeZone | Origin$ Library | Destination$ Hand";

#[rstest]
#[case::two_kinds(
    "ChangeType$ EACH Creature & Artifact",
    &[
        r#"{"Search":{"as":"found","filter":[{"Type":"Creature"}],"reveal":true}}"#,
        r#"{"Search":{"as":"found2","filter":[{"Type":"Artifact"},{"Not":{"Is":{"Bound":"found"}}}],"reveal":true}}"#,
        r#"{"Move":{"to":"Hand","what":{"Bound":"found"}}},{"Move":{"to":"Hand","what":{"Bound":"found2"}}},{"Shuffle":{}}"#,
    ],
    ""
)]
#[case::three_kinds_leave_out_each_card_found(
    "ChangeType$ EACH Card.White & Card.Blue & Card.Black",
    &[r#"{"Search":{"as":"found3","filter":[{"Color":"Black"},{"Not":{"Is":{"Bound":"found"}}},{"Not":{"Is":{"Bound":"found2"}}}],"reveal":true}}"#],
    ""
)]
#[case::two_of_each_is_a_gap("ChangeType$ EACH Creature & Land | ChangeNum$ 2", &[], "EACH-shape")]
#[case::one_kind_alone_is_a_gap("ChangeType$ EACH Creature", &[], "EACH-shape")]
#[case::remembered_is_a_gap(
    "ChangeType$ EACH Creature & Land | RememberChanged$ True",
    &[],
    "EACH-shape"
)]
fn g13a_a_search_for_one_card_of_each_kind(
    #[case] keys: &str,
    #[case] parts: &[&str],
    #[case] gap: &str,
) {
    reads("Sorcery", &format!("{SEARCH} | {keys}"), parts, gap);
}

#[rstest]
#[case::static_condition(
    "S:Mode$ Continuous | Affected$ Creature.YouCtrl+Other | AddPower$ 1 | Condition$ FatefulHour",
    r#""condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":5}}"#
)]
#[case::amount(
    "A:SP$ Draw | NumCards$ X\nSVar:X:Count$FatefulHour.5.2",
    r#"{"When":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":5}},"otherwise":2,"then":5}}"#
)]
fn g13a_fateful_hour_is_five_or_less_life(#[case] lines: &str, #[case] part: &str) {
    reads("Enchantment", lines, &[part], "");
}

fn cast_trigger(more: &str) -> String {
    format!(
        "T:Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | {more} | Execute$ D\nSVar:D:DB$ Draw"
    )
}

#[rstest]
#[case::an_opponent_or_their_creature(
    "TargetsValid$ Opponent,Creature.OppCtrl",
    r#"{"AnyOf":[{"TargetsPlayer":"Opponents"},{"Targeting":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}]}"#
)]
#[case::you_or_your_creature(
    "TargetsValid$ You,Creature.YouCtrl+inZoneBattlefield",
    r#"{"AnyOf":[{"TargetsPlayer":"You"},{"Targeting":[{"Type":"Creature"},{"ControlledBy":"You"},{"In":"Battlefield"}]}]}"#
)]
#[case::you_alone("TargetsValid$ You", r#"{"TargetsPlayer":"You"}"#)]
#[case::any_player("TargetsValid$ Player", r#"{"TargetsPlayer":"EachPlayer"}"#)]
#[case::objects_alone(
    "TargetsValid$ Creature.YouCtrl",
    r#"{"Targeting":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#
)]
#[case::mana_from_a_treasure(
    "ValidSA$ Spell.ManaFromTreasure",
    r#"{"SpentFrom":{"from":{"subtypes":["Treasure"]},"least":1}}"#
)]
#[case::three_mana_from_creatures(
    "ValidSA$ Spell.ManaFromCreature_3",
    r#"{"SpentFrom":{"from":{"types":["Creature"]},"least":3}}"#
)]
fn g13a_a_spell_cast_by_its_targets_and_its_mana(#[case] keys: &str, #[case] part: &str) {
    reads("Enchantment", &cast_trigger(keys), &[part], "");
}

#[rstest]
#[case::mana_from_an_unknown_kind("ValidSA$ Spell.ManaFromGoblin")]
#[case::mana_from_a_bad_count("ValidSA$ Spell.ManaFromCreature_x")]
fn g13a_mana_from_an_unread_kind_stays_a_gap(#[case] keys: &str) {
    let text = format!("Name:Subject\nTypes:Enchantment\n{}\n", cast_trigger(keys));
    assert!(!gaps(&text).is_empty());
}

#[rstest]
#[case::alone("Creature.attackingAlone", r#""AttackingAlone""#)]
#[case::you_or_your_planeswalker(
    "Creature.attackingYouOrYourPW",
    r#"{"AnyOf":[{"AttackingPlayer":"You"},{"AttackingPlaneswalkerOf":"You"}]}"#
)]
fn g13a_how_a_creature_attacks(#[case] valid: &str, #[case] part: &str) {
    let line = format!("A:AB$ Destroy | Cost$ T | ValidTgts$ {valid}");
    reads("Artifact", &line, &[part], "");
}
