//! Goal: the pieces of ep29 translate as the card prints them. A cost that removes every counter of a kind from the card (`SubCounter<All/KIND>`) counts the counters on it, and `SVar$CostCountersRemoved` after it is the number removed (`RemovedThisWay`), with its steps; a removal of X counters, of two kinds or none stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn hydra(cost: &str, x: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Hydra\nPT:1/1\nA:AB$ DealDamage | Cost$ {cost} | ValidTgts$ Any | NumDmg$ X | SpellDescription$ It deals damage to any target equal to the number of +1/+1 counters removed this way.\nSVar:X:{x}\n"
    )
}

#[rstest]
#[case::removed("SVar$CostCountersRemoved", json!({"RemovedThisWay": "+1/+1"}))]
#[case::twice(
    "SVar$CostCountersRemoved/Twice",
    json!({"Times": {"by": 2, "of": {"RemovedThisWay": "+1/+1"}}})
)]
fn ep29_counters_removed_this_way(#[case] x: &str, #[case] amount: serde_json::Value) {
    let script = hydra("T SubCounter<All/P1P1>", x);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    let ability = &found[0]["Activated"];
    assert_eq!(
        ability["cost"],
        json!(["Tap", {"RemoveCounters": {"counter": "+1/+1", "count": {"Counters": {"on": "This", "counter": "+1/+1"}}}}])
    );
    assert_eq!(ability["effects"][0]["DealDamage"]["amount"], amount);
}

#[rstest]
#[case::x_counters("T SubCounter<X/P1P1>")]
#[case::two_kinds("T SubCounter<All/P1P1> SubCounter<All/CHARGE>")]
#[case::no_removal("T")]
fn ep29_other_removals_stay_gaps(#[case] cost: &str) {
    let found = gaps(&hydra(cost, "SVar$CostCountersRemoved"));
    assert!(
        found.iter().any(|gap| gap.contains("CostCountersRemoved")),
        "{cost}: {found:?}"
    );
}

fn ghitu(mana: &str, extra: &str) -> String {
    format!(
        "Name:T\nManaCost:{mana}\nTypes:Sorcery\nK:MayFlashCost:{extra}\nA:SP$ GainLife | LifeAmount$ 2\n"
    )
}

#[rstest]
#[case::x_and_two_more("X R", "2", json!("{2}"))]
#[case::tapping(
    "4 B B",
    "tapXType<3/Creature.withFlying/creatures with flying>",
    json!("TapPermanents")
)]
fn ep29_flash_for_more_is_a_permission_of_the_card(
    #[case] mana: &str,
    #[case] extra: &str,
    #[case] additional: serde_json::Value,
) {
    let script = ghitu(mana, extra);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    let rule = &found[0]["Rule"];
    assert_eq!(rule["from"], json!("Hand"), "{found:?}");
    let permission = &rule["rule"]["MayCast"];
    assert_eq!(
        (&permission["what"], &permission["flash"]),
        (&json!(["This"]), &json!(true))
    );
    // The mana alone is written as a string; another part is named by its kind.
    let said = permission["additional"].to_string();
    let wanted = additional.as_str().unwrap_or_default();
    assert!(said.contains(wanted), "{said}");
}

fn wyll(line: &str, ignore: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nR:Event$ RollDice | {line} | ReplaceWith$ PlusRoll | Description$ x\nSVar:PlusRoll:DB$ ReplaceEffect | VarName$ Number | VarValue$ ReplaceCount$Number/Plus.1 | SubAbility$ IgnoreLowest\nSVar:IgnoreLowest:{ignore}\n"
    )
}

const IGNORE_LOWEST: &str =
    "DB$ ReplaceEffect | VarName$ Ignore | VarValue$ ReplaceCount$Ignore/Plus.1";

#[test]
fn ep29_an_extra_die_for_you() {
    let script = wyll("ActiveZones$ Battlefield | ValidPlayer$ You", IGNORE_LOWEST);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![json!({"Rule": {"rule": {"ExtraDie": "You"}}})]
    );
}

#[rstest]
#[case::another_player(
    "ActiveZones$ Battlefield | ValidPlayer$ Player.IsRemembered",
    IGNORE_LOWEST
)]
#[case::six_sided(
    "ActiveZones$ Battlefield | ValidPlayer$ You | ValidSides$ 6",
    IGNORE_LOWEST
)]
#[case::a_chosen_roll(
    "ActiveZones$ Battlefield | ValidPlayer$ You",
    "DB$ ReplaceEffect | VarName$ IgnoreChosen | VarType$ Map | VarKey$ You | VarValue$ 1"
)]
#[case::two_ignored(
    "ActiveZones$ Battlefield | ValidPlayer$ You",
    "DB$ ReplaceEffect | VarName$ Ignore | VarValue$ ReplaceCount$Ignore/Plus.2"
)]
fn ep29_other_dice_replacements_stay_gaps(#[case] line: &str, #[case] ignore: &str) {
    assert!(!gaps(&wyll(line, ignore)).is_empty());
}

#[rstest]
#[case::without_a_mana_cost(
    "Name:T\nTypes:Sorcery\nK:MayFlashCost:2\nA:SP$ GainLife | LifeAmount$ 2\n"
)]
#[case::behold(
    "Name:T\nManaCost:3 R\nTypes:Instant\nK:MayFlashCost:Behold<1/Creature.Dragon/Dragon>\nA:SP$ GainLife | LifeAmount$ 2\n"
)]
fn ep29_other_flash_for_more_stays_a_gap(#[case] script: &str) {
    assert!(
        gaps(script)
            .iter()
            .any(|gap| gap.starts_with("keyword:MayFlashCost") || gap.starts_with("valid")),
        "{:?}",
        gaps(script)
    );
}
