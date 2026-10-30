//! A static ability that takes every ability of a permanent: "Enchanted creature loses all abilities and can't attack
//! or block.", "... loses all abilities and can't block.", "Enchanted creature has base power and toughness 0/2, has
//! defender, and loses all other abilities." The loss comes first, then the keywords it still has, then the
//! restrictions, which are no abilities of the creature.

use super::selectors::{granted_keywords, granted_text};
use super::tokens::Reader;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::value::Amount;

/// What follows the subject: "loses all abilities [and can't ...]" or "has base power and toughness P/T, has K, and loses
/// all other abilities".
pub(super) fn form(r: &mut Reader<'_>, plural: bool) -> Option<Vec<Modification>> {
    let lose = if plural { "lose" } else { "loses" };
    if r.words(&format!("{lose} all abilities")) {
        let mut mods = vec![Modification::LoseAbilities];
        if r.attempt(|r| r.need("and")).is_some() {
            mods.extend(restrictions(r)?);
        }
        return Some(mods);
    }
    let has = if plural { "have" } else { "has" };
    r.word(has).then_some(())?;
    r.words("base power and toughness").then_some(())?;
    let power = i32::try_from(r.number()?).ok()?;
    r.punct('/').then_some(())?;
    let toughness = i32::try_from(r.number()?).ok()?;
    r.punct(',').then_some(())?;
    r.word(has).then_some(())?;
    let keywords = keyword_list(r)?;
    r.punct(',').then_some(())?;
    r.words(&format!("and {lose} all other abilities"))
        .then_some(())?;
    let mut mods = vec![
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
        Modification::LoseAbilities,
    ];
    mods.extend(keywords.into_iter().map(Modification::AddKeyword));
    Some(mods)
}

/// "can't attack or block", "can't attack", "can't block".
fn restrictions(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    r.need("can't")?;
    if r.words("attack or block") {
        Some(vec![Modification::CantAttack, Modification::CantBlock])
    } else if r.word("attack") {
        Some(vec![Modification::CantAttack])
    } else if r.word("block") {
        Some(vec![Modification::CantBlock])
    } else {
        None
    }
}

/// "defender", "flying and haste": keywords joined with "and".
fn keyword_list(r: &mut Reader<'_>) -> Option<Vec<Keyword>> {
    let mut found = granted_keywords(r)?;
    while let Some(more) = r.attempt(|r| {
        r.word("and").then_some(())?;
        granted_keywords(r)
    }) {
        found.extend(more);
    }
    Some(found)
}

/// The wording `form` reads for the changes, or `None` for any other list.
pub(super) fn text(mods: &[Modification], plural: bool) -> Option<String> {
    let lose = if plural { "lose" } else { "loses" };
    let has = if plural { "have" } else { "has" };
    match mods {
        [Modification::LoseAbilities] => Some(format!("{lose} all abilities")),
        [
            Modification::LoseAbilities,
            Modification::CantAttack,
            Modification::CantBlock,
        ] => Some(format!("{lose} all abilities and can't attack or block")),
        [Modification::LoseAbilities, Modification::CantAttack] => {
            Some(format!("{lose} all abilities and can't attack"))
        }
        [Modification::LoseAbilities, Modification::CantBlock] => {
            Some(format!("{lose} all abilities and can't block"))
        }
        [
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            },
            Modification::LoseAbilities,
            keywords @ ..,
        ] if !keywords.is_empty()
            && keywords
                .iter()
                .all(|m| matches!(m, Modification::AddKeyword(_))) =>
        {
            Some(format!(
                "{has} base power and toughness {power}/{toughness}, {has} {}, and {lose} all other abilities",
                granted_text(keywords)?
            ))
        }
        _ => None,
    }
}
