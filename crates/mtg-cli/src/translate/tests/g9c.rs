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

const HUNT: &str = "Name:T\nManaCost:G\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | SubAbility$ DBMustBlock\nSVar:DBMustBlock:DB$ MustBlock | DefinedAttacker$ ParentTarget | ValidTgts$ Creature\n";
const HOOK: &str = "Name:T\nTypes:Artifact Equipment\nT:Mode$ Attacks | ValidCard$ Creature.EquippedBy | Execute$ TrigBlock | TriggerZones$ Battlefield | TriggerDescription$ Whenever equipped creature attacks, target creature blocks it this combat if able.\nSVar:TrigBlock:DB$ MustBlock | ValidTgts$ Creature | DefinedAttacker$ TriggeredAttackerLKICopy | Duration$ UntilEndOfCombat\n";
const LANDFALL: &str = "Name:T\nTypes:Creature\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Land.YouCtrl | Execute$ TrigBlock | TriggerZones$ Battlefield | TriggerDescription$ Whenever a land you control enters, target creature blocks CARDNAME this turn if able.\nSVar:TrigBlock:DB$ MustBlock | ValidTgts$ Creature\n";

#[rstest]
// The blocker is another target than the attacker, the first target.
#[case::parent_target(HUNT, json!({"Target": 0}), json!({"Target": 1}))]
#[case::the_attacker_of_the_trigger(HOOK, json!("Triggering"), json!({"Target": 0}))]
// Without an attacker named, it is the source, whatever the trigger.
#[case::the_source(LANDFALL, json!("This"), json!({"Target": 0}))]
fn g9c_a_must_block_names_its_attacker(
    #[case] text: &str,
    #[case] attacker: Value,
    #[case] who: Value,
) {
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = Value::Array(abilities(text)).to_string();
    let rule = json!({"MustBlockAttacker": {"attacker": attacker, "who": who}}).to_string();
    assert!(found.contains(&rule), "{found}");
    if who == json!({"Target": 1}) {
        assert!(found.contains(r#"{"Not":{"Is":{"Target":0}}}"#), "{found}");
    }
}

#[test]
fn g9c_a_must_block_of_another_attacker_word_is_a_gap() {
    let text = HOOK.replace("TriggeredAttackerLKICopy", "Remembered");
    assert!(
        gaps(&text)
            .iter()
            .any(|gap| gap == "key:MustBlock.DefinedAttacker=Remembered"),
        "{:?}",
        gaps(&text)
    );
}

fn shadow_wall(blocker: &str, description: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Wall\nPT:0/4\nS:Mode$ CanBlockIfShadow | ValidAttacker$ Creature.withShadow | ValidBlocker$ {blocker} | Description$ {description}\n"
    )
}

#[rstest]
#[case::as_though_it_had(
    "Card.Self",
    "This creature can block creatures with shadow as though it had shadow.",
    Some(json!({"Combat": {"who": "This", "quirk": "BlockAsThoughShadow"}}))
)]
#[case::as_though_they_had_none(
    "Creature.EnchantedBy",
    "Enchanted creature can block creatures with shadow as though they didn't have shadow.",
    Some(json!({"Combat": {"who": "Attached", "quirk": "BlockIgnoringShadow"}}))
)]
#[case::another_wording("Card.Self", "This creature can block shadow creatures.", None)]
fn g9c_a_block_of_shadow_follows_its_wording(
    #[case] blocker: &str,
    #[case] description: &str,
    #[case] rule: Option<Value>,
) {
    let text = shadow_wall(blocker, description);
    let Some(rule) = rule else {
        assert!(
            gaps(&text)
                .iter()
                .any(|gap| gap == "key:Static.CanBlockIfShadow.Description"),
            "{:?}",
            gaps(&text)
        );
        return;
    };
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Rule"]["rule"], rule);
}
