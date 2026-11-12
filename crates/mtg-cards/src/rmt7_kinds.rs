//! Kinds of keywords that one modification takes away whatever their parameter: "loses all landwalk abilities",
//! "lose ... protection ... and ward".

use serde::{Deserialize, Serialize};

use crate::keyword::Keyword;

/// A family of keyword abilities that differ only in their parameter (CR 702.14, 702.16, 702.21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeywordKind {
    /// Every landwalk ability, whatever land it names.
    Landwalk,
    /// Every protection ability, whatever it protects from, with or without an exception.
    Protection,
    /// Every ward ability, whatever its cost.
    Ward,
    /// Every "bands with other" ability.
    BandsWithOther,
}

impl KeywordKind {
    /// Whether the keyword is of this kind.
    pub fn covers(self, keyword: &Keyword) -> bool {
        matches!(
            (self, keyword),
            (KeywordKind::Landwalk, Keyword::Landwalk(_))
                | (
                    KeywordKind::Protection,
                    Keyword::Protection(_) | Keyword::ProtectionSparing { .. }
                )
                | (KeywordKind::Ward, Keyword::Ward(_))
                | (KeywordKind::BandsWithOther, Keyword::BandsWithOther(_))
        )
    }

    /// The words of the kind after "loses": "all landwalk abilities", "protection", "ward".
    pub fn lost_words(self) -> &'static str {
        match self {
            KeywordKind::Landwalk => "all landwalk abilities",
            KeywordKind::Protection => "protection",
            KeywordKind::Ward => "ward",
            KeywordKind::BandsWithOther => "all \"bands with other\" abilities",
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a keyword kind covers every parameter of its own keyword and no other keyword.
    use super::*;
    use crate::cost::Cost;
    use crate::refs::{Pred, Selector};
    use mtg_core::card::Subtype;
    use rstest::rstest;

    fn island() -> Selector {
        Selector(vec![Pred::Subtype(Subtype("Island".to_owned()))])
    }

    #[rstest]
    #[case::islandwalk(KeywordKind::Landwalk, Keyword::Landwalk(island()), true)]
    #[case::any_landwalk(KeywordKind::Landwalk, Keyword::Landwalk(Selector::default()), true)]
    #[case::flying_is_no_landwalk(KeywordKind::Landwalk, Keyword::Flying, false)]
    #[case::protection(KeywordKind::Protection, Keyword::Protection(island()), true)]
    #[case::protection_sparing(
        KeywordKind::Protection,
        Keyword::ProtectionSparing { from: island(), spares: Selector::default() },
        true
    )]
    #[case::hexproof_is_no_protection(KeywordKind::Protection, Keyword::Hexproof, false)]
    #[case::ward(KeywordKind::Ward, Keyword::Ward(Cost::default()), true)]
    #[case::shroud_is_no_ward(KeywordKind::Ward, Keyword::Shroud, false)]
    #[case::r301_bands_with_other(
        KeywordKind::BandsWithOther,
        Keyword::BandsWithOther(island()),
        true
    )]
    #[case::r301_banding_is_no_bands_with_other(
        KeywordKind::BandsWithOther,
        Keyword::Banding,
        false
    )]
    fn a_kind_covers_its_keywords(
        #[case] kind: KeywordKind,
        #[case] keyword: Keyword,
        #[case] covers: bool,
    ) {
        assert_eq!(kind.covers(&keyword), covers);
    }
}
