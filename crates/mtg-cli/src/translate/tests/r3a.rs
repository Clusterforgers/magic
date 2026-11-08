//! Goal: permanents that give others counters as they enter, prepare cards, soulbond pairs, saddled Mounts, devour, awaken and mayhem, and the color a Thriving land excludes read into the nodes the engine has; shapes outside them stay named gaps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn enters_with(counter: Value, count: Value, what: Value) -> Value {
    let mut found = json!({"Replacement": {"when": {"Enters": {}}, "by": {"WithCounters": {}}}});
    found["Replacement"]["when"]["Enters"]["what"] = what;
    found["Replacement"]["by"]["WithCounters"]["counter"] = counter;
    found["Replacement"]["by"]["WithCounters"]["count"] = count;
    found
}

#[rstest]
#[case::every_creature_you_control(
    "Creature.YouCtrl",
    "P1P1",
    "1",
    enters_with(json!("+1/+1"), json!(1), json!([{"Type": "Creature"}, {"ControlledBy": "You"}]))
)]
#[case::another_creature_two_counters(
    "Creature.Other+YouCtrl",
    "P1P1",
    "2",
    enters_with(json!("+1/+1"), json!(2), json!([{"Type": "Creature"}, "Other", {"ControlledBy": "You"}]))
)]
#[case::planeswalkers_get_loyalty(
    "Planeswalker.YouCtrl",
    "LOYALTY",
    "1",
    enters_with(json!("loyalty"), json!(1), json!([{"Type": "Planeswalker"}, {"ControlledBy": "You"}]))
)]
#[case::a_keyword_counter(
    "Creature.faceDown+YouCtrl",
    "Flying",
    "1",
    enters_with(
        json!({"Other": "flying"}),
        json!(1),
        json!([{"Type": "Creature"}, "FaceDown", {"ControlledBy": "You"}])
    )
)]
fn permanents_enter_with_extra_counters(
    #[case] valid: &str,
    #[case] kind: &str,
    #[case] num: &str,
    #[case] expected: Value,
) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nK:ETBReplacement:Other:AddExtraCounter:Mandatory:Battlefield:{valid}\nSVar:AddExtraCounter:DB$ PutCounter | ETB$ True | Defined$ ReplacedCard | CounterType$ {kind} | CounterNum$ {num}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text), vec![expected]);
}

#[test]
fn a_source_in_a_graveyard_works_from_there_and_a_count_is_the_sources() {
    let text = "Name:T\nTypes:Creature Elf\nPT:2/2\nK:ETBReplacement:Other:AddExtraCounter:Mandatory:Graveyard:Creature.Elf+YouCtrl\nSVar:AddExtraCounter:DB$ PutCounter | ETB$ True | Defined$ ReplacedCard | CounterType$ P1P1 | CounterNum$ X\nSVar:X:Count$CardToughness\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = abilities(text);
    assert_eq!(found[0]["Replacement"]["from"], json!("Graveyard"));
    assert_eq!(
        found[0]["Replacement"]["by"]["WithCounters"]["count"],
        json!({"Stat": {"of": "This", "stat": "Toughness"}})
    );
}

#[rstest]
#[case::the_entering_card_in_the_count(
    "CounterNum$ X",
    "SVar:X:ReplacedCard$CastTotalManaSpent Artifact",
    "amount:ReplacedCard"
)]
#[case::a_second_part(
    "CounterNum$ 1 | SubAbility$ DBAnimate",
    "SVar:DBAnimate:DB$ Animate | Types$ Mutant",
    "key:ETBReplacement.SubAbility"
)]
#[case::a_condition(
    "CounterNum$ 1 | ConditionCheckSVar$ X",
    "SVar:X:Count$Valid Land.YouCtrl",
    "key:ETBReplacement.ConditionCheckSVar"
)]
fn a_counter_part_the_reader_cannot_keep_whole_is_a_gap(
    #[case] fields: &str,
    #[case] extra: &str,
    #[case] gap: &str,
) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nK:ETBReplacement:Other:AddExtraCounter:Mandatory:Battlefield:Creature.YouCtrl\nSVar:AddExtraCounter:DB$ PutCounter | ETB$ True | Defined$ ReplacedCard | CounterType$ P1P1 | {fields}\n{extra}\n"
    );
    let found = gaps(&text);
    assert!(found.iter().any(|name| name == gap), "{found:?}");
}

#[rstest]
#[case::plain(
    "DB$ AlterAttribute | Attributes$ Prepared | SpellDescription$ This creature enters prepared.",
    true
)]
#[case::on_a_condition(
    "DB$ AlterAttribute | Attributes$ Prepared | ConditionPresent$ Creature.YouOwn | ConditionCompare$ GE3 | ConditionZone$ Graveyard",
    false
)]
#[case::another_attribute("DB$ AlterAttribute | Attributes$ Suspected", false)]
fn a_card_enters_prepared_only_without_a_condition(#[case] part: &str, #[case] reads: bool) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:2/2\nK:ETBReplacement:Other:DBPrepare\nSVar:DBPrepare:{part}\n"
    );
    if reads {
        assert_eq!(gaps(&text), Vec::<String>::new());
        assert_eq!(
            abilities(&text),
            vec![
                json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": "EntersPrepared"}})
            ]
        );
    } else {
        assert_ne!(gaps(&text), Vec::<String>::new());
    }
}

