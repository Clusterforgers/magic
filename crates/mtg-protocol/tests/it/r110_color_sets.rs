//! Goal: the hidden-object check reads a color set as no object id: a continuous effect that makes a permanent every color
//! carries the set 31 (all five bits), which must not count as naming a hidden object 31, while the same number under an
//! object key still does, and so is the player a goad names; no corpus deck is involved.

use std::collections::BTreeMap;

use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::effects::Modification;
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::leaks;

#[rstest]
#[case(Modification::SetColors(Color::ALL.into_iter().collect()))]
#[case(Modification::AddColors(Color::ALL.into_iter().collect()))]
fn r110_a_color_set_is_no_object_id(#[case] modification: Modification) {
    let hidden: BTreeMap<ObjectId, String> =
        BTreeMap::from([(ObjectId(31), "Hidden Card".to_owned())]);
    let value = serde_json::to_value(&modification).unwrap();
    assert_eq!(
        value.as_object().and_then(|map| map.values().next()),
        Some(&Value::from(31))
    );
    assert!(leaks(&value, &hidden).is_empty(), "{value}");
    let named = serde_json::json!({ "object": 31 });
    assert_eq!(leaks(&named, &hidden), ["ObjectId(31) under object"]);
}

#[test]
fn r110_the_player_a_goad_names_is_no_object_id() {
    let hidden: BTreeMap<ObjectId, String> =
        BTreeMap::from([(ObjectId(1), "Hidden Card".to_owned())]);
    let value = serde_json::to_value(Modification::Goaded {
        by: mtg_core::ids::PlayerId(1),
    })
    .unwrap();
    assert_eq!(value, serde_json::json!({ "Goaded": { "by": 1 } }));
    assert!(leaks(&value, &hidden).is_empty(), "{value}");
    // An object a `by` names is a reference whose id is still checked.
    let held = serde_json::json!({ "Held": { "by": { "id": 1, "counter": 0 }, "looker": 0 } });
    assert_eq!(leaks(&held, &hidden), ["ObjectId(1) under id"]);
}
