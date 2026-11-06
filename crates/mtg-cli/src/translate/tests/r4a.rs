//! Goal: equip with a once-a-turn limit, a second cost or a reduction reads into the activated ability of equip; shapes outside them stay named gaps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn equipment(line: &str) -> String {
    format!("Name:Test Blade\nTypes:Artifact Equipment\nManaCost:2\n{line}\n")
}

fn attach(cost: &Value, limit: bool) -> Value {
    let mut found = json!({"Activated": {"cost": cost, "timing": "Sorcery",
        "targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}],
        "effects": [{"Attach": {"what": "This", "to": {"Target": 0}}}]}});
    if limit {
        found["Activated"]["limit"] = json!("OncePerTurn");
    }
    found
}

#[rstest]
#[case::limit_with_mana("K:Equip:0:::ActivationLimit$ 1:Activate only once each turn", json!("{0}"), true)]
#[case::alternative_mana("K:Equip:2:::AlternateCost$ B", json!("{2}"), false)]
fn equip_options(#[case] line: &str, #[case] cost: Value, #[case] limit: bool) {
    let text = equipment(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0], attach(&cost, limit));
}

#[test]
fn equip_with_a_second_cost_is_two_abilities() {
    let found = abilities(&equipment("K:Equip:2:::AlternateCost$ B"));
    assert_eq!(
        found,
        vec![attach(&json!("{2}"), false), attach(&json!("{B}"), false)]
    );
}

#[rstest]
#[case::fixed_reduction("K:Equip:3:::ReduceCost$ 1:less", "keyword:Equip-fixed-reduction")]
#[case::unknown_option("K:Equip:3:::Foo$ 1:less", "key:Equip.Foo")]
#[case::restriction_and_option(
    "K:Equip:3:Creature.Pirate:::ActivationLimit$ 1",
    "keyword:Equip-restriction-with-options"
)]
fn equip_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&equipment(line)), vec![gap.to_owned()]);
}

const LANCER: &str = "Name:Test Lancer\nTypes:Creature Human Knight\nManaCost:2 W\nPT:2/2\nK:Kicker:2 W\nK:ETBReplacement:Other:DBPutCounter:Mandatory::Card.Self+kicked\nSVar:DBPutCounter:DB$ PutCounter | ETB$ True | Defined$ Self | CounterType$ P1P1 | CounterNum$ 2 | SubAbility$ DBPump\nSVar:DBPump:DB$ Animate | Defined$ Self | Keywords$ First Strike | Duration$ Permanent\n";

const VOLVER: &str = "Name:Test Volver\nTypes:Creature Volver\nManaCost:3 G\nPT:2/2\nK:Kicker:1 U:B\nK:ETBReplacement:Other:Strength:Mandatory::Card.Self+kicked 1\nK:ETBReplacement:Other:Pumped:Mandatory::Card.Self+kicked 2\nSVar:Strength:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 2 | ETB$ True | SubAbility$ Launch\nSVar:Launch:DB$ Animate | Defined$ Self | Keywords$ Flying | Duration$ Permanent\nSVar:Pumped:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1 | ETB$ True\n";

fn replacement(when: &Value, by: &Value) -> Value {
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": by, "condition": when}})
}

#[test]
fn a_kicked_permanent_enters_with_counters_and_a_keyword() {
    let by = json!({"WithCountersAnd": {"counter": "+1/+1", "count": 2,
        "mods": [{"AddKeyword": "FirstStrike"}]}});
    assert_eq!(gaps(LANCER), Vec::<String>::new());
    assert!(abilities(LANCER).contains(&replacement(&json!("Kicked"), &by)));
}

