//! Exceptions of a copy that add rather than replace: "it's a 2/2 black Zombie in addition to its other colors and
//! types" (`SetPT`, `AddColors`, `AddSubtype`), "its starting loyalty is 1" (`SetLoyalty`) and "it's not legendary".

use mtg_core::card::{Subtype, Supertype};

use super::copies::Subject;
use super::create::colors_text;
use super::selectors::{color, subtype};
use super::tokens::Reader;
use super::triggers::article;
use crate::modify::Modification;
use crate::value::Amount;

/// After "it's a" and the size: colors and creature types added to the others, "in addition to its other colors and
/// types".
pub(super) fn r371_colors_too(r: &mut Reader<'_>, size: Modification) -> Option<Vec<Modification>> {
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
        kinds.push(Modification::AddSubtype(Subtype(name)));
    }
    (!kinds.is_empty()).then_some(())?;
    r.need("in addition to its other colors and types")?;
    let mut mods = vec![size, Modification::AddColors(colors)];
    mods.extend(kinds);
    Some(mods)
}

/// The words after "it's" of a size, colors and creature types added to the others at the head of `mods`, with how many
/// changes they stand for.
pub(super) fn r371_colors_too_text(mods: &[Modification]) -> Option<(usize, String)> {
    let [
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
        Modification::AddColors(colors),
        rest @ ..,
    ] = mods
    else {
        return None;
    };
    let names: Vec<&str> = rest
        .iter()
        .map_while(|found| match found {
            Modification::AddSubtype(Subtype(name)) if found_creature_type(name) => {
                Some(name.as_str())
            }
            _ => None,
        })
        .collect();
    (*power >= 0 && *toughness >= 0 && !colors.is_empty() && !names.is_empty()).then_some(())?;
    let size = format!("{power}/{toughness}");
    Some((
        2 + names.len(),
        format!(
            "{} {size} {} {} in addition to its other colors and types",
            article(&size),
            colors_text(colors),
            names.join(" ")
        ),
    ))
}

fn found_creature_type(name: &str) -> bool {
    Subtype(name.to_owned()).is_creature_type()
}

/// A part of the exceptions with its subject: "it's not legendary", "its starting loyalty is 1".
pub(super) fn r371_part(r: &mut Reader<'_>, subject: Subject) -> Option<Modification> {
    (subject == Subject::It).then_some(())?;
    if r.words("it's not legendary") {
        return Some(Modification::RemoveSupertype(Supertype::Legendary));
    }
    r.need("its starting loyalty is")?;
    loyalty(r)
}

/// A part of a list that names its subject once: ", and its starting loyalty is 1".
pub(super) fn r371_listed(r: &mut Reader<'_>, subject: Subject) -> Option<Modification> {
    (subject == Subject::It).then_some(())?;
    r.punct(',').then_some(())?;
    r.word("and");
    r.need("its starting loyalty is")?;
    loyalty(r)
}

fn loyalty(r: &mut Reader<'_>) -> Option<Modification> {
    let value = i32::try_from(r.number()?).ok()?;
    Some(Modification::SetLoyalty(Amount::Fixed(value)))
}

/// The words of a starting loyalty set by a copy.
pub(super) fn r371_loyalty_text(modification: &Modification, subject: Subject) -> Option<String> {
    let Modification::SetLoyalty(Amount::Fixed(value)) = modification else {
        return None;
    };
    (subject == Subject::It && *value >= 0).then(|| format!("its starting loyalty is {value}"))
}

/// Every wording that says "except it isn't legendary" also as "except it's not legendary".
pub(super) fn r371_not_legendary_too(texts: &mut Vec<String>) {
    let others: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("except it isn't legendary"))
        .map(|text| text.replace("except it isn't legendary", "except it's not legendary"))
        .collect();
    for other in others {
        if !texts.contains(&other) {
            texts.push(other);
        }
    }
}

/// Keeps "a copy of that creature" in a triggered ability only for a trigger on a creature's death, where it names the
/// creature as it last existed; any other trigger words its triggering object otherwise.
pub(super) fn r371_that_creature_dies(ability: &crate::ability::Ability, texts: &mut Vec<String>) {
    use crate::refs::ZoneKind;
    use crate::trigger::TriggerPattern;
    let other_trigger = matches!(ability, crate::ability::Ability::Triggered { .. });
    let dies = matches!(
        ability,
        crate::ability::Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
                ..
            },
            ..
        }
    );
    if other_trigger && !dies {
        texts.retain(|text| !text.contains("a copy of that creature"));
    }
}
