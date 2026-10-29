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

/// A token as the table knows it; the stored token that shares the most of it stands for it.
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

/// Which facts of a query a stored token has; a fact the query leaves out counts as held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fit {
    type_line: bool,
    colors: bool,
    /// Power and toughness of the face, both.
    stats: bool,
}

impl Fit {
    fn of(card: &Card, face: usize, query: &TokenQuery) -> Fit {
        let stored = card.faces.get(face).or_else(|| card.faces.first());
        let stat = |asked: &Option<String>, held: Option<&String>| {
            asked.as_ref().is_none_or(|asked| held == Some(asked))
        };
        Fit {
            type_line: query
                .type_line
                .as_ref()
                .is_none_or(|line| plain_type(line) == plain_type(&card.type_line)),
            colors: query.colors.is_none_or(|colors| colors == card.colors),
            stats: stat(&query.power, stored.and_then(|f| f.power.as_ref()))
                && stat(&query.toughness, stored.and_then(|f| f.toughness.as_ref())),
        }
    }

    /// The order of preference, best first: every fact, then all but power and toughness (which a
    /// buff or a counter moves), then power and toughness, then the type line, then the colors.
    fn rank(self) -> (bool, bool, bool, bool, bool) {
        let shape = self.type_line && self.colors;
        (
            shape && self.stats,
            shape,
            self.stats,
            self.type_line,
            self.colors,
        )
    }
}

/// The stored token that stands for a query, so a token whose power, toughness, type or colors an
/// effect changed keeps its picture. In order: every fact given; all but power and toughness;
/// the name alone, where the tokens that share the most facts (power and toughness first) come
/// first. Ties go to the smallest oracle id, so the same query always gets the same token.
pub(crate) fn pick_token(
    found: Vec<TokenPictures>,
    face: usize,
    query: &TokenQuery,
) -> Option<TokenPictures> {
    let rank = |token: &TokenPictures| Fit::of(&token.card, face, query).rank();
    found.into_iter().min_by(|a, b| {
        rank(b)
            .cmp(&rank(a))
            .then_with(|| a.card.oracle_id.as_str().cmp(b.card.oracle_id.as_str()))
    })
}

