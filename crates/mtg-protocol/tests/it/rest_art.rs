//! Goal: the art bodies keep their documented JSON shape: a line's choice names its zone, card and printing (null for the default) and refuses other fields, the preferred art is a map of printing by card, a printing from an older server reads without its look, a deck picture names its printing only when the deck chose one, and the bindings carry every art type.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_protocol::rest::{
    DeckPicture, LinePrintingBody, PreferredPrintingBody, PreferredPrintings, PrintingView,
};
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
#[case::chosen(json!({"zone": "commander", "oracle_id": "o", "printing_id": "p"}), Some("p"))]
#[case::default(json!({"zone": "main", "oracle_id": "o", "printing_id": null}), None)]
fn a_lines_choice_reads_and_writes_one_shape(#[case] body: Value, #[case] printing: Option<&str>) {
    let read: LinePrintingBody = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(read.printing_id.as_deref(), printing);
    assert_eq!(serde_json::to_value(&read).unwrap(), body);
}

#[rstest]
#[case::other_field(json!({"zone": "main", "oracle_id": "o", "printing_id": null, "count": 1}))]
#[case::unknown_zone(json!({"zone": "maybe", "oracle_id": "o", "printing_id": null}))]
#[case::no_card(json!({"zone": "main", "printing_id": null}))]
fn a_lines_choice_refuses_other_shapes(#[case] body: Value) {
    assert!(serde_json::from_value::<LinePrintingBody>(body).is_err());
}

#[test]
fn the_preferred_art_is_a_map_and_its_body_one_printing() {
    let prefs = PreferredPrintings {
        printings: [("o".to_owned(), "p".to_owned())].into(),
    };
    assert_eq!(
        serde_json::to_value(&prefs).unwrap(),
        json!({"printings": {"o": "p"}})
    );
    let body: PreferredPrintingBody = serde_json::from_value(json!({"printing_id": "p"})).unwrap();
    assert_eq!(body.printing_id, "p");
    assert!(
        serde_json::from_value::<PreferredPrintingBody>(json!({"printing_id": "p", "x": 1}))
            .is_err()
    );
}

#[test]
fn a_printing_of_an_older_server_reads_without_its_look() {
    let printing: PrintingView = serde_json::from_value(json!({
        "printing_id": "p", "set": "lea", "collector_number": "1", "rarity": null,
        "released_at": null, "lang": "en", "digital": false, "images": {"card": null, "faces": []}
    }))
    .unwrap();
    assert_eq!((printing.set_name, printing.frame), (None, None));
    assert!(printing.finishes.is_empty() && !printing.full_art);
}

#[rstest]
#[case::chosen(Some("p"), json!({"oracle_id": "o", "name": "N", "printing_id": "p"}))]
#[case::default(None, json!({"oracle_id": "o", "name": "N"}))]
fn a_deck_picture_names_its_printing_only_when_chosen(
    #[case] printing: Option<&str>,
    #[case] wire: Value,
) {
    let picture = DeckPicture {
        oracle_id: "o".into(),
        name: "N".into(),
        printing_id: printing.map(str::to_owned),
    };
    assert_eq!(serde_json::to_value(&picture).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<DeckPicture>(wire).unwrap(),
        picture
    );
}

#[test]
fn the_bindings_carry_the_art_types() {
    let files = generated();
    for needed in [
        "PrintingList.ts",
        "PreferredPrintings.ts",
        "PreferredPrintingBody.ts",
        "LinePrintingBody.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["PrintingView.ts"].contains("full_art: boolean"));
    assert!(files["DeckPicture.ts"].contains("printing_id?: string"));
}
