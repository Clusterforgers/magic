//! Converts raw Scryfall objects into normalized records.

use std::collections::BTreeMap;

use mtg_core::color::{Color, ColorSet};
use thiserror::Error;

use super::raw::{RawCard, RawCardFace, RawRuling};
use super::record::{
    CardFace, CardRecord, Layout, Legality, ProducedMana, Rarity, RulingRecord, RulingSource,
};
use crate::ids::{OracleId, PrintingId};

/// A Scryfall object that lacks a field we cannot work without.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidRecord {
    /// The card has no name.
    #[error("card has no name")]
    MissingName,
    /// The card has no printing id.
    #[error("card {name:?} has no id")]
    MissingId {
        /// Name of the offending card.
        name: String,
    },
    /// Neither the card nor any face carries an oracle id.
    #[error("card {name:?} has no oracle id")]
    MissingOracleId {
        /// Name of the offending card.
        name: String,
    },
    /// A face of a multi-faced card has no name.
    #[error("face {face} of card {name:?} has no name")]
    MissingFaceName {
        /// Name of the offending card.
        name: String,
        /// Zero-based face index.
        face: usize,
    },
    /// A ruling has no oracle id.
    #[error("ruling has no oracle id")]
    RulingWithoutOracleId,
    /// A ruling has no text.
    #[error("ruling for {oracle_id} has no comment")]
    RulingWithoutComment {
        /// Oracle id of the card the ruling belongs to.
        oracle_id: String,
    },
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

fn colors_from(symbols: &[String]) -> ColorSet {
    symbols
        .iter()
        .filter_map(|s| s.chars().next())
        .filter_map(Color::from_symbol)
        .collect()
}

fn face_color_union(front: &CardFace, back_faces: &[CardFace]) -> ColorSet {
    let faces: Vec<&CardFace> = std::iter::once(front).chain(back_faces).collect();
    Color::ALL
        .into_iter()
        .filter(|&color| faces.iter().any(|face| face.colors.contains(color)))
        .collect()
}

fn joined_type_line(front: &CardFace, back_faces: &[CardFace]) -> String {
    std::iter::once(front)
        .chain(back_faces)
        .map(|face| face.type_line.as_str())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" // ")
}

fn legalities_from(raw: BTreeMap<String, String>) -> BTreeMap<String, Legality> {
    raw.into_iter()
        .map(|(format, value)| (format, Legality::parse(&value)))
        .collect()
}

fn produced_mana_from(symbols: &[String]) -> ProducedMana {
    ProducedMana {
        colors: colors_from(symbols),
        colorless: symbols.iter().any(|s| s == "C"),
    }
}

fn face_from_raw(raw: RawCardFace, card: &str, index: usize) -> Result<CardFace, InvalidRecord> {
    let name = non_empty(raw.name).ok_or_else(|| InvalidRecord::MissingFaceName {
        name: card.to_owned(),
        face: index,
    })?;
    Ok(CardFace {
        name,
        mana_cost: raw.mana_cost.unwrap_or_default(),
        type_line: raw.type_line.unwrap_or_default(),
        oracle_text: raw.oracle_text.unwrap_or_default(),
        power: raw.power,
        toughness: raw.toughness,
        loyalty: raw.loyalty,
        defense: raw.defense,
        colors: colors_from(&raw.colors.unwrap_or_default()),
        images: raw.image_uris,
    })
}

/// Splits Scryfall's face list into front and back faces, synthesizing a front for single-faced cards.
fn split_faces(
    card_faces: Option<Vec<RawCardFace>>,
    single: impl FnOnce() -> RawCardFace,
    name: &str,
) -> Result<(CardFace, Vec<CardFace>), InvalidRecord> {
    let mut faces = card_faces.unwrap_or_default().into_iter();
    match faces.next() {
        Some(first) => {
            let front = face_from_raw(first, name, 0)?;
            let back = faces
                .enumerate()
                .map(|(i, face)| face_from_raw(face, name, i + 1))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((front, back))
        }
        None => Ok((face_from_raw(single(), name, 0)?, Vec::new())),
    }
}

impl TryFrom<RawCard> for CardRecord {
    type Error = InvalidRecord;

