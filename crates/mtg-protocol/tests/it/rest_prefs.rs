//! Goal: the account preferences keep their documented wire shape (default stops and the end-turn choice, off and empty by default, a JSON round trip).

#![allow(clippy::unwrap_used)]

use mtg_protocol::rest::UserPrefs;
use serde_json::json;

#[test]
fn prefs_survive_a_json_round_trip() {
    let prefs: UserPrefs = serde_json::from_value(json!({
        "default_stops": [{"side": "Own", "step": "Upkeep", "keep": true}],
        "end_turn_by_hand": true
    }))
    .unwrap();
    assert_eq!(prefs.default_stops.len(), 1);
    assert_eq!(
        serde_json::from_value::<UserPrefs>(serde_json::to_value(&prefs).unwrap()).unwrap(),
        prefs
    );
    assert_eq!(UserPrefs::default().default_stops.len(), 0);
    assert!(!UserPrefs::default().end_turn_by_hand);
}
