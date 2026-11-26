//! The stickers of a sticker sheet (CR 123) paired with its lines: a "{TK}{TK} — ..." line stands for the sticker of that
//! ticket cost whose power and toughness or ability it prints; the free name and art stickers have no line.

use crate::ability::Sticker;
use crate::compile::{read_keywords, sticker_keywords, sticker_line, written_variants};

/// Whether the sticker is one the sheet prints no line for.
pub(super) fn is_unprinted(sticker: &Sticker) -> bool {
    matches!(sticker, Sticker::Name { .. } | Sticker::Art {})
}

/// Whether a printed sticker line stands for the sticker; `same` compares a written wording with the rest of the line.
pub(super) fn stands_for(sticker: &Sticker, line: &str, same: impl Fn(&str, &str) -> bool) -> bool {
    let Some((tickets, rest)) = sticker_line(line) else {
        return false;
    };
    if tickets != sticker.tickets() {
        return false;
    }
    match sticker {
        Sticker::PowerToughness {
            power, toughness, ..
        } => rest.trim() == format!("{power}/{toughness}"),
        Sticker::Ability { ability, .. } => {
            let keywords = sticker_keywords(ability);
            if keywords.is_empty() {
                written_variants(ability)
                    .iter()
                    .any(|variant| same(variant, rest))
            } else {
                read_keywords(rest).is_some_and(|read| read.iter().eq(keywords))
            }
        }
        Sticker::Name { .. } | Sticker::Art {} => false,
    }
}
