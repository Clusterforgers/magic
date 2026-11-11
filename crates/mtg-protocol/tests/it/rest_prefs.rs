//! Goal: the account preferences keep their documented wire shape (default stops and the end-turn choice, off and empty by default except the own declare attackers and postcombat main stops, a JSON round trip), and the hand sort reads "as drawn" over every group when absent and keeps its order of groups.

#![allow(clippy::unwrap_used)]

use mtg_protocol::rest::{HAND_GROUPS, HandGroup, HandSort, HandSortBy, UserPrefs, default_stops};
use rstest::rstest;
use serde_json::json;

#[test]
fn prefs_survive_a_json_round_trip() {
    let prefs: UserPrefs = serde_json::from_value(json!({
        "default_stops": [{"side": "Own", "step": "Upkeep", "keep": true}],
        "end_turn_by_hand": true,
        "auto_choices": {"order_triggers": true, "assign_damage": false}
    }))
    .unwrap();
    assert_eq!(prefs.default_stops.len(), 1);
    assert!(prefs.auto_choices.order_triggers && !prefs.auto_choices.assign_damage);
    assert_eq!(
        serde_json::from_value::<UserPrefs>(serde_json::to_value(&prefs).unwrap()).unwrap(),
        prefs
    );
    assert!(!UserPrefs::default().end_turn_by_hand);
}

#[test]
fn the_defaults_pin_the_own_declare_attackers_and_postcombat_main() {
    let stops = serde_json::to_value(UserPrefs::default().default_stops).unwrap();
    assert_eq!(
        stops,
        json!([
            {"side": "Own", "step": "DeclareAttackers", "keep": true},
            {"side": "Own", "step": "Main2", "keep": true}
        ])
    );
    assert_eq!(UserPrefs::default().default_stops, default_stops());
}

#[test]
fn a_body_without_auto_choices_reads_them_as_off() {
    let prefs: UserPrefs =
        serde_json::from_value(json!({"default_stops": [], "end_turn_by_hand": false})).unwrap();
    assert_eq!(
        prefs,
        UserPrefs {
            default_stops: Vec::new(),
            ..UserPrefs::default()
        }
    );
}

#[test]
fn a_body_without_a_hand_sort_reads_as_drawn_over_every_group() {
    let prefs: UserPrefs =
        serde_json::from_value(json!({"default_stops": [], "end_turn_by_hand": false})).unwrap();
    assert_eq!(prefs.hand_sort.by, HandSortBy::Drawn);
    assert_eq!(prefs.hand_sort.groups, HAND_GROUPS.to_vec());
}

#[rstest]
#[case::lands_first(HandSortBy::Groups, vec![HandGroup::Land, HandGroup::Artifact, HandGroup::Creature])]
#[case::spells_first(HandSortBy::Groups, vec![HandGroup::Instant, HandGroup::Sorcery, HandGroup::Land])]
#[case::by_mana(HandSortBy::ManaValue, HAND_GROUPS.to_vec())]
#[case::no_groups(HandSortBy::Drawn, Vec::new())]
fn a_hand_sort_keeps_its_order_on_the_wire(#[case] by: HandSortBy, #[case] groups: Vec<HandGroup>) {
    let prefs = UserPrefs {
        hand_sort: HandSort {
            by,
            groups: groups.clone(),
        },
        ..UserPrefs::default()
    };
    let value = serde_json::to_value(&prefs).unwrap();
    assert_eq!(value["hand_sort"]["by"], json!(format!("{by:?}")));
    let back: UserPrefs = serde_json::from_value(value).unwrap();
    assert_eq!(back.hand_sort.groups, groups);
}
