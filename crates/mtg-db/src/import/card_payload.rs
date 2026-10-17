//! Conversion of normalized Scryfall records into the JSON rows the upsert statements read.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use mtg_import::resolve::normalize_name;
use mtg_import::scryfall::CardRecord;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::codec::{layout_keyword, legality_keyword, rarity_keyword};
use crate::colors::to_mask;
use crate::model::{PrintingImages, StoredFace};

/// Separator between the face names of a multi-faced card.
const FACE_SEPARATOR: &str = " // ";

/// Why a record cannot be stored; such records are skipped, not fatal.
#[derive(Debug, Error, PartialEq)]
pub(crate) enum Rejection {
    /// The oracle id is not a UUID.
    #[error("oracle id {0:?} is not a UUID")]
    OracleId(String),
    /// The printing id is not a UUID.
    #[error("printing id {0:?} is not a UUID")]
    PrintingId(String),
    /// Nothing is left of the name after normalization.
    #[error("name {0:?} is empty once normalized")]
    BlankName(String),
    /// The release or publication date is not `YYYY-MM-DD`.
    #[error("date {0:?} is not YYYY-MM-DD")]
    Date(String),
    /// A ruling has no text.
    #[error("ruling text is empty")]
    BlankComment,
    /// A deck entry has more copies than the database can hold.
    #[error("a deck entry has too many copies")]
    CountTooLarge,
    /// The mana value is negative or not finite.
    #[error("mana value {0} is not a non-negative finite number")]
    ManaValue(f64),
}

/// A `cards` row in the shape of the upsert statement's `jsonb_to_recordset` columns.
#[derive(Debug, Serialize)]
pub(crate) struct CardPayload {
    pub oracle_id: Uuid,
    name: String,
    name_normalized: String,
    front_name_normalized: String,
    layout: String,
    is_deck_card: bool,
    mana_cost: String,
    mana_value: f64,
    type_line: String,
    oracle_text: String,
    keywords: Vec<String>,
    colors: i16,
    color_identity: i16,
    produced_mana: i16,
    produces_colorless: bool,
    legalities: BTreeMap<String, &'static str>,
    faces: Vec<StoredFace>,
}

/// A `printings` row in the shape of the upsert statement's `jsonb_to_recordset` columns.
#[derive(Debug, Serialize)]
pub(crate) struct PrintingPayload {
    pub printing_id: Uuid,
    oracle_id: Uuid,
    set_code: String,
    collector_number: String,
    rarity: Option<&'static str>,
    released_at: Option<String>,
    lang: String,
    digital: bool,
    images: PrintingImages,
}

/// Splits a record into its oracle-level and printing-level rows.
pub(crate) fn split_record(
    record: &CardRecord,
) -> Result<(CardPayload, PrintingPayload), Rejection> {
    let oracle_id = Uuid::parse_str(record.oracle_id.as_str())
        .map_err(|_| Rejection::OracleId(record.oracle_id.to_string()))?;
    let printing_id = Uuid::parse_str(record.printing_id.as_str())
        .map_err(|_| Rejection::PrintingId(record.printing_id.to_string()))?;
    let released_at = record.released_at.as_deref().map(iso_date).transpose()?;
    if !record.mana_value.is_finite() || record.mana_value < 0.0 {
        return Err(Rejection::ManaValue(record.mana_value));
    }
    let name_normalized = normalize_name(&record.name);
    if name_normalized.is_empty() {
        return Err(Rejection::BlankName(record.name.clone()));
    }
    // Same front-face rule as `CardIndex`, so exact lookups agree between memory and database.
    let front_name_normalized = name_normalized
        .split(FACE_SEPARATOR)
        .next()
        .unwrap_or(&name_normalized)
        .to_owned();

    let card = CardPayload {
        oracle_id,
        name: record.name.trim().to_owned(),
        name_normalized,
        front_name_normalized,
        layout: layout_keyword(&record.layout).into_owned(),
        is_deck_card: record.layout.is_deck_card(),
        mana_cost: record.mana_cost.clone(),
        mana_value: record.mana_value,
        type_line: record.type_line.clone(),
        oracle_text: joined_oracle_text(record),
        keywords: record.keywords.clone(),
        colors: to_mask(record.colors),
        color_identity: to_mask(record.color_identity),
        produced_mana: to_mask(record.produced_mana.colors),
        produces_colorless: record.produced_mana.colorless,
        legalities: record
            .legalities
            .iter()
            .map(|(format, legality)| (format.clone(), legality_keyword(*legality)))
            .collect(),
        faces: record
            .faces()
            .map(|face| StoredFace {
                name: face.name.clone(),
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone(),
                oracle_text: face.oracle_text.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
                colors: face.colors,
            })
            .collect(),
    };
    let printing = PrintingPayload {
        printing_id,
        oracle_id,
        set_code: record.set.clone(),
        collector_number: record.collector_number.clone(),
        rarity: record.rarity.map(rarity_keyword),
        released_at,
        lang: record.lang.clone(),
        digital: record.digital,
        images: PrintingImages {
            card: record.images.clone(),
            faces: record.faces().map(|face| face.images.clone()).collect(),
        },
    };
    Ok((card, printing))
}

