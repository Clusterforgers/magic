//! The fingerprint of a deck: what a game's recipe remembers of the cards it was created with.
//!
//! A recipe names a built-in deck by its id, so a deploy that edits the deck would replay a stored
//! event log under other cards and nothing would fail. The fingerprint is a SHA-256 over a canonical
//! encoding of the ordered card definitions the engine is given; resume computes it again from the
//! deck as it is now and refuses a game whose deck no longer hashes to what was stored.
//!
//! The encoding, version 1, is UTF-8 text: the line `mtg-deck/v1`, the number of cards, then one
//! line per card in deck order holding the card's serde JSON with every object's keys sorted and no
//! whitespace. It follows the card's serde form, so a change to any field of [`CardDefinition`], or to
//! how one is written, changes the fingerprint; that includes a new field, which changes every
//! fingerprint unless it is left out of the JSON while it holds its default (the pinned values in the
//! tests notice). The stored text is `v1:` and the 64 lowercase hex digits of the digest. A different
//! encoding is a new version; this server reads only the one it writes, so a recipe stored with
//! another version does not decode and its game fails with that reason instead of going unchecked.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use mtg_core::card::CardDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// The version of the canonical encoding; it is hashed in and written before the stored digest.
pub const FINGERPRINT_VERSION: u32 = 1;

/// Bytes in a digest.
const DIGEST_LEN: usize = 32;

/// SHA-256 over the canonical encoding of one deck's ordered cards; stored as `v1:` and 64 hex digits.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeckFingerprint([u8; DIGEST_LEN]);

/// Why a deck could not be fingerprinted or a stored fingerprint could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FingerprintError {
    /// A card would not serialise, which no card type of today does.
    #[error("card {card:?} cannot be encoded: {reason}")]
    Encode {
        /// The card's name.
        card: String,
        /// What the serialiser said.
        reason: String,
    },
    /// The text is not a fingerprint of the version this server reads.
    #[error("not a deck fingerprint this server can read: {0:?}")]
    Malformed(String),
}

impl DeckFingerprint {
    /// The fingerprint of `cards`, the deck exactly as the engine is given it: order and size count.
    pub fn of(cards: &[Arc<CardDefinition>]) -> Result<DeckFingerprint, FingerprintError> {
        let text = encode(cards)?;
        Ok(DeckFingerprint(Sha256::digest(text.as_bytes()).into()))
    }
}

impl fmt::Display for DeckFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{FINGERPRINT_VERSION}:")?;
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for DeckFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeckFingerprint({self})")
    }
}

impl FromStr for DeckFingerprint {
    type Err = FingerprintError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || FingerprintError::Malformed(text.to_owned());
        let digits = text
            .strip_prefix(&format!("v{FINGERPRINT_VERSION}:"))
            .filter(|digits| digits.len() == 2 * DIGEST_LEN)
            .ok_or_else(malformed)?;
        let mut bytes = [0; DIGEST_LEN];
        let (pairs, _) = digits.as_bytes().as_chunks::<2>();
        for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
            *byte = hex_value(*high)
                .zip(hex_value(*low))
                .map(|(high, low)| (high << 4) | low)
                .ok_or_else(malformed)?;
        }
        Ok(DeckFingerprint(bytes))
    }
}