    fn try_from(raw: RawCard) -> Result<Self, Self::Error> {
        let RawCard {
            id,
            oracle_id,
            name,
            lang,
            layout,
            set,
            collector_number,
            rarity,
            released_at,
            digital,
            mana_cost,
            cmc,
            type_line,
            oracle_text,
            power,
            toughness,
            loyalty,
            defense,
            colors,
            color_identity,
            keywords,
            produced_mana,
            legalities,
            image_uris,
            card_faces,
        } = raw;

        let name = non_empty(name).ok_or(InvalidRecord::MissingName)?;
        let printing_id =
            non_empty(id).ok_or_else(|| InvalidRecord::MissingId { name: name.clone() })?;
        let oracle_id = non_empty(oracle_id)
            .or_else(|| {
                card_faces
                    .iter()
                    .flatten()
                    .find_map(|face| non_empty(face.oracle_id.clone()))
            })
            .ok_or_else(|| InvalidRecord::MissingOracleId { name: name.clone() })?;

        let (front, back_faces) = split_faces(
            card_faces,
            || RawCardFace {
                name: Some(name.clone()),
                oracle_id: None,
                mana_cost: mana_cost.clone(),
                type_line: type_line.clone(),
                oracle_text,
                power,
                toughness,
                loyalty,
                defense,
                colors: colors.clone(),
                image_uris: None,
            },
            &name,
        )?;

        let mana_cost = non_empty(mana_cost).unwrap_or_else(|| front.mana_cost.clone());
        let type_line =
            non_empty(type_line).unwrap_or_else(|| joined_type_line(&front, &back_faces));
        let colors = colors.map_or_else(
            || face_color_union(&front, &back_faces),
            |symbols| colors_from(&symbols),
        );

        Ok(CardRecord {
            oracle_id: OracleId(oracle_id),
            printing_id: PrintingId(printing_id),
            layout: Layout::parse(layout.as_deref().unwrap_or("normal")),
            mana_cost,
            mana_value: cmc.unwrap_or(0.0),
            type_line,
            colors,
            color_identity: colors_from(&color_identity.unwrap_or_default()),
            keywords: keywords.unwrap_or_default(),
            produced_mana: produced_mana_from(&produced_mana.unwrap_or_default()),
            legalities: legalities_from(legalities.unwrap_or_default()),
            images: image_uris,
            set: set.unwrap_or_default(),
            collector_number: collector_number.unwrap_or_default(),
            rarity: rarity.as_deref().and_then(Rarity::parse),
            released_at,
            lang: non_empty(lang).unwrap_or_else(|| "en".to_owned()),
            digital: digital.unwrap_or(false),
            name,
            front,
            back_faces,
        })
    }
}

impl TryFrom<RawRuling> for RulingRecord {
    type Error = InvalidRecord;