/// Checks that `text` is a real `YYYY-MM-DD` date and returns it in canonical form.
pub(crate) fn iso_date(text: &str) -> Result<String, Rejection> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map(|date| date.to_string())
        .map_err(|_| Rejection::Date(text.to_owned()))
}

/// Rules text of every face, separated by a `//` line; empty when no face has text.
fn joined_oracle_text(record: &CardRecord) -> String {
    let texts: Vec<&str> = record
        .faces()
        .map(|face| face.oracle_text.as_str())
        .collect();
    if texts.iter().all(|text| text.is_empty()) {
        String::new()
    } else {
        texts.join("\n//\n")
    }
}

#[cfg(test)]
mod tests {
    //! Goal: records become storable rows exactly when their ids, dates and numbers are usable, and multi-face text and names are folded consistently with the in-memory card index.
    use mtg_import::ids::OracleId;
    use mtg_import::scryfall::cards;
    use rstest::rstest;

    use super::*;

    const FIXTURE: &str =
        include_str!("../../../mtg-import/tests/fixtures/scryfall_oracle_cards.json");

    fn record(name: &str) -> CardRecord {
        cards(FIXTURE.as_bytes())
            .filter_map(Result::ok)
            .find(|card| card.name == name)
            .unwrap_or_else(|| panic!("fixture has no card {name:?}"))
    }

    #[rstest]
    #[case::bad_oracle_id(|r: &mut CardRecord| r.oracle_id = OracleId("not-a-uuid".into()), Rejection::OracleId("not-a-uuid".into()))]
    #[case::bad_printing_id(|r: &mut CardRecord| r.printing_id.0 = "xyz".into(), Rejection::PrintingId("xyz".into()))]
    #[case::bad_date(|r: &mut CardRecord| r.released_at = Some("2009-13-45".into()), Rejection::Date("2009-13-45".into()))]
    #[case::date_with_time(|r: &mut CardRecord| r.released_at = Some("2009-07-17T00:00".into()), Rejection::Date("2009-07-17T00:00".into()))]
    #[case::negative_mana_value(|r: &mut CardRecord| r.mana_value = -1.0, Rejection::ManaValue(-1.0))]
    #[case::infinite_mana_value(|r: &mut CardRecord| r.mana_value = f64::INFINITY, Rejection::ManaValue(f64::INFINITY))]
    #[case::blank_name(|r: &mut CardRecord| r.name = " \u{200B} ".into(), Rejection::BlankName(" \u{200B} ".into()))]
    fn unusable_records_are_rejected(
        #[case] damage: fn(&mut CardRecord),
        #[case] expected: Rejection,
    ) {
        let mut bolt = record("Lightning Bolt");
        damage(&mut bolt);
        assert_eq!(split_record(&bolt).unwrap_err(), expected);
    }

    #[test]
    fn a_missing_release_date_is_allowed() {
        let mut bolt = record("Lightning Bolt");
        bolt.released_at = None;
        assert!(split_record(&bolt).unwrap().1.released_at.is_none());
    }

    #[rstest]
    #[case("Lightning Bolt", "lightning bolt", "lightning bolt")]
    #[case("Fire // Ice", "fire // ice", "fire")]
    #[case("Æther Vial", "aether vial", "aether vial")]
    #[case(
        "Atraxa, Praetors' Voice",
        "atraxa, praetors' voice",
        "atraxa, praetors' voice"
    )]
    fn names_are_normalized_like_the_card_index(
        #[case] name: &str,
        #[case] full: &str,
        #[case] front: &str,
    ) {
        let (card, _) = split_record(&record(name)).unwrap();
        assert_eq!(card.name_normalized, full);
        assert_eq!(card.front_name_normalized, front);
    }

    #[rstest]
    #[case("Lightning Bolt", "Lightning Bolt deals 3 damage to any target.")]
    #[case(
        "Fire // Ice",
        "Fire deals 2 damage divided as you choose among one or two targets.\n//\nTap target permanent.\nDraw a card."
    )]
    #[case("Mountain", "({T}: Add {R}.)")]
    #[case("Mystery Card", "")]
    fn oracle_text_joins_faces_with_a_slash_line(#[case] name: &str, #[case] expected: &str) {
        let (card, _) = split_record(&record(name)).unwrap();
        assert_eq!(card.oracle_text, expected);
    }

    #[test]
    fn printing_images_keep_card_level_and_per_face_art_apart() {
        let (_, printing) =
            split_record(&record("Delver of Secrets // Insectile Aberration")).unwrap();
        assert!(printing.images.card.is_none());
        assert_eq!(printing.images.faces.len(), 2);
        assert!(printing.images.faces.iter().all(Option::is_some));
        let (_, bolt) = split_record(&record("Lightning Bolt")).unwrap();
        assert!(bolt.images.card.is_some());
        assert_eq!(bolt.images.faces, vec![None]);
    }
}
