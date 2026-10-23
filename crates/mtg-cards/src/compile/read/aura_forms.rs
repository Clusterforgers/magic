//! What an Aura makes of the creature it enchants: "Enchanted creature is a Treefolk with base power and toughness 0/4
//! and loses all abilities.", "... loses all abilities and is a green Elk creature with base power and toughness 3/3.",
//! "... is an Insect artifact creature with base power and toughness 0/1 and has indestructible, and it loses all other
//! abilities, card types, and creature types.", "... is a black Dragon with base power and toughness 4/2, has flying and
//! deathtouch, and is goaded." One list of changes in the order the layers need them: colors, card types, creature
//! types, size, the loss of abilities, then the keywords it has, then "is goaded".

use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

use super::becomes::{colors, colors_text};
use super::selectors::{granted_keywords, granted_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::modify::Modification;
use crate::value::Amount;

const OTHER_LOSSES: &str = "and it loses all other abilities, card types, and creature types";

/// The changes after "Enchanted creature": "is a ..." or "loses all abilities and is a ...".
pub(super) fn form(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let lose_first = r.words("loses all abilities and");
    r.word("is").then_some(())?;
    (r.word("a") || r.word("an")).then_some(())?;
    let paint = colors(r);
    let mut kinds = Vec::new();
    while let Some(name) = r.name() {
        kinds.push(Subtype(name.to_owned()));
    }
    let artifact = r.word("artifact");
    let creature = r.word("creature");
    (!kinds.is_empty() && (creature || !artifact)).then_some(())?;
    r.words("with base power and toughness").then_some(())?;
    let power = i32::try_from(r.number()?).ok()?;
    r.punct('/').then_some(())?;
    let toughness = i32::try_from(r.number()?).ok()?;
    let mut mods = Vec::new();
    mods.extend(paint.map(Modification::SetColors));
    if creature {
        let mut types = Vec::new();
        types.extend(artifact.then_some(CardType::Artifact));
        types.push(CardType::Creature);
        mods.push(Modification::SetTypes(types));
    }
    mods.push(Modification::SetCreatureTypes(kinds));
    mods.push(Modification::SetPT {
        power: Amount::Fixed(power),
        toughness: Amount::Fixed(toughness),
    });
    let mut lose = lose_first;
    let mut goaded = false;
    let mut keywords = Vec::new();
    if r.words("and loses all abilities") {
        lose = true;
    } else if r
        .attempt(|r| {
            if !r.punct(',') {
                r.need("and")?;
            }
            r.word("has").then_some(())
        })
        .is_some()
    {
        keywords = keyword_list(r)?;
        if r.punct(',') {
            if r.words("and is goaded") {
                goaded = true;
            } else if creature && other_losses(r) {
                lose = true;
            }
        }
    }
    if lose {
        mods.push(Modification::LoseAbilities);
    }
    mods.extend(keywords.into_iter().map(Modification::AddKeyword));
    if goaded {
        mods.push(Modification::Goaded);
    }
    Some(mods)
}

/// The words of `OTHER_LOSSES`, whose commas are tokens of their own.
fn other_losses(r: &mut Reader<'_>) -> bool {
    r.attempt(|r| {
        r.words("and it loses all other abilities").then_some(())?;
        r.punct(',').then_some(())?;
        r.words("card types").then_some(())?;
        r.punct(',').then_some(())?;
        r.words("and creature types").then_some(())
    })
    .is_some()
}

/// "flying and deathtouch": keywords joined with "and".
fn keyword_list(r: &mut Reader<'_>) -> Option<Vec<crate::keyword::Keyword>> {
    let mut found = granted_keywords(r)?;
    while let Some(more) = r.attempt(|r| {
        r.word("and").then_some(())?;
        granted_keywords(r)
    }) {
        found.extend(more);
    }
    Some(found)
}

/// Every wording of the changes `form` reads; empty for any other list.
pub(super) fn texts(mods: &[Modification]) -> Vec<String> {
    let mut rest = mods;
    let mut paint: Option<&[Color]> = None;
    if let Some((Modification::SetColors(colors), after)) = rest.split_first() {
        paint = Some(colors);
        rest = after;
    }
    let mut types: Option<&[CardType]> = None;
    if let Some((Modification::SetTypes(set), after)) = rest.split_first() {
        types = Some(set);
        rest = after;
    }
    let [
        Modification::SetCreatureTypes(kinds),
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
        tail @ ..,
    ] = rest
    else {
        return Vec::new();
    };
    let Some(noun) = noun(paint, kinds, types) else {
        return Vec::new();
    };
    let (lose, tail) = match tail.split_first() {
        Some((Modification::LoseAbilities, after)) => (true, after),
        _ => (false, tail),
    };
    let (goaded, keywords) = match tail.split_last() {
        Some((Modification::Goaded, before)) => (true, before),
        _ => (false, tail),
    };
    let given = if keywords.is_empty() {
        None
    } else {
        match granted_text(keywords) {
            Some(text)
                if keywords
                    .iter()
                    .all(|m| matches!(m, Modification::AddKeyword(_))) =>
            {
                Some(text)
            }
            _ => return Vec::new(),
        }
    };
    let head = format!(
        "is {} {noun} with base power and toughness {power}/{toughness}",
        article(&noun)
    );
    let tail = match (lose, given, goaded) {
        (false, None, false) => String::new(),
        (true, None, false) => " and loses all abilities".to_owned(),
        (true, Some(given), false) if types.is_some() => {
            format!(" and has {given}, {OTHER_LOSSES}")
        }
        (false, Some(given), true) => format!(", has {given}, and is goaded"),
        _ => return Vec::new(),
    };
    let mut written = vec![format!("{head}{tail}")];
    if lose && tail == " and loses all abilities" {
        written.push(format!("loses all abilities and {head}"));
    }
    written
}

/// "green Elk creature", "Insect artifact creature", "black Dragon", "Treefolk".
fn noun(paint: Option<&[Color]>, kinds: &[Subtype], types: Option<&[CardType]>) -> Option<String> {
    let mut words = Vec::new();
    if let Some(paint) = paint.filter(|colors| !colors.is_empty()) {
        words.push(colors_text(paint));
    }
    (!kinds.is_empty()).then_some(())?;
    words.extend(kinds.iter().map(|kind| kind.0.clone()));
    match types {
        None => {}
        Some([CardType::Creature]) => words.push("creature".to_owned()),
        Some([CardType::Artifact, CardType::Creature]) => {
            words.push("artifact creature".to_owned());
        }
        Some(_) => return None,
    }
    Some(words.join(" "))
}
