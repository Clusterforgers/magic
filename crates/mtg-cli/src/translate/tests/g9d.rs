//! Goal: a look at the top cards may end in a shuffle the controller may have, a group or a pick exiled until the source
//! leaves lasts while it stays, a move to the battlefield takes its end-step clean-up and an added color, a shuffle-in of
//! up to some targets needs no shuffle option, and counters numbered per object read each object; other shapes stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn effects(script: &str) -> Value {
    abilities(script)[0]["Spell"]["effects"].clone()
}

fn has_gap(script: &str, gap: &str) -> bool {
    gaps(script).iter().any(|found| found == gap)
}

#[rstest]
#[case::yours("Defined$ You", json!({}))]
#[case::target_player("ValidTgts$ Player", json!({"who": {"Target": 0}}))]
fn a_look_may_end_in_a_shuffle_of_that_library(#[case] whose: &str, #[case] shuffle: Value) {
    let script = spell(&format!(
        "RearrangeTopOfLibrary | {whose} | NumCards$ 3 | MayShuffle$ True"
    ));
    let found = effects(&script);
    assert_eq!(
        found[0]["LookAtTop"].get("who"),
        shuffle.get("who"),
        "{:?}",
        gaps(&script)
    );
    assert_eq!(
        found[2],
        json!({"May": {"effects": [{"Shuffle": shuffle}]}})
    );
    assert_eq!(found.as_array().map(Vec::len), Some(3));
}

#[test]
fn a_look_without_the_shuffle_has_no_may() {
    let found = effects(&spell("RearrangeTopOfLibrary | Defined$ You | NumCards$ 3"));
    assert_eq!(found.as_array().map(Vec::len), Some(2));
}

#[rstest]
#[case::group(
    "ChangeZoneAll | Origin$ Battlefield | Destination$ Exile | ChangeType$ Artifact.OppCtrl | Duration$ UntilHostLeavesPlay",
    "/0/Exile/duration"
)]
#[case::picked(
    "ChangeZone | Origin$ Battlefield | Destination$ Exile | Hidden$ True | ChangeType$ Creature.YouCtrl | ChangeNum$ X | Duration$ UntilHostLeavesPlay",
    "/1/Exile/duration"
)]
fn an_exile_until_the_source_leaves_lasts_while_it_stays(#[case] line: &str, #[case] at: &str) {
    let script = format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ E\nSVar:E:DB$ {line}\nSVar:X:Count$Valid Creature.YouCtrl\n"
    );
    let found = &abilities(&script)[0]["Triggered"]["effects"];
    assert_eq!(
        found.pointer(at),
        Some(&json!("WhileSourceOnBattlefield")),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::group_from_a_graveyard(
    "ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card | Duration$ UntilHostLeavesPlay",
    "key:ChangeZoneAll.Duration"
)]
#[case::group_for_a_turn(
    "ChangeZoneAll | Origin$ Battlefield | Destination$ Exile | ChangeType$ Creature | Duration$ UntilEndOfTurn",
    "key:ChangeZoneAll.Duration"
)]
#[case::picked_from_a_hand(
    "ChangeZone | Origin$ Hand | Destination$ Exile | ValidTgts$ Opponent | Chooser$ You | ChangeType$ Card.nonLand | ChangeNum$ 1 | Duration$ UntilHostLeavesPlay",
    "key:ChangeZone.Choose.Duration"
)]
fn an_exile_until_from_elsewhere_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert!(has_gap(&script, gap), "{:?}", gaps(&script));
}

#[rstest]
#[case::sacrifice("Sacrifice", json!({"step": "End"}), "Sacrifice")]
#[case::your_sacrifice("YourSacrifice", json!({"step": "End", "whose": "You"}), "Sacrifice")]
#[case::to_hand("Hand", json!({"step": "End"}), "Move")]
fn a_returned_creature_is_cleaned_up_at_the_end_step(
    #[case] word: &str,
    #[case] when: Value,
    #[case] verb: &str,
) {
    let script = spell(&format!(
        "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | GainControl$ True | AtEOT$ {word}"
    ));
    let found = effects(&script);
    let moved = found[0]["Move"]["as"].clone();
    let delayed = &found[1]["DelayedTrigger"];
    assert_eq!(delayed["when"]["StepBegins"], when, "{:?}", gaps(&script));
    let later = &delayed["effects"][0][verb];
    let named = later.get("what").or(later.get("filter"));
    assert!(
        named.is_some_and(|value| value.to_string().contains(moved.as_str().unwrap_or("?"))),
        "{later}"
    );
}

