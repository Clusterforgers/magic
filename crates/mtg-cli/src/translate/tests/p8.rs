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

#[rstest]
#[case::hybrid("Cycling:WB", json!({"Cycling": "{W/B}"}))]
#[case::hybrid_in_cost("Cycling:1 UR UR", json!({"Cycling": "{1}{U/R}{U/R}"}))]
#[case::phyrexian("Flashback:3 GP", json!({"Flashback": "{3}{G/P}"}))]
fn a_hybrid_or_phyrexian_word_is_its_symbol(#[case] line: &str, #[case] expected: Value) {
    let script = creature(&format!("K:{line}"));
    assert_eq!(
        abilities(&script),
        vec![json!({ "Keyword": expected })],
        "{:?}",
        gaps(&script)
    );
    let symbols = expected.as_object().and_then(|map| map.values().next());
    let text = symbols.and_then(Value::as_str).unwrap_or_default();
    assert!(text.parse::<mtg_cards::mana::ManaCostX>().is_ok(), "{text}");
}

#[test]
fn returning_the_source_is_a_cost() {
    let script = creature("A:AB$ Draw | Cost$ T Return<1/CARDNAME> | NumCards$ 1\n");
    let found = abilities(&script);
    assert_eq!(
        found[0]["Activated"]["cost"],
        json!(["Tap", {"ReturnToHand": {"filter": ["This"]}}]),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::mandatory(
    "A:AB$ Draw | Cost$ Mandatory Sac<1/CARDNAME> | NumCards$ 1",
    "cost:Mandatory"
)]
#[case::x_at_least_one("A:AB$ Draw | Cost$ XMin1 X T | NumCards$ 1", "cost:XMin1")]
#[case::unless_y("K:Cycling:Y", "cost:Y")]
fn a_word_that_changes_who_decides_stays_a_named_gap(#[case] lines: &str, #[case] gap: &str) {
    let script = creature(lines);
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

#[test]
fn an_alternate_additional_cost_is_one_of_two_costs() {
    let script = "Name:T\nTypes:Instant\nK:AlternateAdditionalCost:PayLife<3>:Discard<1/Card>\n";
    let found = abilities(script);
    let expected = json!({"AdditionalCost": {
        "cost": [{"OneOf": [[{"PayLife": 3}], [{"Discard": {}}]]}], "required": true}});
    assert_eq!(found, vec![expected.clone()], "{:?}", gaps(script));
    let ability: mtg_cards::ability::Ability = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(ability).unwrap(), expected);
}

#[test]
fn an_alternate_additional_cost_with_an_unknown_cost_is_a_gap() {
    let script = "Name:T\nTypes:Instant\nK:AlternateAdditionalCost:Quux<1>:2\n";
    assert!(abilities(script).is_empty());
    assert!(gaps(script).iter().any(|gap| gap == "cost:Quux"), "{:?}", gaps(script));
}

#[test]
#[ignore]
fn scratch_survey() {
    use std::collections::BTreeMap;
    let dir = std::path::Path::new(&std::env::var("HOME").unwrap())
        .join(".cache/mtg-cli/forge/forge-gui/res/cardsfolder");
    let prefix = std::env::var("SURVEY").unwrap_or_default();
    let mut lines: BTreeMap<String, usize> = BTreeMap::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(t) = std::fs::read_to_string(&p) {
                for l in t.lines() {
                    if let Some(k) = l.strip_prefix(&std::env::var("LINE").unwrap_or("K:".into())) {
                        if k.starts_with(&prefix) {
                            *lines.entry(k.to_owned()).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    let mut tally: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for (k, n) in &lines {
        let script = format!(
            "Name:T\nTypes:Creature Elf\n{}{k}\n",
            std::env::var("LINE").unwrap_or("K:".into())
        );
        for g in super::gaps(&script) {
            let e = tally.entry(g).or_default();
            e.0 += n;
            if e.1.is_empty() {
                e.1 = k.clone();
            }
        }
    }
    let mut v: Vec<_> = tally.into_iter().collect();
    v.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for (g, (n, ex)) in v {
        println!("{n:5} {g}  e.g. {ex}");
    }
}
