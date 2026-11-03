//! Goal: the favourite-precon list keeps its documented wire shape (one `codes` list, empty by default, a JSON round trip), the deck pictures of the lists are an oracle id and a name, and the TypeScript bindings carry both.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_protocol::rest::{DeckPicture, FavouritePrecons, MAX_DECK_PICTURES};
use serde_json::json;

#[test]
fn a_list_survives_a_json_round_trip_with_one_key() {
    let list = FavouritePrecons {
        codes: vec![
            "c19-merciless-rage".to_owned(),
            "c18-draconic-domination".to_owned(),
        ],
    };
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(
        json,
        json!({"codes": ["c19-merciless-rage", "c18-draconic-domination"]})
    );
    assert_eq!(
        serde_json::from_value::<FavouritePrecons>(json).unwrap(),
        list
    );
    assert_eq!(FavouritePrecons::default().codes, Vec::<String>::new());
}

#[test]
fn a_picture_is_an_oracle_id_and_a_name() {
    let picture = DeckPicture {
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".to_owned(),
        name: "Sol Ring".to_owned(),
    };
    let json = serde_json::to_value(&picture).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 2);
    assert_eq!(
        serde_json::from_value::<DeckPicture>(json).unwrap(),
        picture
    );
    assert_eq!(MAX_DECK_PICTURES, 2);
}

#[test]
fn the_bindings_carry_the_favourite_and_picture_types() {
    let files = generated();
    for needed in ["FavouritePrecons.ts", "DeckPicture.ts"] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["FavouritePrecons.ts"].contains("codes: Array<string>"));
    assert!(files["DeckPicture.ts"].contains("oracle_id: string"));
    assert!(files["DeckListItem.ts"].contains("pictures: Array<DeckPicture>"));
}
