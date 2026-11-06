//! Goal: the shapes the fix pass 6 found wrong are read to the rules the words say or stay named gaps: counters "you put" are caused by you, a colour change of a spell on the stack and mana counted by its source kind are gaps.
use rstest::rstest;

use super::{abilities, gaps};

fn kate(description: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Human\nPT:1/1\nT:Mode$ CounterAddedOnce | ValidCard$ Permanent.YouCtrl | CounterType$ TIME | Execute$ Tok | TriggerDescription$ {description}\nSVar:Tok:DB$ Token | TokenScript$ w_1_1_soldier\n"
    )
}

#[rstest]
#[case::you_put(
    "Whenever you put one or more time counters on a permanent you control, create a token.",
    true
)]
#[case::any_player(
    "Whenever one or more time counters are put on a permanent you control, create a token.",
    false
)]
fn counters_you_put_are_caused_by_you(#[case] description: &str, #[case] caused: bool) {
    let text = kate(description);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert_eq!(json.contains(r#""Caused":{"by":"You""#), caused, "{json}");
}

#[rstest]
#[case::spell_or_permanent(
    "SP$ Animate | Colors$ Red | OverwriteColors$ True | ValidTgts$ Card | TgtZone$ Stack,Battlefield | Duration$ Permanent",
    true
)]
#[case::permanent_only(
    "SP$ Animate | Colors$ Red | OverwriteColors$ True | ValidTgts$ Creature | Duration$ Permanent",
    false
)]
fn a_colour_change_of_a_spell_is_a_gap(#[case] line: &str, #[case] gap: bool) {
    let text = format!("Name:T\nTypes:Instant\nA:{line}\n");
    let expected = if gap {
        vec!["key:Animate.TgtZone=Stack".to_owned()]
    } else {
        Vec::new()
    };
    assert_eq!(gaps(&text), expected, "{:?}", abilities(&text));
}

#[rstest]
#[case::treasure(
    "Count$CastTotalManaSpent Treasure",
    "amount:Count$CastTotalManaSpent-pool-mana"
)]
#[case::snow(
    "Count$CastTotalManaSpent Snow",
    "amount:Count$CastTotalManaSpent-pool-mana"
)]
#[case::unknown(
    "Count$CastTotalManaSpent Wibble",
    "amount:Count$CastTotalManaSpent-source"
)]
fn mana_counted_by_source_is_a_gap_while_the_pool_keeps_no_source(
    #[case] x: &str,
    #[case] gap: &str,
) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:{x}\n");
    assert_eq!(gaps(&text), vec![gap.to_owned()]);
}
