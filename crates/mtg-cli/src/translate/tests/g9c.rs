//! Goal: combat triggers and rules of round g9c read as the engine holds them: a block of a kind and a block by a kind
//! ("one or more" once, "an artifact creature" for each), any creature as no kind, and a kind without "one or more"
//! stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn trigger(line: &str, description: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:2/2\nT:Mode$ {line} | Execute$ TrigPump | TriggerDescription$ {description}\nSVar:TrigPump:DB$ Pump | Defined$ Self | NumAtt$ +1\n"
    )
}

#[rstest]
#[case::blocks_one_or_more(
    "Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature.Black",
    "Whenever CARDNAME blocks one or more black creatures, it gets +1/+0.",
    Some(json!({"Blocks": {"what": ["This"], "blocking": [{"Type": "Creature"}, {"Color": "Black"}]}}))
)]
#[case::blocks_any_creature(
    "Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature",
    "Whenever CARDNAME blocks, it gets +1/+0.",
    Some(json!({"Blocks": {"what": ["This"]}}))
)]
#[case::blocks_a_kind_each(
    "Blocks | ValidCard$ Card.Self | ValidBlocked$ Creature.Black",
    "Whenever CARDNAME blocks a black creature, it gets +1/+0.",
    None
)]
#[case::blocked_by_one_or_more(
    "AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature.Colorless",
    "Whenever CARDNAME becomes blocked by one or more colorless creatures, it gets +1/+0.",
    Some(json!({"BecomesBlocked": {"what": ["This"], "by": [{"Type": "Creature"}, "Colorless"], "once": true}}))
)]
#[case::blocked_by_each(
    "AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature.Artifact",
    "Whenever CARDNAME becomes blocked by an artifact creature, it gets +1/+0.",
    Some(json!({"BecomesBlocked": {"what": ["This"], "by": [{"Type": "Creature"}, {"Type": "Artifact"}]}}))
)]
#[case::blocked_by_any_creature(
    "AttackerBlocked | ValidCard$ Card.Self | ValidBlocker$ Creature",
    "Whenever CARDNAME becomes blocked, it gets +1/+0.",
    Some(json!({"BecomesBlocked": {"what": ["This"]}}))
)]
#[case::blocked_written_as_a_block(
    "Blocks | ValidCard$ Creature | ValidBlocked$ Card.Self",
    "Whenever CARDNAME becomes blocked, it gets +1/+0.",
    Some(json!({"BecomesBlocked": {"what": ["This"]}}))
)]
fn g9c_a_block_of_a_kind_reads_once_only_for_one_or_more(
    #[case] line: &str,
    #[case] description: &str,
    #[case] when: Option<Value>,
) {
    let text = trigger(&format!("{line} | TriggerZones$ Battlefield"), description);
    let Some(when) = when else {
        assert!(
            gaps(&text)
                .iter()
                .any(|gap| gap == "key:Trigger.Blocks.ValidBlocked"),
            "{:?}",
            gaps(&text)
        );
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Triggered"]["when"], when);
}