#[test]
fn each_kicker_of_two_has_its_own_entering() {
    let first = json!({"WithCountersAnd": {"counter": "+1/+1", "count": 2,
        "mods": [{"AddKeyword": "Flying"}]}});
    let second = json!({"WithCounters": {"counter": "+1/+1", "count": 1}});
    let found = abilities(VOLVER);
    assert_eq!(gaps(VOLVER), Vec::<String>::new());
    assert!(found.contains(&replacement(&json!({"KickedWith": "{1}{U}"}), &first)));
    assert!(found.contains(&replacement(&json!({"KickedWith": "{B}"}), &second)));
}

#[rstest]
#[case::one_type("Goblin", json!([{"Subtype": "Goblin"}, "Other", {"ControlledBy": "You"}]))]
#[case::any_creature("Creature", json!([{"Type": "Creature"}, "Other", {"ControlledBy": "You"}]))]
fn champion_exiles_another_creature_or_is_sacrificed(#[case] types: &str, #[case] choices: Value) {
    let text =
        format!("Name:Test Mob\nTypes:Creature Goblin\nManaCost:3 B\nPT:3/3\nK:Champion:{types}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text),
        vec![json!({"Triggered": {
            "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
            "effects": [
                {"Choose": {"from": {"Each": choices}, "count": {"min": 0, "max": 1}, "as": "champion"}},
                {"Exile": {"what": {"Bound": "champion"}, "duration": "WhileSourceOnBattlefield"}},
                {"If": {"condition": {"Not": "Performed"}, "then": [{"Sacrifice": {"filter": ["This"]}}]}}]}})]
    );
}

#[test]
fn tribute_asks_an_opponent_and_triggers_when_unpaid() {
    let text = "Name:Test Bones\nTypes:Creature Minotaur\nPT:3/1\nK:Tribute:2\nSVar:TrigNotTribute:DB$ Pump | Defined$ Self | NumAtt$ +1 | NumDef$ +1 | KW$ Haste\n";
    let found = abilities(text);
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        found[0],
        json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"Tribute": 2}}})
    );
    assert_eq!(
        found[1]["Triggered"]["if"],
        json!({"Compare": {"left": "Chosen", "op": "Eq", "right": 0}})
    );
    assert_eq!(
        found[1]["Triggered"]["when"],
        json!({"ZoneChange": {"what": ["This"], "to": "Battlefield"}})
    );
}

#[rstest]
#[case::one("1", Some(1))]
#[case::three("3", Some(3))]
#[case::zero("0", None)]
#[case::x("X", None)]
fn amplify_enters_with_counters_for_the_cards_revealed(
    #[case] number: &str,
    #[case] counters: Option<u32>,
) {
    let text =
        format!("Name:Test Warhawk\nTypes:Creature Bird Soldier\nPT:2/2\nK:Amplify:{number}\n");
    match counters {
        Some(count) => assert_eq!(
            abilities(&text),
            vec![
                json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"Amplify": count}}})
            ]
        ),
        None => assert_eq!(gaps(&text), vec!["keyword:Amplify-shape".to_owned()]),
    }
}

#[test]
fn tribute_without_a_number_is_a_gap() {
    let text = "Name:Test Bones\nTypes:Creature Minotaur\nPT:3/1\nK:Tribute:X\n";
    assert_eq!(gaps(text), vec!["keyword:Tribute-shape".to_owned()]);
}

#[test]
fn champion_of_an_unknown_type_is_a_gap() {
    let text = "Name:Test Mob\nTypes:Creature Goblin\nPT:3/3\nK:Champion:Zorp\n";
    assert_eq!(gaps(text), vec!["keyword:Champion-type".to_owned()]);
}

#[rstest]
#[case::kicker_out_of_range("Card.Self+kicked 3", "keyword:ETBReplacement-kicker-number")]
#[case::another_way("Card.Self+foretold", "keyword:ETBReplacement-shape")]
fn entering_with_gaps(#[case] valid: &str, #[case] gap: &str) {
    let text = VOLVER.replace("Card.Self+kicked 2", valid);
    assert_eq!(gaps(&text), vec![gap.to_owned()]);
}