#[rstest]
#[case::to_hand(
    "ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | AtEOT$ Sacrifice"
)]
#[case::unknown_word(
    "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | AtEOT$ Later"
)]
fn an_end_step_clean_up_of_another_move_stays_a_gap(#[case] line: &str) {
    let script = spell(line);
    assert!(
        gaps(&script)
            .iter()
            .any(|gap| gap.starts_with("key:ChangeZone.AtEOT") || gap.starts_with("key:AtEOT=")),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::added("AddColor$ Black", json!({"AddColors": ["Black"]}))]
#[case::set("SetColor$ Black", json!({"SetColors": ["Black"]}))]
fn a_returned_creature_takes_the_color_as_written(#[case] color: &str, #[case] wanted: Value) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature | GainControl$ True | StaticEffect$ A\nSVar:A:Mode$ Continuous | Affected$ Card.IsRemembered | AddType$ Zombie | {color}\n"
    );
    let found = effects(&script);
    let mods = &found[1]["Continuous"]["mods"];
    assert_eq!(
        mods[0],
        json!({"AddSubtype": "Zombie"}),
        "{:?}",
        gaps(&script)
    );
    assert_eq!(mods[1], wanted);
}

#[test]
fn an_added_color_that_is_no_color_stays_a_gap() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature | StaticEffect$ A\nSVar:A:Mode$ Continuous | Affected$ Card.IsRemembered | AddColor$ ChosenColor\n";
    assert!(
        has_gap(script, "key:ChangeZone.StaticEffect=color"),
        "{:?}",
        gaps(script)
    );
}

#[test]
fn a_shuffle_in_of_up_to_some_targets_is_the_plain_shuffle_in() {
    let line = "ChangeZone | Origin$ Graveyard | Destination$ Library | ValidTgts$ Card.YouOwn | TargetMin$ 0 | TargetMax$ 4 | Shuffle$ True";
    let plain = effects(&spell(line));
    let optional = spell(&format!("{line} | ShuffleNonMandatory$ True"));
    assert_eq!(effects(&optional), plain, "{:?}", gaps(&optional));
}

#[rstest]
#[case::exactly_one(
    "ChangeZone | Origin$ Graveyard | Destination$ Library | ValidTgts$ Card.YouOwn | Shuffle$ True | ShuffleNonMandatory$ True"
)]
#[case::to_the_top(
    "ChangeZone | Origin$ Graveyard | Destination$ Library | ValidTgts$ Card.YouOwn | TargetMin$ 0 | TargetMax$ 1 | ShuffleNonMandatory$ True"
)]
fn an_optional_shuffle_of_another_move_stays_a_gap(#[case] line: &str) {
    let script = spell(line);
    assert!(
        has_gap(&script, "key:ChangeZone.ShuffleNonMandatory"),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::toughness("Count$CardToughness", json!({"Stat": {"of": {"Bound": "each"}, "stat": "Toughness"}}))]
#[case::colors("Count$CardNumColors", json!({"Stat": {"of": {"Bound": "each"}, "stat": "Colors"}}))]
fn counters_numbered_per_object_read_each_object(#[case] x: &str, #[case] count: Value) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | Defined$ Valid Creature.YouCtrl+Other | CounterType$ P1P1 | CounterNumPerDefined$ X\nSVar:X:{x}\n"
    );
    let found = effects(&script);
    let repeat = &found[0]["RepeatEach"];
    assert_eq!(
        repeat["over"],
        json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}, "Other"]}),
        "{:?}",
        gaps(&script)
    );
    assert_eq!(
        repeat["effects"],
        json!([{"PutCounters": {"on": {"Bound": "each"}, "counter": "+1/+1", "count": count}}])
    );
}

#[rstest]
#[case::another_count("Count$Valid Creature.YouCtrl")]
#[case::memory("Remembered$Amount")]
fn counters_numbered_per_object_by_another_count_stay_a_gap(#[case] x: &str) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | Defined$ Valid Creature.YouCtrl | CounterType$ P1P1 | CounterNumPerDefined$ X\nSVar:X:{x}\n"
    );
    assert!(
        has_gap(&script, "key:PutCounter.CounterNumPerDefined"),
        "{:?}",
        gaps(&script)
    );
}
