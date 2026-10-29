//! From a card identity, a face and a size to the link the database stores.

use mtg_core::color::ColorSet;
use mtg_db::{Card, CardRepo, DbError, PrintingImages, TokenPictures};
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::scryfall::ImageUris;

/// The sizes of a picture, as Scryfall serves them. The picture is never resized or cropped
/// here: `art_crop` is the crop Scryfall provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Size {
    Small,
    Normal,
    Large,
    ArtCrop,
    Png,
}

impl Size {
    /// The sizes the endpoint knows, as written in its path.
    pub(crate) const NAMES: &'static str = "small, normal, large, art_crop or png";

    pub(crate) fn parse(text: &str) -> Option<Size> {
        match text {
            "small" => Some(Size::Small),
            "normal" => Some(Size::Normal),
            "large" => Some(Size::Large),
            "art_crop" => Some(Size::ArtCrop),
            "png" => Some(Size::Png),
            _ => None,
        }
    }

    fn link(self, uris: &ImageUris) -> Option<&str> {
        match self {
            Size::Small => uris.small.as_deref(),
            Size::Normal => uris.normal.as_deref(),
            Size::Large => uris.large.as_deref(),
            Size::ArtCrop => uris.art_crop.as_deref(),
            Size::Png => uris.png.as_deref(),
        }
    }
}

/// A token as the table knows it; every part given must match the stored token.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TokenQuery {
    pub(crate) name: String,
    /// The type line, with or without Scryfall's leading `Token`.
    pub(crate) type_line: Option<String>,
    pub(crate) colors: Option<ColorSet>,
    pub(crate) power: Option<String>,
    pub(crate) toughness: Option<String>,
}

/// What a picture is asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Identity {
    /// A card: the picture of its newest paper printing that has one.
    Oracle(OracleId),
    /// One printing.
    Printing(PrintingId),
    /// A token by its facts.
    Token(TokenQuery),
}

/// The picture of face `face` of a printing: the face's own when the printing has face pictures,
/// else the card's, which shows every face (a split card, an adventure).
pub(crate) fn uris_of(images: &PrintingImages, face: usize) -> Option<&ImageUris> {
    if images.faces.iter().any(Option::is_some) {
        return images.faces.get(face)?.as_ref();
    }
    if face >= images.faces.len().max(1) {
        return None;
    }
    images.card.as_ref()
}

/// The stored link for a size of a face of a printing's pictures.
pub(crate) fn link_of(images: &PrintingImages, face: usize, size: Size) -> Option<String> {
    size.link(uris_of(images, face)?).map(str::to_owned)
}

