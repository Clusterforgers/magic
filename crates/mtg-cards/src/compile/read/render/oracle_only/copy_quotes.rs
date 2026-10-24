//! An ability a token copy is given: the oracle text calls the token "this token" and "it" where the renderers write the
//! card as "~": "create X tokens that are copies of target creature, except they have "When this token enters, it
//! fights up to one target creature you don't control."".

use crate::ability::Ability;

/// The granted abilities as the renderers write them and as the oracle text words them, one per line.
const QUOTED: [(&str, &str); 1] = [(
    "except they have \"When ~ enters, ~ ",
    "except they have \"When this token enters, it ",
)];

/// Adds the wordings that name the token in a quoted ability, for the copies of a creature or a permanent.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    for (said, wanted) in QUOTED {
        let found: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(" copies of ") && text.contains(said))
            .map(|text| text.replacen(said, wanted, 1))
            .collect();
        texts.extend(found);
    }
}
