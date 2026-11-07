//! Goal: round 14 pack e14b reads Forge's "pay as it would enter or it goes to the graveyard" chain (Lotus Vale, Kjeldoran Outpost, Mox Diamond), "X can't be 0" (`XMin1`) and the unattaching of the Equipment that gave an ability; a chain that compares another count, moves elsewhere or has no cleanup stays a gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const LINE: &str =
    "R:Event$ Moved | Destination$ Battlefield | ValidCard$ Card.Self | ReplaceWith$ PayBeforeETB";

/// A land with the replacement line and the chain `pay`, then the two moves comparing with `fewer` and `all`.
fn land(pay: &str, fewer: &str, all: &str, to: &str) -> String {
    format!(
        "Name:T\nTypes:Land\n{LINE}\nSVar:PayBeforeETB:{pay} | SubAbility$ MoveToGraveyard\nSVar:MoveToGraveyard:DB$ ChangeZone | Origin$ All | Destination$ {to} | Defined$ ReplacedCard | ConditionCheckSVar$ X | ConditionSVarCompare$ {fewer} | SubAbility$ MoveToBattlefield\nSVar:MoveToBattlefield:DB$ ChangeZone | Origin$ All | Destination$ Battlefield | Defined$ ReplacedCard | ConditionCheckSVar$ X | ConditionSVarCompare$ {all} | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount\n"
    )
}

fn paying(by: Value) -> Value {
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"PayOrGraveyard": by}}})
}

const TWO_LANDS: &str = "DB$ Sacrifice | SacValid$ Land.untapped | Defined$ You | RememberSacrificed$ True | Amount$ 2 | StrictAmount$ True";
const A_PLAINS: &str = "DB$ Sacrifice | SacValid$ Plains | Defined$ You | RememberSacrificed$ True";
const A_LAND_CARD: &str = "DB$ Discard | DiscardValid$ Land | Mode$ TgtChoose | RememberDiscarded$ True | Optional$ True | AILogic$ Always";

#[rstest]
#[case::two_untapped_lands(TWO_LANDS, "LT2", "GE2", paying(json!({"cost": [{"Sacrifice": {"count": 2, "filter": [{"Type": "Land"}, "Untapped"]}}]})))]
#[case::a_plains(A_PLAINS, "EQ0", "EQ1", paying(json!({"cost": [{"Sacrifice": {"filter": [{"Subtype": "Plains"}]}}]})))]
#[case::a_land_card(A_LAND_CARD, "EQ0", "EQ1", paying(json!({"cost": [{"Discard": {"filter": [{"Type": "Land"}]}}], "optional": true})))]
fn a_permanent_paid_for_as_it_would_enter(
    #[case] pay: &str,
    #[case] fewer: &str,
    #[case] all: &str,
    #[case] expected: Value,
) {
    let text = land(pay, fewer, all, "Graveyard");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0], expected);
}

#[rstest]
#[case::another_count(TWO_LANDS, "EQ0", "EQ1", "Graveyard")]
#[case::exiled_instead(A_PLAINS, "EQ0", "EQ1", "Exile")]
#[case::a_discard_that_must_be_paid(
    "DB$ Discard | DiscardValid$ Land | Mode$ TgtChoose | RememberDiscarded$ True",
    "EQ0",
    "EQ1",
    "Graveyard"
)]
#[case::an_opponent_sacrifices(
    "DB$ Sacrifice | SacValid$ Plains | Defined$ Opponent | RememberSacrificed$ True",
    "EQ0",
    "EQ1",
    "Graveyard"
)]
fn another_chain_stays_a_gap(
    #[case] pay: &str,
    #[case] fewer: &str,
    #[case] all: &str,
    #[case] to: &str,
) {
    let text = land(pay, fewer, all, to);
    let found = gaps(&text);
    assert!(
        found.iter().any(|gap| gap.contains("PayOrGraveyard")),
        "{found:?}"
    );
}

#[test]
fn x_that_cannot_be_0_is_a_part_of_the_cost() {
    let text = "Name:T\nTypes:Creature\nPT:1/1\nA:AB$ GainLife | Cost$ X G XMin1 | LifeAmount$ X | SpellDescription$ You gain X life. X can't be 0.\nSVar:X:Count$xPaid\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let cost = &abilities(text)[0]["Activated"]["cost"];
    assert!(
        cost.as_array().unwrap().contains(&json!({"XAtLeast": 1})),
        "{cost}"
    );
}

#[rstest]
#[case::granted(
    "S:Mode$ Continuous | Affected$ Creature.EquippedBy | AddAbility$ Seek\nSVar:Seek:AB$ Destroy | Cost$ T Unattach<OriginalHost/T> | ValidTgts$ Creature | SpellDescription$ Destroy target creature.",
    true
)]
fn the_equipment_that_gave_the_ability_is_unattached(#[case] lines: &str, #[case] read: bool) {
    let text = format!("Name:T\nTypes:Artifact Equipment\nK:Equip:2\n{lines}\n");
    let found = gaps(&text);
    assert_eq!(found.is_empty(), read, "{found:?}");
    let all = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(all.contains(r#"["Tap","Unattach"]"#), "{all}");
}