/// The pictures an identity stands for: the printing's, the card's newest one with pictures, or
/// the stored token that best fits the query. `None` when nothing stands for it.
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
            let found = repo.token_pictures(&query.name).await?;
            Ok(pick_token(found, face, query).map(|token| token.pictures.images))
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
    //! Goal: the face and size pick the right stored link, and the token that shares the most facts with a query stands for it in a fixed order.
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

    use mtg_db::PrintingPictures;

    fn colors_of(letters: &str) -> ColorSet {
        letters
            .chars()
            .filter_map(mtg_core::color::Color::from_symbol)
            .collect()
    }

    /// A stored token with oracle id `id`; its printing shows the single picture.
    fn token(
        id: &str,
        name: &str,
        type_line: &str,
        colors: &str,
        pt: (&str, &str),
    ) -> TokenPictures {
        let face = StoredFace {
            name: name.into(),
            mana_cost: String::new(),
            type_line: type_line.into(),
            oracle_text: String::new(),
            power: Some(pt.0.into()),
            toughness: Some(pt.1.into()),
            loyalty: None,
            defense: None,
            colors: colors_of(colors),
        };
        let card = Card {
            oracle_id: OracleId(id.into()),
            name: name.into(),
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
        };
        TokenPictures {
            pictures: PrintingPictures {
                printing_id: PrintingId(format!("printing-{id}")),
                images: single(),
            },
            card,
        }
    }

    fn query(
        name: &str,
        type_line: Option<&str>,
        colors: Option<&str>,
        pt: Option<(&str, &str)>,
    ) -> TokenQuery {
        TokenQuery {
            name: name.into(),
            type_line: type_line.map(str::to_owned),
            colors: colors.map(colors_of),
            power: pt.map(|(p, _)| p.to_owned()),
            toughness: pt.map(|(_, t)| t.to_owned()),
        }
    }

    fn goblin(
        type_line: Option<&str>,
        colors: Option<&str>,
        pt: Option<(&str, &str)>,
    ) -> TokenQuery {
        query("Goblin", type_line, colors, pt)
    }

    #[rstest]
    #[case::name_only(goblin(None, None, None), true)]
    #[case::everything(goblin(Some("Creature \u{2014} Goblin"), Some("R"), Some(("1", "1"))), true)]
    #[case::with_the_token_word(goblin(Some("Token Creature \u{2014} Goblin"), None, None), true)]
    #[case::case_and_spacing(goblin(Some("creature  \u{2014}  goblin"), None, None), true)]
    #[case::ascii_dash(goblin(Some("Creature - Goblin"), None, None), true)]
    #[case::other_type(goblin(Some("Creature \u{2014} Zombie"), None, None), false)]
    #[case::other_colors(goblin(None, Some("B"), None), false)]
    #[case::colorless_is_not_red(goblin(None, Some("C"), None), false)]
    #[case::other_power(goblin(None, None, Some(("2", "1"))), false)]
    #[case::other_toughness(goblin(None, None, Some(("1", "2"))), false)]
    fn a_token_fits_exactly_when_every_given_fact_does(
        #[case] asked: TokenQuery,
        #[case] fits: bool,
    ) {
        let stored = token(
            "g",
            "Goblin",
            "Token Creature \u{2014} Goblin",
            "R",
            ("1", "1"),
        );
        let fit = Fit::of(&stored.card, 0, &asked);
        assert_eq!(fit.type_line && fit.colors && fit.stats, fits);
    }

    const THOPTER: &str = "Token Artifact Creature \u{2014} Thopter";
    const ARTIFACT: Option<&str> = Some("Artifact Creature \u{2014} Thopter");

    /// Four Thopter records: `t0` and `t1` alike, a bigger `t3` and a white `w1`.
    fn thopters() -> Vec<TokenPictures> {
        vec![
            token("t3", "Thopter", THOPTER, "C", ("3", "3")),
            token("t1", "Thopter", THOPTER, "C", ("1", "1")),
            token("w1", "Thopter", THOPTER, "W", ("1", "1")),
            token("t0", "Thopter", THOPTER, "C", ("1", "1")),
        ]
    }

    fn thopter(
        type_line: Option<&str>,
        colors: Option<&str>,
        pt: Option<(&str, &str)>,
    ) -> TokenQuery {
        query("Thopter", type_line, colors, pt)
    }

    #[rstest]
    #[case::exact_stats(thopter(ARTIFACT, Some("C"), Some(("3", "3"))), Some("t3"))]
    #[case::exact_stats_tie_goes_to_the_smallest_id(thopter(ARTIFACT, Some("C"), Some(("1", "1"))), Some("t0"))]
    #[case::exact_in_another_color(thopter(ARTIFACT, Some("W"), Some(("1", "1"))), Some("w1"))]
    #[case::buffed_falls_back_to_type_and_colors(thopter(ARTIFACT, Some("C"), Some(("2", "2"))), Some("t0"))]
    #[case::buffed_white(thopter(ARTIFACT, Some("W"), Some(("2", "2"))), Some("w1"))]
    #[case::unknown_colors_keep_the_stats(thopter(ARTIFACT, Some("G"), Some(("3", "3"))), Some("t3"))]
    #[case::unknown_colors_and_stats(thopter(ARTIFACT, Some("G"), Some(("9", "9"))), Some("t0"))]
    #[case::unknown_type_keeps_the_colors(thopter(Some("Creature"), Some("W"), Some(("9", "9"))), Some("w1"))]
    #[case::name_only(thopter(None, None, None), Some("t0"))]
    #[case::name_and_stats(thopter(None, None, Some(("3", "3"))), Some("t3"))]
    fn the_token_that_shares_the_most_facts_stands_for_the_query(
        #[case] asked: TokenQuery,
        #[case] expected: Option<&str>,
    ) {
        let pick = |found: Vec<TokenPictures>| {
            pick_token(found, 0, &asked).map(|token| token.card.oracle_id.0)
        };
        let expected = expected.map(str::to_owned);
        assert_eq!(pick(thopters()), expected);
        // The order the database returns them in does not matter.
        let mut reversed = thopters();
        reversed.reverse();
        assert_eq!(pick(reversed), expected);
    }

    #[test]
    fn no_token_of_that_name_has_no_picture() {
        assert!(pick_token(Vec::new(), 0, &thopter(None, None, None)).is_none());
    }
}
