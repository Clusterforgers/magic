//! Goal: piece P8 of the rules plan translates its constructs, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(lines: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\n{lines}\n")
}

#[rstest]
#[case::protection_color("Protection from black", json!({"Protection": [{"Color": "Black"}]}))]
#[case::affinity("Affinity:Artifact", json!({"Affinity": [{"Type": "Artifact"}]}))]
#[case::affinity_described(
    "Affinity:Creature.Artifact:artifact creature",
    json!({"Affinity": [{"Type": "Creature"}, {"Type": "Artifact"}]})
)]
#[case::basic_landcycling(
    "TypeCycling:Basic:2",
    json!({"Typecycling": {"find": [{"Type": "Land"}, {"Supertype": "Basic"}], "cost": "{2}"}})
)]
#[case::plainscycling(
    "TypeCycling:Plains:1 W",
    json!({"Typecycling": {"find": [{"Subtype": "Plains"}], "cost": "{1}{W}"}})
)]
#[case::disturb("Disturb:1 U", json!({"Disturb": "{1}{U}"}))]
#[case::harmonize("Harmonize:X R R", json!({"Harmonize": "{X}{R}{R}"}))]
#[case::offspring("Offspring:2", json!({"Offspring": "{2}"}))]
#[case::escape(
    "Escape:2 B ExileFromGrave<4/Card.Other/other>",
    json!({"Escape": [{"Mana": "{2}{B}"}, {"ExileFromGraveyard": {"filter": ["Other"], "count": 4}}]})
)]
#[case::bargain("Bargain", json!("Bargain"))]
#[case::background("Choose a Background", json!("ChooseABackground"))]
#[case::doctors("Doctor's companion", json!("DoctorsCompanion"))]
#[case::station("Station:8", json!("Station"))]
#[case::equip_legendary(
    "Equip:3:Creature.YouCtrl+Legendary:legendary creature",
    json!({"EquipTo": {"to": [{"Supertype": "Legendary"}], "cost": "{3}"}})
)]
#[case::equip_commander(
    "Equip:2:Creature.IsCommander+YouCtrl:commander",
    json!({"EquipTo": {"to": ["Commander"], "cost": "{2}"}})
)]
fn a_keyword_with_a_parameter_carries_it(#[case] line: &str, #[case] expected: Value) {
    let script = creature(&format!("K:{line}"));
    assert_eq!(
        abilities(&script),
        vec![json!({ "Keyword": expected })],
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::card("a card", "Card")]
#[case::fish("a tapped Fish", "TappedFish")]
#[case::treasure("a Treasure", "Treasure")]
fn a_gift_is_the_one_the_card_describes(#[case] words: &str, #[case] kind: &str) {
    let script = creature(&format!(
        "K:Gift\nSVar:GiftAbility:DB$ Draw | GiftDescription$ {words}"
    ));
    assert_eq!(abilities(&script), vec![json!({"Keyword": {"Gift": kind}})]);
}

#[test]
fn a_backup_keeps_its_number() {
    let script = creature("K:Backup:2:BackupAbility\nSVar:BackupAbility:DB$ Animate | Triggers$ T");
    assert_eq!(
        abilities(&script),
        vec![json!({"Keyword": {"Backup": {"count": 2}}})]
    );
}

#[rstest]
#[case::unknown_gift(
    "K:Gift\nSVar:GiftAbility:DB$ Draw | GiftDescription$ a Rhystic Study",
    "keyword:Gift-kind"
)]
#[case::protection_everything("K:Protection from everything", "keyword:Protection from everything")]
#[case::hexproof_from("K:Hexproof:Black", "keyword:Hexproof")]
#[case::equip_planeswalker(
    "K:Equip:2:Planeswalker.YouCtrl:planeswalker",
    "keyword:Equip-with-restriction"
)]
#[case::second_kicker("K:Kicker:1 B:G", "keyword:Kicker-with-restriction")]
#[case::other_companion(
    "K:Companion:Special:SharesCardType:Each nonland card shares a card type.",
    "keyword:Companion"
)]
fn a_shape_the_node_cannot_say_stays_a_named_gap(#[case] lines: &str, #[case] gap: &str) {
    let script = creature(lines);
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}
