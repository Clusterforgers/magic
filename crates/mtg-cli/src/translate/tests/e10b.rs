//! Goal: a face-down exile no player may look at (CR 406.3) translates for every Forge verb that names one (`Dig`, `ChangeZone`, `ChangeZoneAll`) and binds the cards in exile for what the line remembers, a "you may look at" line becomes the look rule of the cards it names beside a permission to play them, and a shape the engine cannot say (a duration, a foretelling, a card turned face up in exile) stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn sorcery(line: &str) -> String {
    format!("Name:Subject\nManaCost:R\nTypes:Sorcery\n{line}\n")
}

fn creature(lines: &str) -> String {
    format!("Name:Subject\nManaCost:R\nTypes:Creature Elf\nPT:1/1\n{lines}\n")
}

/// The effects of the spell the line is.
fn spell_effects(line: &str) -> Value {
    let script = sorcery(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    normal(found[0].clone())["Spell"]["effects"].clone()
}

#[rstest]
#[case::the_top_card(
    "A:SP$ Dig | Defined$ You | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | ExileFaceDown$ True | NoReveal$ True",
    json!([{"ExileTop": {"as": "exiled", "sealed": true}}])
)]
#[case::each_players_top_card(
    "A:SP$ Dig | Defined$ Player | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | ExileFaceDown$ True",
    json!([{"ExileTop": {"as": "exiled", "who": "EachPlayer", "sealed": true}}])
)]
#[case::the_hand(
    "A:SP$ ChangeZoneAll | ChangeType$ Card.YouCtrl | Origin$ Hand | Destination$ Exile | ExileFaceDown$ True",
    json!([{"ExileHidden": {"what": {"Each": [{"OwnedBy": "You"}, {"In": "Hand"}]}, "sealed": true}}])
)]
#[case::the_top_card_named(
    "A:SP$ ChangeZone | Defined$ TopOfLibrary | Origin$ Library | Destination$ Exile | ExileFaceDown$ True",
    json!([{"ExileHidden": {"what": {"Each": [{"In": "Library"}, {"OwnedBy": "You"}, "TopOfLibrary"]}, "sealed": true}}])
)]
#[case::the_exiler_looks(
    "A:SP$ ChangeZone | Defined$ TopOfLibrary | Origin$ Library | Destination$ Exile | ExileFaceDown$ True | WithMayLook$ True",
    json!([{"ExileHidden": {"what": {"Each": [{"In": "Library"}, {"OwnedBy": "You"}, "TopOfLibrary"]}, "held": true}}])
)]
fn a_face_down_exile_maps(#[case] line: &str, #[case] effects: Value) {
    let found = spell_effects(line);
    let expected = normal(json!({"Spell": {"effects": effects}}))["Spell"]["effects"].clone();
    assert_eq!(found, expected);
}

#[test]
fn a_search_remembers_the_card_in_exile_not_the_one_found_in_the_library() {
    let effects = spell_effects(
        "A:SP$ ChangeZone | Origin$ Library | Destination$ Exile | ChangeType$ Card | ChangeNum$ 1 | ExileFaceDown$ True | RememberChanged$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ STPlay | Duration$ Permanent | ForgetOnMoved$ Exile\nSVar:STPlay:Mode$ Continuous | MayLookAt$ You | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile",
    );
    let text = effects.to_string();
    let exiled = effects
        .as_array()
        .unwrap()
        .iter()
        .find_map(|effect| effect.pointer("/ExileHidden/as"))
        .and_then(Value::as_str)
        .expect("the exile names the cards")
        .to_owned();
    assert!(
        text.contains(&format!(r#""only":{{"Bound":"{exiled}"}}"#)),
        "{text}"
    );
    assert!(
        text.contains("LookAtExiled") && text.contains("MayCast"),
        "{text}"
    );
    assert!(!text.contains(r#""only":{"Bound":"found"}"#), "{text}");
}

#[rstest]
#[case::look(
    "S:Mode$ Continuous | Affected$ Card.ExiledWithSource | AffectedZone$ Exile | MayLookAt$ You",
    &[json!({"Rule": {"rule": {"LookAtExiled": {"what": [{"ExiledWith": "This"}, {"In": "Exile"}]}}}})]
)]
#[case::look_and_play(
    "S:Mode$ Continuous | Affected$ Card.ExiledWithSource | AffectedZone$ Exile | MayPlay$ True | MayLookAt$ You | Description$ You may look at the cards exiled with CARDNAME, and you may play lands and cast spells from among those cards.",
    &[
        json!({"Rule": {"rule": {"MayCast": {"from": "Exile", "play": true, "what": [{"ExiledWith": "This"}]}}}}),
        json!({"Rule": {"rule": {"LookAtExiled": {"what": [{"ExiledWith": "This"}, {"In": "Exile"}]}}}}),
    ]
)]
fn a_look_at_exiled_cards_maps(#[case] line: &str, #[case] expected: &[Value]) {
    let script = creature(line);
    let found: Vec<Value> = abilities(&script).into_iter().map(normal).collect();
    let expected: Vec<Value> = expected.iter().cloned().map(normal).collect();
    assert_eq!(found, expected, "{:?}", gaps(&script));
}

#[rstest]
#[case::until_it_leaves(
    "A:SP$ ChangeZoneAll | ChangeType$ Card.YouCtrl | Origin$ Hand | Destination$ Exile | ExileFaceDown$ True | Duration$ UntilHostLeavesPlay",
    "key:ChangeZoneAll.ExileFaceDown-with-Duration"
)]
#[case::foretold(
    "A:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 1 | ExileFaceDown$ True | Mandatory$ True | Foretold$ True",
    "key:ChangeZone.ExileFaceDown-with-Foretold"
)]
#[case::not_to_exile(
    "A:SP$ ChangeZoneAll | ChangeType$ Card.YouCtrl | Origin$ Hand | Destination$ Graveyard | ExileFaceDown$ True",
    "key:ChangeZoneAll.ExileFaceDown"
)]
#[case::turned_face_up_in_exile(
    "A:SP$ SetState | Defined$ ValidExile Card.ExiledWithSource | Mode$ TurnFaceUp",
    "key:SetState.TurnFaceUp-in-exile"
)]
fn a_shape_the_engine_cannot_say_is_a_named_gap(#[case] line: &str, #[case] gap: &str) {
    let found = gaps(&sorcery(line));
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}
