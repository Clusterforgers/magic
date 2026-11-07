//! "except it's a 4/4 black Zombie": an exception of a copy that sets the size, the colors and the creature types
//! (`SetPT`, `SetColors`, `SetCreatureTypes`), the creature types replacing the others (CR 205.1b, 707.9b).

use mtg_core::card::Subtype;

use super::create::colors_text;
use super::selectors::{color, subtype};
use super::tokens::Reader;
use super::triggers::article;
use crate::modify::Modification;
use crate::value::Amount;

/// After "it's a" and the size: the colors and then the creature types, not followed by "in addition to its other
/// types" (those add to the creature types).
pub(super) fn g12b_kinds(r: &mut Reader<'_>, size: Modification) -> Option<Vec<Modification>> {
    let mut colors = vec![r.peek_word().and_then(color)?];
    r.skip();
    while let Some(more) = r.attempt(|r| {
        r.word("and").then_some(())?;
        let found = r.peek_word().and_then(color)?;
        r.skip();
        Some(found)
    }) {
        colors.push(more);
    }
    let mut kinds = Vec::new();
    while let Some(name) =
        r.attempt(|r| subtype(r).filter(|name| Subtype(name.clone()).is_creature_type()))
    {
        kinds.push(Subtype(name));
    }
    (!kinds.is_empty()).then_some(())?;
    r.lookahead(|r| r.need("in addition"))
        .is_none()
        .then_some(())?;
    Some(vec![
        size,
        Modification::SetColors(colors),
        Modification::SetCreatureTypes(kinds),
    ])
}

/// The words of a size, colors and creature types at the head of `mods`, after the subject's "it's", with how many
/// changes they stand for.
pub(super) fn g12b_kinds_text(mods: &[Modification]) -> Option<(usize, String)> {
    let [
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
        Modification::SetColors(colors),
        Modification::SetCreatureTypes(kinds),
        ..,
    ] = mods
    else {
        return None;
    };
    (*power >= 0 && *toughness >= 0 && !colors.is_empty() && !kinds.is_empty()).then_some(())?;
    let names: Vec<&str> = kinds.iter().map(|kind| kind.0.as_str()).collect();
    let size = format!("{power}/{toughness}");
    Some((
        3,
        format!(
            "{} {size} {} {}",
            article(&size),
            colors_text(colors),
            names.join(" ")
        ),
    ))
}
