//! Goal: the card bodies keep their documented JSON shape, refuse nothing a client may send back and say nothing a client cannot read, and the TypeScript bindings carry every card type with the exact unions of colors and support states.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use crate::common::generated;
use mtg_cards::card::Span;
use mtg_cards::support::{Reason, Support};
use mtg_protocol::rest::{
    CardColor, CardDetail, CardFaceView, CardHit, CardImages, CardSearchPage, ImageLinks,
    PrintingView, RulingView,
};
use rstest::rstest;
use serde_json::json;

fn hit() -> CardHit {
    CardHit {
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: "Lightning Bolt".into(),
        mana_cost: "{R}".into(),
        mana_value: 1.0,
        type_line: "Instant".into(),
        colors: vec![CardColor::R],
        color_identity: vec![CardColor::R],
        images: CardImages {
            card: Some(ImageLinks {
                normal: Some("https://cards.scryfall.io/normal/front/e/3/e3285e6b.jpg".into()),
                ..ImageLinks::default()
            }),
            faces: vec![None],
        },
        support: Support::Verified,
    }
}

#[test]
fn a_search_page_has_the_documented_wire_shape() {
    let page = CardSearchPage {
        cards: vec![hit()],
        total: 1234,
        page: 2,
        per_page: 40,
    };
    let value = serde_json::to_value(&page).unwrap();
    assert_eq!(
        value,
        json!({
            "cards": [{
                "oracle_id": "4457ed35-7c10-48c8-9776-456485fdf070",
                "name": "Lightning Bolt",
                "mana_cost": "{R}",
                "mana_value": 1.0,
                "type_line": "Instant",
                "colors": ["R"],
                "color_identity": ["R"],
                "images": {
                    "card": {
                        "small": null,
                        "normal": "https://cards.scryfall.io/normal/front/e/3/e3285e6b.jpg",
                        "large": null,
                        "png": null,
                        "art_crop": null,
                        "border_crop": null
                    },
                    "faces": [null]
                },
                "support": "verified"
            }],
            "total": 1234,
            "page": 2,
            "per_page": 40
        })
    );
    assert_eq!(
        serde_json::from_value::<CardSearchPage>(value).unwrap(),
        page
    );
}

#[test]
fn a_card_page_has_the_documented_wire_shape() {
    let detail = CardDetail {
        card: hit(),
        oracle_text: "Lightning Bolt deals 3 damage to any target.".into(),
        keywords: vec![],
        faces: vec![CardFaceView {
            name: "Lightning Bolt".into(),
            mana_cost: "{R}".into(),
            type_line: "Instant".into(),
            oracle_text: "Lightning Bolt deals 3 damage to any target.".into(),
            power: None,
            toughness: None,
            loyalty: None,
            defense: None,
            colors: vec![CardColor::R],
        }],
        legalities: BTreeMap::from([("commander".to_owned(), "legal".to_owned())]),
        printings: vec![PrintingView {
            printing_id: "e3285e6b-3e79-4d7c-bf96-d920f973b122".into(),
            set: "m10".into(),
            collector_number: "146".into(),
            rarity: Some("common".into()),
            released_at: Some("2009-07-17".into()),
            lang: "en".into(),
            digital: false,
            images: CardImages::default(),
        }],
        rulings: vec![RulingView {
            source: "wotc".into(),
            published_at: Some("2004-10-04".into()),
            comment: "The damage is dealt at once.".into(),
        }],
        reasons: vec![Reason {
            face: 0,
            span: Span { start: 0, end: 44 },
            text: "the engine does not run Effect.DealDamage yet".into(),
        }],
    };
    let value = serde_json::to_value(&detail).unwrap();
    assert_eq!(value["legalities"], json!({"commander": "legal"}));
    assert_eq!(
        value["reasons"],
        json!([{"face": 0, "span": {"start": 0, "end": 44}, "text": "the engine does not run Effect.DealDamage yet"}])
    );
    assert_eq!(value["printings"][0]["released_at"], "2009-07-17");
    assert_eq!(value["faces"][0]["power"], json!(null));
    assert_eq!(serde_json::from_value::<CardDetail>(value).unwrap(), detail);
}

#[rstest]
#[case(Support::Verified, "verified")]
#[case(Support::Partial, "partial")]
#[case(Support::Manual, "manual")]
#[case(Support::Stale, "stale")]
#[case(Support::Unplayable, "unplayable")]
fn support_states_are_snake_case_words(#[case] state: Support, #[case] word: &str) {
    assert_eq!(serde_json::to_value(state).unwrap(), json!(word));
    assert_eq!(
        serde_json::from_value::<Support>(json!(word)).unwrap(),
        state
    );
}

#[rstest]
#[case(CardColor::W, "W")]
#[case(CardColor::U, "U")]
#[case(CardColor::B, "B")]
#[case(CardColor::R, "R")]
#[case(CardColor::G, "G")]
fn colors_are_single_letters(#[case] color: CardColor, #[case] letter: &str) {
    assert_eq!(serde_json::to_value(color).unwrap(), json!(letter));
    assert_eq!(
        serde_json::from_value::<CardColor>(json!(letter)).unwrap(),
        color
    );
}

#[rstest]
#[case("w")]
#[case("White")]
#[case("C")]
#[case("")]
fn other_spellings_of_a_color_do_not_exist(#[case] text: &str) {
    assert!(serde_json::from_value::<CardColor>(json!(text)).is_err());
}

#[test]
fn the_bindings_carry_the_card_types() {
    let files = generated();
    for needed in [
        "CardSearchPage.ts",
        "CardHit.ts",
        "CardDetail.ts",
        "CardFaceView.ts",
        "PrintingView.ts",
        "RulingView.ts",
        "CardImages.ts",
        "ImageLinks.ts",
        "CardColor.ts",
        "Support.ts",
        "Reason.ts",
        "Span.ts",
    ] {
        assert!(files.contains_key(needed), "missing {needed}");
        assert!(files["index.ts"].contains(needed.trim_end_matches(".ts")));
    }
    assert!(files["CardColor.ts"].contains("\"W\" | \"U\" | \"B\" | \"R\" | \"G\""));
    assert!(files["Support.ts"].contains(
        "\"verified\" | \"read\" | \"partial\" | \"manual\" | \"stale\" | \"unplayable\""
    ));
    assert!(files["CardSearchPage.ts"].contains("cards: Array<CardHit>"));
    assert!(files["CardSearchPage.ts"].contains("total: number"));
    assert!(files["CardHit.ts"].contains("support: Support"));
    assert!(files["CardHit.ts"].contains("colors: Array<CardColor>"));
    assert!(files["CardDetail.ts"].contains("reasons: Array<Reason>"));
    assert!(files["Reason.ts"].contains("span: Span"));
    assert!(files["Span.ts"].contains("start: number"));
}
