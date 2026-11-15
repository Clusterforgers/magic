//! Goal: the deck-group bodies keep their documented wire shape (a change leaves out what stays, JSON round trips) and the TypeScript bindings carry every one of them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_protocol::rest::{
    DeckGroupChange, DeckGroupOrder, DeckGroupView, DeckGroups, NewDeckGroup,
};
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
#[case(DeckGroupChange::default(), json!({}))]
#[case(DeckGroupChange { name: Some("Fun".into()), collapsed: None }, json!({"name": "Fun"}))]
#[case(DeckGroupChange { name: None, collapsed: Some(true) }, json!({"collapsed": true}))]
fn a_change_sends_only_what_changes(#[case] change: DeckGroupChange, #[case] wire: Value) {
    assert_eq!(serde_json::to_value(&change).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<DeckGroupChange>(wire).unwrap(),
        change
    );
}

#[test]
fn the_list_order_and_new_group_survive_a_round_trip() {
    let list = DeckGroups {
        groups: vec![
            DeckGroupView {
                id: "g1".into(),
                name: "Hardcore".into(),
                collapsed: true,
                decks: vec!["d1".into(), "d2".into()],
            },
            // A deck may be listed under several groups.
            DeckGroupView {
                id: "g2".into(),
                name: "Fun".into(),
                collapsed: false,
                decks: vec!["d1".into()],
            },
        ],
    };
    let wire = serde_json::to_value(&list).unwrap();
    assert_eq!(wire["groups"][0]["decks"], json!(["d1", "d2"]));
    assert_eq!(wire["groups"][1]["decks"], json!(["d1"]));
    assert_eq!(serde_json::from_value::<DeckGroups>(wire).unwrap(), list);
    let order = DeckGroupOrder {
        ids: vec!["b".into(), "a".into()],
    };
    assert_eq!(
        serde_json::to_value(&order).unwrap(),
        json!({"ids": ["b", "a"]})
    );
    let new = NewDeckGroup {
        name: "Dungeon".into(),
    };
    assert_eq!(
        serde_json::to_value(&new).unwrap(),
        json!({"name": "Dungeon"})
    );
}

#[test]
fn the_bindings_carry_the_group_types() {
    let files = generated();
    for needed in [
        "DeckGroups.ts",
        "DeckGroupView.ts",
        "NewDeckGroup.ts",
        "DeckGroupChange.ts",
        "DeckGroupOrder.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["DeckGroupChange.ts"].contains("name?: string"));
}
