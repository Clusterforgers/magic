//! Text encodings of `mtg_import` enums, as Scryfall spells them and as stored in Postgres.

use std::borrow::Cow;

use mtg_import::scryfall::{Layout, Legality, Rarity, RulingSource};

/// Scryfall keyword of a layout; the inverse of [`Layout::parse`].
pub fn layout_keyword(layout: &Layout) -> Cow<'_, str> {
    Cow::Borrowed(match layout {
        Layout::Normal => "normal",
        Layout::Split => "split",
        Layout::Flip => "flip",
        Layout::Transform => "transform",
        Layout::ModalDfc => "modal_dfc",
        Layout::Meld => "meld",
        Layout::Leveler => "leveler",
        Layout::Class => "class",
        Layout::Case => "case",
        Layout::Saga => "saga",
        Layout::Adventure => "adventure",
        Layout::Mutate => "mutate",
        Layout::Prototype => "prototype",
        Layout::Battle => "battle",
        Layout::Planar => "planar",
        Layout::Scheme => "scheme",
        Layout::Vanguard => "vanguard",
        Layout::Token => "token",
        Layout::DoubleFacedToken => "double_faced_token",
        Layout::Emblem => "emblem",
        Layout::Augment => "augment",
        Layout::Host => "host",
        Layout::ArtSeries => "art_series",
        Layout::FrontCard => "front_card",
        Layout::ReversibleCard => "reversible_card",
        Layout::Other(keyword) => return Cow::Borrowed(keyword),
    })
}

/// Scryfall keyword of a rarity; the inverse of [`Rarity::parse`].
pub const fn rarity_keyword(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "common",
        Rarity::Uncommon => "uncommon",
        Rarity::Rare => "rare",
        Rarity::Mythic => "mythic",
        Rarity::Special => "special",
        Rarity::Bonus => "bonus",
    }
}

/// Scryfall keyword of a legality; the inverse of [`Legality::parse`].
pub const fn legality_keyword(legality: Legality) -> &'static str {
    match legality {
        Legality::Legal => "legal",
        Legality::NotLegal => "not_legal",
        Legality::Restricted => "restricted",
        Legality::Banned => "banned",
        Legality::Unknown => "unknown",
    }
}

/// Stored keyword of a ruling source.
pub const fn ruling_source_keyword(source: RulingSource) -> &'static str {
    match source {
        RulingSource::Wotc => "wotc",
        RulingSource::Scryfall => "scryfall",
        RulingSource::Unknown => "unknown",
    }
}

/// Reads a stored ruling source; anything unrecognized is [`RulingSource::Unknown`].
pub fn ruling_source_from_keyword(keyword: &str) -> RulingSource {
    match keyword {
        "wotc" => RulingSource::Wotc,
        "scryfall" => RulingSource::Scryfall,
        _ => RulingSource::Unknown,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every enum value survives the trip through its stored keyword, so reading back never changes imported data.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(Layout::Normal)]
    #[case(Layout::Split)]
    #[case(Layout::Flip)]
    #[case(Layout::Transform)]
    #[case(Layout::ModalDfc)]
    #[case(Layout::Meld)]
    #[case(Layout::Leveler)]
    #[case(Layout::Class)]
    #[case(Layout::Case)]
    #[case(Layout::Saga)]
    #[case(Layout::Adventure)]
    #[case(Layout::Mutate)]
    #[case(Layout::Prototype)]
    #[case(Layout::Battle)]
    #[case(Layout::Planar)]
    #[case(Layout::Scheme)]
    #[case(Layout::Vanguard)]
    #[case(Layout::Token)]
    #[case(Layout::DoubleFacedToken)]
    #[case(Layout::Emblem)]
    #[case(Layout::Augment)]
    #[case(Layout::Host)]
    #[case(Layout::ArtSeries)]
    #[case(Layout::FrontCard)]
    #[case(Layout::ReversibleCard)]
    #[case(Layout::Other("future_layout".into()))]
    fn layouts_round_trip(#[case] layout: Layout) {
        assert_eq!(Layout::parse(&layout_keyword(&layout)), layout);
    }

    #[rstest]
    #[case(Rarity::Common)]
    #[case(Rarity::Uncommon)]
    #[case(Rarity::Rare)]
    #[case(Rarity::Mythic)]
    #[case(Rarity::Special)]
    #[case(Rarity::Bonus)]
    fn rarities_round_trip(#[case] rarity: Rarity) {
        assert_eq!(Rarity::parse(rarity_keyword(rarity)), Some(rarity));
    }

    #[rstest]
    #[case(Legality::Legal)]
    #[case(Legality::NotLegal)]
    #[case(Legality::Restricted)]
    #[case(Legality::Banned)]
    #[case(Legality::Unknown)]
    fn legalities_round_trip(#[case] legality: Legality) {
        assert_eq!(Legality::parse(legality_keyword(legality)), legality);
    }

    #[rstest]
    #[case(RulingSource::Wotc)]
    #[case(RulingSource::Scryfall)]
    #[case(RulingSource::Unknown)]
    fn ruling_sources_round_trip(#[case] source: RulingSource) {
        assert_eq!(
            ruling_source_from_keyword(ruling_source_keyword(source)),
            source
        );
    }

    #[test]
    fn unrecognized_ruling_sources_read_as_unknown() {
        assert_eq!(ruling_source_from_keyword("from-the-future"), RulingSource::Unknown);
    }
}