/// A type line without Scryfall's `Token` word and with the dash the cards use, in lower case.
fn plain_type(line: &str) -> String {
    let line = line.trim();
    let line = line.strip_prefix("Token ").unwrap_or(line);
    line.replace(" - ", " \u{2014} ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Whether the stored token has every fact the query gives; power and toughness are the face's.
pub(crate) fn token_matches(card: &Card, face: usize, query: &TokenQuery) -> bool {
    let stored = card.faces.get(face).or_else(|| card.faces.first());
    query
        .type_line
        .as_ref()
        .is_none_or(|line| plain_type(line) == plain_type(&card.type_line))
        && query.colors.is_none_or(|colors| colors == card.colors)
        && query
            .power
            .as_ref()
            .is_none_or(|power| stored.and_then(|f| f.power.as_ref()) == Some(power))
        && query
            .toughness
            .as_ref()
            .is_none_or(|toughness| stored.and_then(|f| f.toughness.as_ref()) == Some(toughness))
}

/// The pictures an identity stands for: the printing's, the card's newest one with pictures, or
/// the first stored token that matches the query. `None` when nothing stands for it.
pub(crate) async fn pictures_of(
    repo: &CardRepo,
    identity: &Identity,
    face: usize,
) -> Result<Option<PrintingImages>, DbError> {
    match identity {
        Identity::Oracle(id) => Ok(repo.picture_printing(id).await?.map(|found| found.images)),
        Identity::Printing(id) => Ok(repo
            .printings(std::slice::from_ref(id))
            .await?
            .into_iter()
            .next()
            .map(|printing| printing.images)),
        Identity::Token(query) => {
            let found: Vec<TokenPictures> = repo.token_pictures(&query.name).await?;
            Ok(found
                .into_iter()
                .find(|token| token_matches(&token.card, face, query))
                .map(|token| token.pictures.images))
        }
    }
}

/// The stored link of a picture, or `None` when the identity, face or size has none.
pub(crate) async fn link_for(
    repo: &CardRepo,
    identity: &Identity,
    face: usize,
    size: Size,
) -> Result<Option<String>, DbError> {
    Ok(pictures_of(repo, identity, face)
        .await?
        .and_then(|images| link_of(&images, face, size)))
}

#[cfg(test)]
mod tests {
    //! Goal: the face and size pick the right stored link for cards with one picture, with a picture per face and with none, and a token must match every fact it is asked for.
    use mtg_db::StoredFace;
    use rstest::rstest;

    use super::*;

    fn uris(tag: &str) -> ImageUris {
        ImageUris {
            small: Some(format!("s-{tag}")),
            normal: Some(format!("n-{tag}")),
            large: Some(format!("l-{tag}")),
            png: None,
            art_crop: Some(format!("a-{tag}")),
            border_crop: None,
        }
    }

    fn single() -> PrintingImages {
        PrintingImages {
            card: Some(uris("card")),
            faces: vec![None],
        }
    }

    fn split() -> PrintingImages {
        PrintingImages {
            card: Some(uris("card")),
            faces: vec![None, None],
        }
    }

    fn two_sided() -> PrintingImages {
        PrintingImages {
            card: None,
            faces: vec![Some(uris("front")), Some(uris("back"))],
        }
    }

    fn lopsided() -> PrintingImages {
        PrintingImages {
            card: None,
            faces: vec![Some(uris("front")), None],
        }
    }

    #[rstest]
    #[case::single_front(single(), 0, Size::Normal, Some("n-card"))]
    #[case::single_has_no_back(single(), 1, Size::Normal, None)]
    #[case::split_shows_both_halves_on_the_card(split(), 1, Size::Normal, Some("n-card"))]
    #[case::split_third_face(split(), 2, Size::Normal, None)]
    #[case::two_sided_front(two_sided(), 0, Size::Large, Some("l-front"))]
    #[case::two_sided_back(two_sided(), 1, Size::Small, Some("s-back"))]
    #[case::two_sided_art(two_sided(), 1, Size::ArtCrop, Some("a-back"))]
    #[case::two_sided_no_third(two_sided(), 2, Size::Normal, None)]
    #[case::a_face_without_a_picture(lopsided(), 1, Size::Normal, None)]
    #[case::a_size_without_a_link(single(), 0, Size::Png, None)]
    #[case::nothing_at_all(PrintingImages::default(), 0, Size::Normal, None)]
    fn the_face_and_size_pick_a_link(
        #[case] images: PrintingImages,
        #[case] face: usize,
        #[case] size: Size,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(link_of(&images, face, size).as_deref(), expected);
    }

    #[rstest]
    #[case("small", Some(Size::Small))]
    #[case("normal", Some(Size::Normal))]
    #[case("large", Some(Size::Large))]
    #[case("art_crop", Some(Size::ArtCrop))]
    #[case("png", Some(Size::Png))]
    #[case("border_crop", None)]
    #[case("Normal", None)]
    #[case("", None)]
    fn sizes_are_named_exactly(#[case] text: &str, #[case] size: Option<Size>) {
        assert_eq!(Size::parse(text), size);
    }

    fn token(type_line: &str, colors: &str, power: &str, toughness: &str) -> Card {
        let face = StoredFace {
            name: "Goblin".into(),
            mana_cost: String::new(),
            type_line: type_line.into(),
            oracle_text: String::new(),
            power: Some(power.into()),
            toughness: Some(toughness.into()),
            loyalty: None,
            defense: None,
            colors: colors
                .chars()
                .filter_map(mtg_core::color::Color::from_symbol)
                .collect(),
        };
        Card {
            oracle_id: OracleId("t".into()),
            name: "Goblin".into(),
            layout: mtg_import::scryfall::Layout::Token,
            is_deck_card: false,
            mana_cost: String::new(),
            mana_value: 0.0,
            type_line: type_line.into(),
            oracle_text: String::new(),
            keywords: Vec::new(),
            colors: face.colors,
            color_identity: face.colors,
            produced_mana: mtg_import::scryfall::ProducedMana::default(),
            legalities: std::collections::BTreeMap::new(),
            faces: vec![face],
            impl_status: mtg_db::ImplStatus::Manual,
        }
    }

    fn query(
        type_line: Option<&str>,
        colors: Option<&str>,
        pt: Option<(&str, &str)>,
    ) -> TokenQuery {
        TokenQuery {
            name: "Goblin".into(),
            type_line: type_line.map(str::to_owned),
            colors: colors.map(|c| {
                c.chars()
                    .filter_map(mtg_core::color::Color::from_symbol)
                    .collect()
            }),
            power: pt.map(|(p, _)| p.to_owned()),
            toughness: pt.map(|(_, t)| t.to_owned()),
        }
    }

    #[rstest]
    #[case::name_only(query(None, None, None), true)]
    #[case::everything(query(Some("Creature \u{2014} Goblin"), Some("R"), Some(("1", "1"))), true)]
    #[case::with_the_token_word(query(Some("Token Creature \u{2014} Goblin"), None, None), true)]
    #[case::case_and_spacing(query(Some("creature  \u{2014}  goblin"), None, None), true)]
    #[case::ascii_dash(query(Some("Creature - Goblin"), None, None), true)]
    #[case::other_type(query(Some("Creature \u{2014} Zombie"), None, None), false)]
    #[case::other_colors(query(None, Some("B"), None), false)]
    #[case::colorless_is_not_red(query(None, Some("C"), None), false)]
    #[case::other_power(query(None, None, Some(("2", "1"))), false)]
    #[case::other_toughness(query(None, None, Some(("1", "2"))), false)]
    fn a_token_matches_only_when_every_given_fact_does(
        #[case] asked: TokenQuery,
        #[case] fits: bool,
    ) {
        let stored = token("Token Creature \u{2014} Goblin", "R", "1", "1");
        assert_eq!(token_matches(&stored, 0, &asked), fits);
    }
}