    fn try_from(raw: RawRuling) -> Result<Self, Self::Error> {
        let oracle_id = non_empty(raw.oracle_id).ok_or(InvalidRecord::RulingWithoutOracleId)?;
        let comment =
            non_empty(raw.comment).ok_or_else(|| InvalidRecord::RulingWithoutComment {
                oracle_id: oracle_id.clone(),
            })?;
        let source = match raw.source.as_deref() {
            Some("wotc") => RulingSource::Wotc,
            Some("scryfall") => RulingSource::Scryfall,
            _ => RulingSource::Unknown,
        };
        Ok(RulingRecord {
            oracle_id: OracleId(oracle_id),
            source,
            published_at: raw.published_at,
            comment,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Scryfall objects of every layout normalize to one record shape, whatever optional data they omit or add.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;
    use serde_json::{Value, json};

    fn convert(value: Value) -> Result<CardRecord, InvalidRecord> {
        CardRecord::try_from(serde_json::from_value::<RawCard>(value).unwrap())
    }

    fn base(extra: Value) -> Value {
        let mut card = json!({"id": "p-1", "oracle_id": "o-1", "name": "Test Card"});
        if let (Some(card), Value::Object(extra)) = (card.as_object_mut(), extra) {
            card.extend(extra);
        }
        card
    }

    fn color_string(set: ColorSet) -> String {
        Color::ALL
            .into_iter()
            .filter(|&c| set.contains(c))
            .map(|c| match c {
                Color::White => 'W',
                Color::Blue => 'U',
                Color::Black => 'B',
                Color::Red => 'R',
                Color::Green => 'G',
            })
            .collect()
    }

    #[test]
    fn a_bare_card_gets_documented_defaults() {
        let card = convert(base(json!({}))).unwrap();
        assert_eq!(card.oracle_id, OracleId("o-1".into()));
        assert_eq!(card.printing_id, PrintingId("p-1".into()));
        assert_eq!(card.layout, Layout::Normal);
        assert_eq!(card.front.name, "Test Card");
        assert_eq!(card.back_faces, Vec::new());
        assert!(!card.is_multi_faced());
        assert_eq!((card.mana_cost.as_str(), card.type_line.as_str()), ("", ""));
        assert!((card.mana_value - 0.0).abs() < f64::EPSILON);
        assert!(card.colors.is_empty() && card.color_identity.is_empty());
        assert!(card.keywords.is_empty() && card.legalities.is_empty());
        assert_eq!(card.produced_mana, ProducedMana::default());
        assert_eq!(card.lang, "en");
        assert!(!card.digital);
        assert_eq!(card.rarity, None);
        assert_eq!(card.images, None);
    }

    #[rstest]
    #[case::normal(json!({"layout": "normal", "mana_cost": "{R}", "type_line": "Instant", "colors": ["R"]}), 1, "{R}", "Instant", "R")]
    #[case::missing_layout_is_normal(json!({"mana_cost": "{R}", "type_line": "Instant", "colors": ["R"]}), 1, "{R}", "Instant", "R")]
    #[case::split_keeps_combined_root(
        json!({"layout": "split", "mana_cost": "{1}{R} // {1}{U}", "type_line": "Instant // Instant", "colors": ["R", "U"],
               "card_faces": [{"name": "Fire", "mana_cost": "{1}{R}", "colors": ["R"]}, {"name": "Ice", "mana_cost": "{1}{U}", "colors": ["U"]}]}),
        2, "{1}{R} // {1}{U}", "Instant // Instant", "UR"
    )]
    #[case::transform_falls_back_to_front_cost_and_unions_colors(
        json!({"layout": "transform", "mana_cost": "", "type_line": "Creature — Human // Creature — Insect",
               "card_faces": [{"name": "Front", "mana_cost": "{U}", "colors": ["U"]}, {"name": "Back", "mana_cost": "", "colors": ["R"]}]}),
        2, "{U}", "Creature — Human // Creature — Insect", "UR"
    )]
    #[case::mdfc_without_root_type_line_joins_faces(
        json!({"layout": "modal_dfc",
               "card_faces": [{"name": "Spell", "mana_cost": "{2}{R}", "type_line": "Instant", "colors": ["R"]}, {"name": "Land", "type_line": "Land", "colors": []}]}),
        2, "{2}{R}", "Instant // Land", "R"
    )]
    #[case::adventure_with_plain_root_cost(
        json!({"layout": "adventure", "mana_cost": "{2}{R}", "type_line": "Creature — Giant // Instant — Adventure", "colors": ["R"],
               "card_faces": [{"name": "Giant", "mana_cost": "{2}{R}"}, {"name": "Stomp", "mana_cost": "{1}{R}"}]}),
        2, "{2}{R}", "Creature — Giant // Instant — Adventure", "R"
    )]
    #[case::empty_faces_array_is_single_faced(json!({"card_faces": [], "mana_cost": "{G}", "type_line": "Sorcery", "colors": ["G"]}), 1, "{G}", "Sorcery", "G")]
    #[case::null_faces_is_single_faced(json!({"card_faces": null, "mana_cost": "{G}", "type_line": "Sorcery", "colors": ["G"]}), 1, "{G}", "Sorcery", "G")]
    #[case::null_colors_mean_colorless(json!({"colors": null, "type_line": "Artifact"}), 1, "", "Artifact", "")]
    fn faces_and_card_level_fields(
        #[case] extra: Value,
        #[case] face_count: usize,
        #[case] mana_cost: &str,
        #[case] type_line: &str,
        #[case] colors: &str,
    ) {
        let card = convert(base(extra)).unwrap();
        assert_eq!(card.faces().count(), face_count);
        assert_eq!(card.is_multi_faced(), face_count > 1);
        assert_eq!(card.mana_cost, mana_cost);
        assert_eq!(card.type_line, type_line);
        let mut expected: Vec<char> = colors.chars().collect();
        let mut actual: Vec<char> = color_string(card.colors).chars().collect();
        expected.sort_unstable();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn single_faced_card_face_mirrors_the_root() {
        let card = convert(base(json!({
            "mana_cost": "{2}{U}{U}", "type_line": "Legendary Planeswalker — Jace",
            "oracle_text": "+2: ...", "loyalty": "3", "colors": ["U"],
            "power": "1", "toughness": "2", "defense": "4"
        })))
        .unwrap();
        let face = &card.front;
        assert_eq!(face.name, "Test Card");
        assert_eq!(face.mana_cost, "{2}{U}{U}");
        assert_eq!(face.oracle_text, "+2: ...");
        assert_eq!(face.loyalty.as_deref(), Some("3"));
        assert_eq!(face.power.as_deref(), Some("1"));
        assert_eq!(face.toughness.as_deref(), Some("2"));
        assert_eq!(face.defense.as_deref(), Some("4"));
        assert_eq!(face.colors, card.colors);
    }

    #[test]
    fn multi_faced_cards_keep_face_order_and_data() {
        let card = convert(base(json!({
            "layout": "transform",
            "card_faces": [
                {"name": "Front", "oracle_text": "a", "power": "1", "toughness": "1"},
                {"name": "Middle", "oracle_text": "b"},
                {"name": "Back", "oracle_text": "c", "loyalty": "4"}
            ]
        })))
        .unwrap();
        let names: Vec<_> = card.faces().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Front", "Middle", "Back"]);
        assert_eq!(card.front.power.as_deref(), Some("1"));
        assert_eq!(card.back_faces[1].loyalty.as_deref(), Some("4"));
    }

    #[test]
    fn oracle_id_may_live_on_the_faces() {
        let card = convert(json!({
            "id": "p", "name": "Flip Side // Flip Side", "layout": "reversible_card",
            "card_faces": [{"name": "Flip Side", "oracle_id": "face-oracle"}, {"name": "Flip Side"}]
        }))
        .unwrap();
        assert_eq!(card.oracle_id, OracleId("face-oracle".into()));
        assert_eq!(card.layout, Layout::ReversibleCard);
    }

    #[rstest]
    #[case::no_name(json!({"id": "p", "oracle_id": "o"}), InvalidRecord::MissingName)]
    #[case::empty_name(json!({"id": "p", "oracle_id": "o", "name": ""}), InvalidRecord::MissingName)]
    #[case::null_name(json!({"id": "p", "oracle_id": "o", "name": null}), InvalidRecord::MissingName)]
    #[case::no_id(json!({"oracle_id": "o", "name": "X"}), InvalidRecord::MissingId { name: "X".into() })]
    #[case::no_oracle_id(json!({"id": "p", "name": "X"}), InvalidRecord::MissingOracleId { name: "X".into() })]
    #[case::empty_oracle_id(json!({"id": "p", "oracle_id": "", "name": "X"}), InvalidRecord::MissingOracleId { name: "X".into() })]
    #[case::nameless_face(
        json!({"id": "p", "oracle_id": "o", "name": "X // Y", "card_faces": [{"name": "X"}, {"mana_cost": "{1}"}]}),
        InvalidRecord::MissingFaceName { name: "X // Y".into(), face: 1 }
    )]
    fn rejects_cards_missing_essentials(#[case] value: Value, #[case] expected: InvalidRecord) {
        assert_eq!(convert(value).unwrap_err(), expected);
    }

    #[test]
    fn unknown_and_future_fields_are_ignored() {
        let card = convert(base(json!({
            "brand_new_field": {"a": [1, 2, 3]}, "prices": {"usd": null}, "all_parts": [{"id": "x"}],
            "card_faces": null
        })))
        .unwrap();
        assert_eq!(card.name, "Test Card");
    }

    #[rstest]
    #[case(json!([]), "", false)]
    #[case(json!(["C"]), "", true)]
    #[case(json!(["W", "U", "B", "R", "G"]), "WUBRG", false)]
    #[case(json!(["G", "C", "R"]), "RG", true)]
    #[case(json!(["X", "", "R"]), "R", false)]
    fn produced_mana_splits_colorless(
        #[case] symbols: Value,
        #[case] colors: &str,
        #[case] colorless: bool,
    ) {
        let card = convert(base(json!({"produced_mana": symbols}))).unwrap();
        assert_eq!(card.produced_mana.colorless, colorless);
        assert_eq!(color_string(card.produced_mana.colors), colors);
    }

    #[rstest]
    #[case(json!([]), "")]
    #[case(json!(["W", "U"]), "WU")]
    #[case(json!(["G", "W", "U", "B"]), "WUBG")]
    #[case(json!(["?", "Z"]), "")]
    fn color_identity_ignores_unknown_symbols(#[case] symbols: Value, #[case] expected: &str) {
        let card = convert(base(json!({"color_identity": symbols}))).unwrap();
        assert_eq!(color_string(card.color_identity), expected);
    }

    #[rstest]
    #[case("legal", Legality::Legal)]
    #[case("not_legal", Legality::NotLegal)]
    #[case("restricted", Legality::Restricted)]
    #[case("banned", Legality::Banned)]
    #[case("suspended", Legality::Unknown)]
    #[case("", Legality::Unknown)]
    fn legality_keywords(#[case] keyword: &str, #[case] expected: Legality) {
        let card = convert(base(json!({"legalities": {"commander": keyword}}))).unwrap();
        assert_eq!(card.legality("commander"), expected);
        assert_eq!(card.legality("no-such-format"), Legality::Unknown);
    }

    #[rstest]
    #[case("common", Some(Rarity::Common))]
    #[case("uncommon", Some(Rarity::Uncommon))]
    #[case("rare", Some(Rarity::Rare))]
    #[case("mythic", Some(Rarity::Mythic))]
    #[case("special", Some(Rarity::Special))]
    #[case("bonus", Some(Rarity::Bonus))]
    #[case("legendary", None)]
    fn rarity_keywords(#[case] keyword: &str, #[case] expected: Option<Rarity>) {
        let card = convert(base(json!({"rarity": keyword}))).unwrap();
        assert_eq!(card.rarity, expected);
    }

    #[rstest]
    #[case("normal", Layout::Normal, true)]
    #[case("split", Layout::Split, true)]
    #[case("flip", Layout::Flip, true)]
    #[case("transform", Layout::Transform, true)]
    #[case("modal_dfc", Layout::ModalDfc, true)]
    #[case("meld", Layout::Meld, true)]
    #[case("leveler", Layout::Leveler, true)]
    #[case("class", Layout::Class, true)]
    #[case("case", Layout::Case, true)]
    #[case("saga", Layout::Saga, true)]
    #[case("adventure", Layout::Adventure, true)]
    #[case("mutate", Layout::Mutate, true)]
    #[case("prototype", Layout::Prototype, true)]
    #[case("battle", Layout::Battle, true)]
    #[case("augment", Layout::Augment, true)]
    #[case("host", Layout::Host, true)]
    #[case("reversible_card", Layout::ReversibleCard, true)]
    #[case("planar", Layout::Planar, false)]
    #[case("scheme", Layout::Scheme, false)]
    #[case("vanguard", Layout::Vanguard, false)]
    #[case("token", Layout::Token, false)]
    #[case("double_faced_token", Layout::DoubleFacedToken, false)]
    #[case("emblem", Layout::Emblem, false)]
    #[case("art_series", Layout::ArtSeries, false)]
    #[case("brand_new_layout", Layout::Other("brand_new_layout".into()), true)]
    fn layouts_and_deck_eligibility(
        #[case] keyword: &str,
        #[case] expected: Layout,
        #[case] deck_card: bool,
    ) {
        let layout = Layout::parse(keyword);
        assert_eq!(layout, expected);
        assert_eq!(layout.is_deck_card(), deck_card);
    }

    #[test]
    fn face_images_fall_back_to_card_images() {
        let card = convert(base(json!({
            "image_uris": {"normal": "root"},
            "card_faces": [
                {"name": "A", "image_uris": {"normal": "front"}},
                {"name": "B"}
            ]
        })))
        .unwrap();
        let normal = |i| card.face_images(i).and_then(|u| u.normal.as_deref());
        assert_eq!(normal(0), Some("front"));
        assert_eq!(normal(1), Some("root"));
        assert_eq!(normal(2), Some("root"));
        let bare = convert(base(json!({}))).unwrap();
        assert_eq!(bare.face_images(0), None);
    }

    #[rstest]
    #[case(json!({"source": "wotc"}), RulingSource::Wotc)]
    #[case(json!({"source": "scryfall"}), RulingSource::Scryfall)]
    #[case(json!({"source": "someone-new"}), RulingSource::Unknown)]
    #[case(json!({}), RulingSource::Unknown)]
    fn ruling_sources(#[case] extra: Value, #[case] expected: RulingSource) {
        let mut value = json!({"oracle_id": "o", "comment": "text", "published_at": "2020-01-01"});
        value
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let ruling =
            RulingRecord::try_from(serde_json::from_value::<RawRuling>(value).unwrap()).unwrap();
        assert_eq!(ruling.source, expected);
        assert_eq!(ruling.comment, "text");
        assert_eq!(ruling.published_at.as_deref(), Some("2020-01-01"));
    }

    #[rstest]
    #[case(json!({"comment": "x"}), InvalidRecord::RulingWithoutOracleId)]
    #[case(json!({"oracle_id": ""}), InvalidRecord::RulingWithoutOracleId)]
    #[case(json!({"oracle_id": "o"}), InvalidRecord::RulingWithoutComment { oracle_id: "o".into() })]
    #[case(json!({"oracle_id": "o", "comment": ""}), InvalidRecord::RulingWithoutComment { oracle_id: "o".into() })]
    fn rejects_incomplete_rulings(#[case] value: Value, #[case] expected: InvalidRecord) {
        let raw = serde_json::from_value::<RawRuling>(value).unwrap();
        assert_eq!(RulingRecord::try_from(raw).unwrap_err(), expected);
    }

    proptest! {
        #[test]
        fn arbitrary_objects_never_panic(
            id in prop::option::of("[a-z0-9-]{0,8}"),
            oracle in prop::option::of("[a-z0-9-]{0,8}"),
            name in prop::option::of("\\PC{0,12}"),
            layout in prop::option::of("[a-z_]{0,12}"),
            cmc in prop::option::of(-5.0f64..20.0),
            colors in prop::option::of(prop::collection::vec("[A-Za-z?]{0,2}", 0..6)),
            faces in prop::option::of(prop::collection::vec(
                (prop::option::of("\\PC{0,8}"), prop::option::of("[{}A-Z0-9/]{0,8}")),
                0..4,
            )),
        ) {
            let faces = faces.map(|f| {
                f.into_iter()
                    .map(|(name, cost)| json!({"name": name, "mana_cost": cost}))
                    .collect::<Vec<_>>()
            });
            let value = json!({
                "id": id, "oracle_id": oracle, "name": name, "layout": layout,
                "cmc": cmc, "colors": colors, "card_faces": faces,
            });
            if let Ok(card) = convert(value) {
                prop_assert!(!card.name.is_empty());
                prop_assert!(card.faces().count() >= 1);
                prop_assert!(card.faces().all(|f| !f.name.is_empty()));
            }
        }

        #[test]
        fn records_survive_a_json_round_trip(
            // Mana values are multiples of one half; arbitrary floats can lose an ULP in serde_json.
            halves in 0u32..2_000_000,
            colors in prop::collection::vec(prop::sample::select(vec!["W", "U", "B", "R", "G"]), 0..5),
            keywords in prop::collection::vec("[A-Za-z ]{1,10}", 0..4),
        ) {
            let cmc = f64::from(halves) / 2.0;
            let card = convert(base(json!({"cmc": cmc, "colors": colors, "keywords": keywords, "layout": "saga"}))).unwrap();
            let text = serde_json::to_string(&card).unwrap();
            prop_assert_eq!(serde_json::from_str::<CardRecord>(&text).unwrap(), card);
        }
    }
}