#[rstest]
#[case::both_creatures("Creature.PairedWith,Creature.Self+Paired")]
#[case::card_self("Creature.PairedWith,Card.Self+Paired")]
fn a_soulbond_line_affects_the_pair_while_paired(#[case] affected: &str) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:2/2\nK:Soulbond\nS:Mode$ Continuous | Affected$ {affected} | AddKeyword$ Reach | Description$ x\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[1],
        json!({"Static": {
            "affected": {"Each": [{"AnyOf": ["This", {"PairedWith": "This"}]}]},
            "mods": [{"AddKeyword": "Reach"}],
            "condition": {"Matches": {"subject": "This", "selector": ["Paired"]}}}})
    );
}

#[test]
fn a_saddled_attacker_is_a_mount_with_the_designation() {
    let text = "Name:T\nTypes:Creature Elf\nPT:2/2\nT:Mode$ Attacks | ValidCard$ Card.Self+IsSaddled | Execute$ TrigDraw | TriggerDescription$ x\nSVar:TrigDraw:DB$ Draw\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        abilities(text)[0]["Triggered"]["when"]["Attacks"]["what"],
        json!(["This", {"HasKeyword": "Saddled"}])
    );
}

#[test]
fn the_creatures_that_saddled_it_this_turn_are_the_mounts_saddlers() {
    let text = "Name:T\nTypes:Creature Elf\nPT:2/2\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigCounter\nSVar:TrigCounter:DB$ PutCounter | ValidTgts$ Creature.SaddledThisTurn | CounterType$ P1P1\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        abilities(text)[0]["Triggered"]["targets"][0]["what"],
        json!({"Object": [{"Type": "Creature"}, {"Saddled": "This"}]})
    );
}

#[rstest]
#[case::mayhem("Mayhem:2 R", json!({"Mayhem": "{2}{R}"}))]
#[case::awaken("Awaken:3:4 U", json!({"Awaken": {"count": 3, "cost": "{4}{U}"}}))]
#[case::devour_creatures("Devour:2", json!({"Devour": {"counters": 2, "of": [{"Type": "Creature"}]}}))]
#[case::devour_lands("Devour:3:Land", json!({"Devour": {"counters": 3, "of": [{"Type": "Land"}]}}))]
#[case::devour_food("Devour:3:Food", json!({"Devour": {"counters": 3, "of": [{"Subtype": "Food"}]}}))]
fn mayhem_awaken_and_devour_are_keywords(#[case] line: &str, #[case] keyword: Value) {
    let text = format!("Name:T\nTypes:Creature Elf\nPT:2/2\nK:{line}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text), vec![json!({"Keyword": keyword})]);
}

#[rstest]
#[case::devour_x("Devour:X::, where X is the number of creatures", "keyword:Devour")]
#[case::devour_a_kind("Devour:2:Planeswalker", "keyword:Devour-kind")]
#[case::awaken_x("Awaken:X:4 U", "keyword:Awaken-shape")]
#[case::mayhem_equal_to_the_mana_cost("Mayhem:CardManaCost", "")]
fn shapes_outside_the_keywords_are_gaps(#[case] line: &str, #[case] gap: &str) {
    let text = format!("Name:T\nTypes:Creature Elf\nPT:2/2\nK:{line}\n");
    let found = gaps(&text);
    assert_ne!(found, Vec::<String>::new());
    if !gap.is_empty() {
        assert!(found.iter().any(|name| name == gap), "{found:?}");
    }
}

#[test]
fn the_creatures_a_permanent_devoured_are_counted() {
    let text = "Name:T\nTypes:Creature Elf\nPT:2/2\nK:Devour:2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigGain\nSVar:TrigGain:DB$ GainLife | Defined$ You | LifeAmount$ X\nSVar:X:Count$Devoured Card/Twice\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert_eq!(
        abilities(text)[1]["Triggered"]["effects"][0],
        json!({"GainLife": {"amount": {"Times": {"by": 2, "of": "Devoured"}}}})
    );
}

#[rstest]
#[case::red("red", Some("Red"))]
#[case::black("black", Some("Black"))]
#[case::unknown("purple", None)]
fn a_thriving_land_excludes_its_color(#[case] color: &str, #[case] named: Option<&str>) {
    let text = format!(
        "Name:T\nTypes:Land\nK:ETBReplacement:Other:ChooseColor\nSVar:ChooseColor:DB$ ChooseColor | Defined$ You | Exclude$ {color}\n"
    );
    match named {
        Some(color) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(
                abilities(&text)[0]["Replacement"]["by"],
                json!({"Choose": {"ColorOtherThan": color}})
            );
        }
        None => assert_ne!(gaps(&text), Vec::<String>::new()),
    }
}
