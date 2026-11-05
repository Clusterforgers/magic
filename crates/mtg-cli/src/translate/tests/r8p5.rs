//! Goal: round 8 piece 5 closes key families with one rule each: a flip, a delayed clean-up after an animation, support counters and a death sent to exile; a shape beyond them stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {body}\n")
}

#[rstest]
#[case::flip("SetState | Defined$ Self | Mode$ Flip", json!({"Flip": {"what": "This"}}))]
#[case::transform("SetState | Mode$ Transform", json!({"Transform": {"what": "This"}}))]
fn a_state_change_maps_to_its_node(#[case] line: &str, #[case] node: Value) {
    let text = spell(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Spell"]["effects"][0], node);
}

#[rstest]
#[case::face_up("SetState | Defined$ Self | Mode$ TurnFaceUp", "key:SetState.Mode")]
#[case::unspecialize("SetState | Defined$ Self | Mode$ Unspecialize", "key:SetState.Mode")]
#[case::fly_flashback(
    "Pump | ValidTgts$ Instant | KW$ Flashback | PumpZone$ Graveyard",
    "key:Pump.PumpZone=Graveyard"
)]
#[case::remembered_dying(
    "DealDamage | ValidTgts$ Creature | NumDmg$ 2 | ReplaceDyingDefined$ Remembered",
    "key:ReplaceDyingDefined=Remembered"
)]
fn a_shape_beyond_the_rule_is_a_named_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::damage("DealDamage | ValidTgts$ Creature | NumDmg$ 4 | ReplaceDyingDefined$ Targeted")]
#[case::damage_any(
    "DealDamage | ValidTgts$ Any | NumDmg$ 3 | ReplaceDyingDefined$ ThisTargetedCard.Creature"
)]
#[case::pump(
    "Pump | ValidTgts$ Creature | NumAtt$ -13 | NumDef$ -13 | ReplaceDyingDefined$ Targeted"
)]
fn a_creature_that_would_die_goes_to_exile(#[case] line: &str) {
    let text = spell(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Spell"]["effects"];
    assert_eq!(
        effects[1],
        json!({"Shield": {
            "when": {"ZoneChange": {
                "what": [{"Is": {"Target": 0}}, {"Type": "Creature"}],
                "from": "Battlefield", "to": "Graveyard"}},
            "by": {"MoveTo": "Exile"}, "duration": "UntilEndOfTurn"}})
    );
}

#[rstest]
#[case::two("2", 2)]
#[case::six("6", 6)]
fn support_puts_a_counter_on_each_of_up_to_n_other_creatures(#[case] n: &str, #[case] max: u64) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Any | Destination$ Battlefield | Execute$ P\nSVar:P:DB$ PutCounter | Support${n}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = &abilities(&text)[0]["Triggered"];
    assert_eq!(found["targets"][0]["bounds"]["max"], max, "{found}");
    assert_eq!(found["effects"][0]["PutCounters"]["counter"], "+1/+1");
}

#[rstest]
#[case::sacrifice("Sacrifice")]
#[case::hand("Hand")]
fn an_animation_with_a_clean_up_sets_a_delayed_trigger(#[case] word: &str) {
    let text = format!(
        "Name:T\nTypes:Artifact\nA:AB$ Animate | Cost$ 1 | Defined$ Self | Power$ 2 | Toughness$ 2 | Types$ Creature | Duration$ Permanent | AtEOT${word}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Activated"]["effects"];
    assert!(effects[1]["DelayedTrigger"].is_object(), "{effects}");
}

#[rstest]
#[case::exile("Exile", true)]
#[case::graveyard("Graveyard", false)]
fn a_counterspell_that_exiles_sets_a_shield_before_it_counters(
    #[case] zone: &str,
    #[case] shield: bool,
) {
    let text = spell(&format!(
        "Counter | TargetType$ Spell | ValidTgts$ Card | Destination${zone}"
    ));
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = abilities(&text)[0]["Spell"]["effects"].clone();
    assert_eq!(effects[0].get("Shield").is_some(), shield, "{effects}");
    assert!(
        effects
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .get("Counter")
            .is_some()
    );
}