impl TryFrom<String> for DeckFingerprint {
    type Error = FingerprintError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<DeckFingerprint> for String {
    fn from(fingerprint: DeckFingerprint) -> String {
        fingerprint.to_string()
    }
}

/// The value of one lowercase hex digit.
fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

/// The canonical text of a deck, described in the module docs.
fn encode(cards: &[Arc<CardDefinition>]) -> Result<String, FingerprintError> {
    let mut text = format!("mtg-deck/v{FINGERPRINT_VERSION}\n{}\n", cards.len());
    for card in cards {
        let unencodable = |error: serde_json::Error| FingerprintError::Encode {
            card: card.name.clone(),
            reason: error.to_string(),
        };
        let value = serde_json::to_value(card.as_ref()).map_err(unencodable)?;
        write_sorted(&value, &mut text).map_err(unencodable)?;
        text.push('\n');
    }
    Ok(text)
}

/// Appends `value` as compact JSON with every object's keys in byte order, whatever order the value
/// holds them in, so the text does not depend on how a struct happens to declare its fields.
fn write_sorted(value: &Value, out: &mut String) -> Result<(), serde_json::Error> {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_unstable_by_key(|(key, _)| *key);
            out.push('{');
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key)?);
                out.push(':');
                write_sorted(item, out)?;
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_sorted(item, out)?;
            }
            out.push(']');
        }
        // A scalar has one spelling: integers in decimal, strings escaped, no spaces.
        scalar => out.push_str(&serde_json::to_string(scalar)?),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Goal: a deck's fingerprint is stable (pinned for the demo decks and the canonical text of a small deck), changes with any field of any card and with the order and size of the list, and its stored text round-trips and refuses everything else.
    use std::collections::BTreeSet;

    use mtg_cards::vanilla::{basic_land, creature};
    use mtg_core::card::{AbilityDef, CardType, ManaAbility, Subtype, Supertype};
    use mtg_core::color::Color;
    use mtg_core::mana::{ManaCost, ManaType};
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::demo;

    fn deck(id: &str) -> Vec<Arc<CardDefinition>> {
        demo::find(id).unwrap().cards()
    }

    fn fingerprint(cards: &[Arc<CardDefinition>]) -> DeckFingerprint {
        DeckFingerprint::of(cards).unwrap()
    }

    /// A card with every field filled in, so that each one can be changed.
    fn full_card() -> CardDefinition {
        CardDefinition::new("Ember Elf")
            .with_mana_cost(ManaCost::parse("{1}{G}").unwrap())
            .with_supertype(Supertype::Legendary)
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_subtype("Druid")
            .with_power_toughness(2, 3)
            .with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green,
            }))
    }

    #[test]
    fn the_canonical_text_of_a_small_deck_is_pinned() {
        let cards = [
            basic_land(Color::Green),
            creature("Grizzly Bears", 1, &[Color::Green], 2, 2),
        ];
        let expected = concat!(
            "mtg-deck/v1\n",
            "2\n",
            r#"{"abilities":[{"Mana":{"produces":"Green"}}],"mana_cost":null,"name":"Forest","power":null,"subtypes":["Forest"],"supertypes":["Basic"],"toughness":null,"types":["Land"]}"#,
            "\n",
            r#"{"abilities":[],"mana_cost":[{"Generic":1},{"Colored":"Green"}],"name":"Grizzly Bears","power":2,"subtypes":[],"supertypes":[],"toughness":2,"types":["Creature"]}"#,
            "\n",
        );
        assert_eq!(encode(&cards).unwrap(), expected);
        // The digest of exactly that text, as `sha256sum` computes it.
        assert_eq!(
            fingerprint(&cards).to_string(),
            "v1:3e6fd213a94b896b4838e238bd16bbbe62e1d2cf2b8aec7e8fe9b07dc4892f8e"
        );
    }

    #[rstest]
    #[case::green(
        "mono-green",
        "v1:7cab379fe30b1810069843fcf113cbbbfa8f11784d9772bdd68b49c77e0c1ceb"
    )]
    #[case::red(
        "mono-red",
        "v1:9f1f1eb342c24e939e463142ee4dadfd1c63ff8a9deaceda80f5ca501b26088f"
    )]
    #[case::white(
        "mono-white",
        "v1:8f905804b4bd079dfe4da0db799f314b16112e3f377a5456ed1b7e490c768f6a"
    )]
    fn demo_decks_have_pinned_fingerprints(#[case] id: &str, #[case] pinned: &str) {
        assert_eq!(fingerprint(&deck(id)).to_string(), pinned, "{id}");
    }

    #[test]
    fn objects_are_written_with_sorted_keys_whatever_order_they_were_built_in() {
        let mut inner = serde_json::Map::new();
        inner.insert("z".into(), json!(1));
        inner.insert("a".into(), json!([3, {"y": 1, "x": 2}, 1]));
        let mut outer = serde_json::Map::new();
        outer.insert("b".into(), Value::Object(inner));
        outer.insert("a".into(), json!("q\"uote\\ \n é"));
        outer.insert("c".into(), json!([null, true, -4, 2.5]));
        let mut text = String::new();
        write_sorted(&Value::Object(outer), &mut text).unwrap();
        assert_eq!(
            text,
            r#"{"a":"q\"uote\\ \n é","b":{"a":[3,{"x":2,"y":1},1],"z":1},"c":[null,true,-4,2.5]}"#
        );
    }

    #[test]
    fn equal_decks_have_equal_fingerprints_however_they_were_made() {
        let cards = deck("mono-red");
        let before = fingerprint(&cards);
        assert_eq!(fingerprint(&deck("mono-red")), before, "built twice");
        assert_eq!(fingerprint(&cards.clone()), before, "cloned");
        let rebuilt: Vec<_> = cards.iter().map(|c| Arc::new((**c).clone())).collect();
        assert_eq!(
            fingerprint(&rebuilt),
            before,
            "same cards, other allocations"
        );
        let mut swapped = cards;
        swapped.swap(0, 1);
        assert_eq!(
            fingerprint(&swapped),
            before,
            "two copies of one card trade places"
        );
    }

    type Card = CardDefinition;
    type Change = fn(&mut Card);

    fn mana(produces: ManaType) -> AbilityDef {
        AbilityDef::Mana(ManaAbility { produces })
    }

    fn cost(text: &str) -> ManaCost {
        ManaCost::parse(text).unwrap()
    }

    /// Every way the tests edit a card, named by the field it edits.
    fn changes() -> Vec<(&'static str, Change)> {
        vec![
            ("name", |c: &mut Card| c.name.push('s')),
            ("name", |c: &mut Card| c.name = "ember elf".into()),
            ("mana_cost", |c: &mut Card| {
                c.mana_cost = Some(cost("{2}{G}"));
            }),
            ("mana_cost", |c: &mut Card| {
                c.mana_cost = Some(cost("{1}{R}"));
            }),
            ("mana_cost", |c: &mut Card| {
                c.mana_cost = Some(cost("{G}{G}"));
            }),
            ("mana_cost", |c: &mut Card| c.mana_cost = None),
            ("supertypes", |c: &mut Card| {
                c.supertypes.insert(Supertype::Snow);
            }),
            ("supertypes", |c: &mut Card| c.supertypes.clear()),
            ("types", |c: &mut Card| {
                c.types.insert(CardType::Artifact);
            }),
            ("types", |c: &mut Card| {
                c.types = BTreeSet::from([CardType::Land]);
            }),
            ("subtypes", |c: &mut Card| c.subtypes.reverse()),
            ("subtypes", |c: &mut Card| {
                c.subtypes.push(Subtype("Shaman".into()));
            }),
            ("subtypes", |c: &mut Card| c.subtypes.clear()),
            ("power", |c: &mut Card| c.power = Some(3)),
            ("power", |c: &mut Card| c.power = None),
            ("toughness", |c: &mut Card| c.toughness = Some(4)),
            ("toughness", |c: &mut Card| c.toughness = None),
            ("abilities", |c: &mut Card| {
                c.abilities.push(mana(ManaType::Green));
            }),
            ("abilities", |c: &mut Card| {
                c.abilities = vec![mana(ManaType::Red)];
            }),
            ("abilities", |c: &mut Card| c.abilities.clear()),
        ]
    }

    #[test]
    fn every_field_of_a_card_changes_the_fingerprint() {
        let before = fingerprint(&[Arc::new(full_card())]);
        let mut seen = BTreeSet::new();
        for (index, (field, change)) in changes().into_iter().enumerate() {
            let mut card = full_card();
            change(&mut card);
            let after = fingerprint(&[Arc::new(card)]);
            assert_ne!(after, before, "change {index} of {field} goes unnoticed");
            assert!(
                seen.insert(after.to_string()),
                "change {index} of {field} equals another"
            );
        }
    }

    #[test]
    fn the_changes_cover_every_field_a_card_serialises() {
        let card = serde_json::to_value(full_card()).unwrap();
        let fields: BTreeSet<&str> = card
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let covered: BTreeSet<&str> = changes().iter().map(|(field, _)| *field).collect();
        assert_eq!(
            covered, fields,
            "a new field of CardDefinition needs a change above"
        );
    }

    type Deck = Vec<Arc<CardDefinition>>;
    type Edit = fn(&mut Deck);

    fn hill_giant() -> Arc<CardDefinition> {
        creature("Hill Giant", 3, &[Color::Red], 3, 3)
    }

    #[rstest]
    #[case::swap_a_land_and_a_creature(|deck: &mut Deck| deck.swap(23, 24))]
    #[case::swap_the_ends(|deck: &mut Deck| deck.swap(0, 59))]
    #[case::rotate(|deck: &mut Deck| deck.rotate_left(1))]
    #[case::reverse(|deck: &mut Deck| deck.reverse())]
    #[case::sort_by_name(|deck: &mut Deck| deck.sort_by(|a, b| a.name.cmp(&b.name)))]
    #[case::drop_the_first(|deck: &mut Deck| { deck.remove(0); })]
    #[case::drop_the_last(|deck: &mut Deck| { deck.pop(); })]
    #[case::add_a_copy(|deck: &mut Deck| deck.push(deck[0].clone()))]
    #[case::add_another_card(|deck: &mut Deck| deck.push(hill_giant()))]
    #[case::replace_one_copy(|deck: &mut Deck| deck[0] = hill_giant())]
    #[case::double(|deck: &mut Deck| deck.extend(deck.clone()))]
    #[case::empty(|deck: &mut Deck| deck.clear())]
    fn a_change_to_the_list_changes_the_fingerprint(#[case] edit: Edit) {
        let mut cards = deck("mono-green");
        let before = fingerprint(&cards);
        edit(&mut cards);
        assert_ne!(fingerprint(&cards), before);
    }

    #[test]
    fn an_empty_deck_and_a_one_card_deck_have_fingerprints_of_their_own() {
        let none = fingerprint(&[]);
        let one = fingerprint(&[basic_land(Color::Green)]);
        let two = fingerprint(&[basic_land(Color::Green), basic_land(Color::Green)]);
        assert_eq!(
            BTreeSet::from([none, one, two].map(|f| f.to_string())).len(),
            3
        );
        assert_eq!(
            encode(&[]).unwrap(),
            format!("mtg-deck/v{FINGERPRINT_VERSION}\n0\n")
        );
    }

    const GOOD: &str = "v1:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[rstest]
    #[case::zeros([0; 32])]
    #[case::ones([0xff; 32])]
    #[case::ascending(std::array::from_fn(|i| u8::try_from(i).unwrap()))]
    fn the_stored_text_round_trips(#[case] bytes: [u8; DIGEST_LEN]) {
        let fingerprint = DeckFingerprint(bytes);
        let text = fingerprint.to_string();
        assert_eq!((text.len(), &text[..3]), (67, "v1:"));
        assert_eq!(text.parse::<DeckFingerprint>(), Ok(fingerprint));
        let value = serde_json::to_value(fingerprint).unwrap();
        assert_eq!(value, json!(text));
        assert_eq!(
            serde_json::from_value::<DeckFingerprint>(value).unwrap(),
            fingerprint
        );
        assert_eq!(
            format!("{fingerprint:?}"),
            format!("DeckFingerprint({text})")
        );
        assert_eq!(GOOD.parse::<DeckFingerprint>().unwrap().to_string(), GOOD);
    }

    #[rstest]
    #[case::empty(String::new())]
    #[case::no_version(GOOD[3..].to_owned())]
    #[case::another_version(GOOD.replace("v1", "v2"))]
    #[case::version_two_digits(GOOD.replace("v1", "v10"))]
    #[case::upper_case_digits(format!("v1:{}", GOOD[3..].to_uppercase()))]
    #[case::too_short(GOOD[..GOOD.len() - 2].to_owned())]
    #[case::too_long(format!("{GOOD}00"))]
    #[case::not_hex(format!("v1:{}", "zz".repeat(32)))]
    #[case::multi_byte(format!("v1:{}", "é".repeat(32)))]
    #[case::padded(format!(" {GOOD}"))]
    #[case::newline(format!("{GOOD}\n"))]
    fn malformed_text_is_refused(#[case] text: String) {
        assert_eq!(
            text.parse::<DeckFingerprint>(),
            Err(FingerprintError::Malformed(text.clone()))
        );
        assert!(serde_json::from_value::<DeckFingerprint>(json!(text)).is_err());
    }

    #[rstest]
    #[case::number(json!(7))]
    #[case::null(json!(null))]
    #[case::bytes(json!([0, 1, 2]))]
    #[case::object(json!({ "v1": GOOD }))]
    fn only_a_string_decodes(#[case] stored: serde_json::Value) {
        assert!(serde_json::from_value::<DeckFingerprint>(stored).is_err());
    }
}
