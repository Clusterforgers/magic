//! A permanent that becomes a creature until end of turn: "{1}: This land becomes a 2/2 Assembly-Worker artifact
//! creature until end of turn. It's still a land.", the duration first ("Until end of turn, this land becomes a 4/4
//! white and blue Elemental creature with flying and vigilance."), "with all creature types", "with \"Whenever this
//! creature attacks, ...\"" and "It can't be blocked this turn." read into one `Continuous` effect of the card.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

use super::abilities;
use super::effects::target_object;
use super::render::{self, capitalize};
use super::selectors::{color, color_name, granted_keywords, granted_text, spec_text, target};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;
use crate::value::Amount;

const END_OF_TURN: &str = "until end of turn";
const STILL_A_LAND: &str = "It's still a land";
const IN_ADDITION: &str = "in addition to its other types";
const ARTIFACT_TOO: &str = "an artifact in addition to its other types";

/// "~ becomes a 3/3 red Elemental creature with haste until end of turn", with the duration first or last, and the
/// sentences "It's still a land." and "It can't be blocked this turn." after it.
pub(super) fn creature(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let leading = r
        .attempt(|r| (r.words(END_OF_TURN) && r.punct(',')).then_some(()))
        .is_some();
    let affected = if r.source() {
        Ref::This
    } else {
        target_object(r)?
    };
    r.word("becomes").then_some(())?;
    let mods = description(r)?;
    let trailing = r.words(END_OF_TURN);
    (leading != trailing).then_some(())?;
    // The period of an ability in quotes is inside the quotes.
    let quoted = matches!(mods.last(), Some(Modification::GrantAbility(_)));
    let mut effects = vec![until_end_of_turn(affected.clone(), mods)];
    let still = r
        .attempt(|r| ((quoted || r.punct('.')) && r.words("it's still a land")).then_some(()))
        .is_some();
    let unblockable = r
        .attempt(|r| {
            (((quoted && !still) || r.punct('.')) && r.words("it can't be blocked this turn"))
                .then_some(())
        })
        .is_some();
    if unblockable {
        (affected == Ref::This).then_some(())?;
        effects.push(until_end_of_turn(
            affected,
            vec![Modification::CantBeBlocked],
        ));
    }
    Some(effects)
}

fn until_end_of_turn(affected: Ref, mods: Vec<Modification>) -> Effect {
    Effect::Continuous {
        affected,
        mods,
        duration: Duration::UntilEndOfTurn,
    }
}

/// What a Vehicle becomes: an artifact creature that keeps its printed power and toughness.
pub(crate) fn artifact_creature_mods() -> Vec<Modification> {
    vec![
        Modification::AddType(CardType::Artifact),
        Modification::AddType(CardType::Creature),
    ]
}

/// What a Mount becomes: saddled, the designation cards read as a keyword.
fn saddled_mods() -> Vec<Modification> {
    vec![Modification::AddKeyword(Keyword::Saddled)]
}

/// "~ becomes an artifact creature until end of turn", "another target Vehicle you control becomes an artifact
/// creature until end of turn" and "target Mount you control becomes saddled until end of turn": a Vehicle animated
/// with the size it prints, or a Mount saddled; the subject is the card itself or a target.
pub(super) fn animate(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let affected = if r.source() { Ref::This } else { target(r)? };
    r.word("becomes").then_some(())?;
    let mods = if r.words("an artifact creature") {
        artifact_creature_mods()
    } else if r.word("saddled") {
        saddled_mods()
    } else if r.words(ARTIFACT_TOO) {
        vec![Modification::AddType(CardType::Artifact)]
    } else {
        return None;
    };
    r.words(END_OF_TURN).then_some(())?;
    Some(vec![Effect::Continuous {
        affected,
        mods,
        duration: Duration::UntilEndOfTurn,
    }])
}

/// The sentence of a `Continuous` effect that makes its subject an artifact creature or saddles it, with the subject
/// written by the caller ("~", or the target in full).
pub(super) fn animate_text(subject: &str, mods: &[Modification]) -> Option<String> {
    let what = if mods == artifact_creature_mods().as_slice() {
        "an artifact creature"
    } else if mods == saddled_mods().as_slice() {
        "saddled"
    } else if mods == [Modification::AddType(CardType::Artifact)] {
        ARTIFACT_TOO
    } else {
        return None;
    };
    Some(format!("{subject} becomes {what} until end of turn"))
}

/// What the card becomes: a size, colors, creature types, "artifact", "creature" and what it has.
fn description(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    (r.word("a") || r.word("an")).then_some(())?;
    let power = side(r)?;
    r.punct('/').then_some(())?;
    let toughness = side(r)?;
    let mut mods = vec![Modification::SetPT { power, toughness }];
    if let Some(colors) = colors(r) {
        mods.push(Modification::SetColors(colors));
    }
    // Between the colors and "creature" a capitalized word is a creature type, also one no type line has ("Blinkmoth").
    let mut types = Vec::new();
    while let Some(name) = r.name() {
        types.push(Subtype(name.to_owned()));
    }
    if !types.is_empty() {
        mods.push(Modification::SetCreatureTypes(types));
    }
    if r.word("artifact") {
        mods.push(Modification::AddType(CardType::Artifact));
    }
    r.word("creature").then_some(())?;
    mods.push(Modification::AddType(CardType::Creature));
    if r.word("with") {
        mods.extend(given(r)?);
    }
    if r.words(IN_ADDITION) {
        mods = mods
            .into_iter()
            .flat_map(|modification| match modification {
                Modification::SetCreatureTypes(types) => {
                    types.into_iter().map(Modification::AddSubtype).collect()
                }
                other => vec![other],
            })
            .collect();
    }
    Some(mods)
}

/// A side of the size: a number, or X that the card names after the sentence.
fn side(r: &mut Reader<'_>) -> Option<Amount> {
    if r.x() {
        return Some(Amount::X);
    }
    Some(Amount::Fixed(i32::try_from(r.number()?).ok()?))
}

/// "red", "white and blue", "white, blue, and red".
pub(super) fn colors(r: &mut Reader<'_>) -> Option<Vec<Color>> {
    let first = r.peek_word().and_then(color)?;
    r.skip();
    let mut found = vec![first];
    while let Some(next) = r.attempt(|r| {
        if r.punct(',') {
            r.word("and");
        } else {
            r.need("and")?;
        }
        let next = r.peek_word().and_then(color)?;
        r.skip();
        Some(next)
    }) {
        found.push(next);
    }
    Some(found)
}

/// What the creature has after "with", joined by commas and "and": keywords, then "all creature types", then one
/// ability in quotes.
fn given(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let mut mods = item(r)?;
    while let Some(next) = r.attempt(|r| {
        if r.punct(',') {
            r.word("and");
        } else {
            r.need("and")?;
        }
        item(r)
    }) {
        mods.extend(next);
    }
    mods.is_sorted_by_key(rank).then_some(mods)
}

fn item(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    if r.words("all creature types") {
        return Some(vec![Modification::AddAllCreatureTypes]);
    }
    if let Some(ability) = r.quoted(quoted) {
        return Some(vec![Modification::GrantAbility(Arc::new(ability))]);
    }
    let keywords = granted_keywords(r)?;
    Some(keywords.into_iter().map(Modification::AddKeyword).collect())
}

/// A triggered or an activated ability in quotes; a mana ability stays with the table.
fn quoted(r: &mut Reader<'_>) -> Option<Ability> {
    r.attempt(abilities::triggered).or_else(|| {
        r.attempt(abilities::activated)
            .filter(|ability| !ability.is_mana_ability())
    })
}

/// The order of what a creature has in the data and in the words: keywords, all creature types, an ability.
fn rank(modification: &Modification) -> u8 {
    match modification {
        Modification::AddKeyword(_) => 0,
        Modification::AddAllCreatureTypes => 1,
        _ => 2,
    }
}

/// The words of colors: "red", "white and blue", "white, blue, and red".
pub(super) fn colors_text(colors: &[Color]) -> String {
    let names: Vec<&str> = colors.iter().map(|found| color_name(*found)).collect();
    list(&names)
}

fn list<S: AsRef<str>>(items: &[S]) -> String {
    let items: Vec<&str> = items.iter().map(AsRef::as_ref).collect();
    match items.as_slice() {
        [rest @ .., last] if rest.len() >= 2 => format!("{}, and {last}", rest.join(", ")),
        _ => items.join(" and "),
    }
}

/// The words after "with": keywords, "all creature types" and an ability in quotes.
fn given_text(mods: &[Modification]) -> Option<String> {
    mods.is_sorted_by_key(rank).then_some(())?;
    let items = mods
        .iter()
        .map(|modification| match modification {
            Modification::AddAllCreatureTypes => Some("all creature types".to_owned()),
            Modification::GrantAbility(ability) => {
                Some(format!("\"{}\"", render::ability(ability)?))
            }
            keyword => granted_text(std::slice::from_ref(keyword)),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(list(&items))
}

/// The sentence of a `Continuous` effect of the card that makes it a creature, without its period: the duration last,
/// except after an ability in quotes, which the period closes.
pub(super) fn text(affected: &Ref, mods: &[Modification]) -> Option<String> {
    (*affected == Ref::This).then_some(())?;
    text_for("~", mods)
}

/// A side of the size in words: a number or X.
fn side_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(number) if *number >= 0 => Some(number.to_string()),
        Amount::X => Some("X".to_owned()),
        _ => None,
    }
}

/// The sentence of a `Continuous` effect that makes its subject (written by the caller) a creature, without its
/// period.
pub(super) fn text_for(subject: &str, mods: &[Modification]) -> Option<String> {
    let Some((Modification::SetPT { power, toughness }, rest)) = mods.split_first() else {
        return None;
    };
    let size = format!("{}/{}", side_text(power)?, side_text(toughness)?);
    let (colors, rest) = match rest.split_first() {
        Some((Modification::SetColors(colors), after)) if !colors.is_empty() => {
            (format!("{} ", colors_text(colors)), after)
        }
        _ => (String::new(), rest),
    };
    let (types, rest) = match rest.split_first() {
        Some((Modification::SetCreatureTypes(types), after)) if !types.is_empty() => {
            let names: Vec<&str> = types.iter().map(|found| found.0.as_str()).collect();
            (format!("{} ", names.join(" ")), after)
        }
        _ => (String::new(), rest),
    };
    // "in addition to its other types": the creature types are added instead of replacing the others.
    let added = rest
        .iter()
        .take_while(|found| matches!(found, Modification::AddSubtype(_)))
        .count();
    let (types, additive, rest) = if added > 0 && types.is_empty() {
        let names: Vec<&str> = rest[..added]
            .iter()
            .filter_map(|found| match found {
                Modification::AddSubtype(subtype) => Some(subtype.0.as_str()),
                _ => None,
            })
            .collect();
        (format!("{} ", names.join(" ")), true, &rest[added..])
    } else {
        (types, false, rest)
    };
    let (artifact, rest) = match rest.split_first() {
        Some((Modification::AddType(CardType::Artifact), after)) => ("artifact ", after),
        _ => ("", rest),
    };
    let [Modification::AddType(CardType::Creature), given @ ..] = rest else {
        return None;
    };
    let with = match given {
        [] => String::new(),
        _ => format!(" with {}", given_text(given)?),
    };
    let addition = if additive {
        format!(" {IN_ADDITION}")
    } else {
        String::new()
    };
    let tail = if with.ends_with('"') {
        ""
    } else {
        " until end of turn"
    };
    Some(format!(
        "{subject} becomes {} {size} {colors}{types}{artifact}creature{with}{addition}{tail}",
        article(&size)
    ))
}

/// The effect's sentence when it is one of a card becoming a creature.
fn becoming(effect: &Effect, ability: &Ability) -> Option<String> {
    let Effect::Continuous {
        affected,
        mods,
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    match affected {
        Ref::This => text(affected, mods),
        Ref::Target(0) => {
            let spec = ability
                .targets()
                .first()
                .filter(|spec| spec.bounds.is_one())?;
            text_for(&spec_text(spec)?, mods)
        }
        _ => None,
    }
}

/// The sentence with "Until end of turn," first and the duration last dropped.
fn leading(sentence: &str) -> String {
    let body = sentence
        .strip_suffix(" until end of turn")
        .unwrap_or(sentence);
    format!("Until end of turn, {body}")
}

/// The sentence followed by "It's still a land": the period of a quote is inside it already.
fn still_a_land(sentence: &str) -> String {
    if sentence.ends_with('"') {
        format!("{sentence} {STILL_A_LAND}")
    } else {
        format!("{sentence}. {STILL_A_LAND}")
    }
}

/// The wordings of a card that becomes a creature beyond the one the renderer writes first: with or without "It's
/// still a land." after it, the duration first, and "It can't be blocked this turn." for the card.
pub(super) fn wordings(ability: &Ability, texts: &[String]) -> Vec<String> {
    let sentences: Vec<String> = ability
        .effects()
        .iter()
        .filter_map(|effect| becoming(effect, ability))
        .collect();
    let mut found = Vec::new();
    for sentence in sentences
        .iter()
        .flat_map(|sentence| [sentence.clone(), capitalize(sentence)])
    {
        let sentence = &sentence;
        let closed = format!("{sentence}.");
        let forms = [
            still_a_land(sentence),
            leading(sentence),
            still_a_land(&leading(sentence)),
        ];
        for text in texts.iter().filter(|text| text.contains(&closed)) {
            for form in &forms {
                found.push(text.replacen(&closed, &format!("{form}."), 1).replace(
                    &format!("{STILL_A_LAND}. ~ can't be blocked this turn"),
                    &format!("{STILL_A_LAND}. It can't be blocked this turn"),
                ));
            }
        }
    }
    let all: Vec<String> = texts.iter().chain(&found).cloned().collect();
    found.extend(quoted_wordings(ability, &all));
    found
}

/// The wordings of the abilities in quotes ("put a +1/+1 counter on it") in each text that has the first one.
fn quoted_wordings(ability: &Ability, texts: &[String]) -> Vec<String> {
    let granted = ability.effects().iter().flat_map(|effect| match effect {
        Effect::Continuous { mods, .. } => mods.as_slice(),
        _ => &[],
    });
    let mut found = Vec::new();
    for modification in granted {
        let Modification::GrantAbility(given) = modification else {
            continue;
        };
        let variants = render::variants(given);
        let Some((first, others)) = variants.split_first() else {
            continue;
        };
        let first = format!("\"{first}\"");
        for text in texts.iter().filter(|text| text.contains(&first)) {
            for other in others {
                found.push(text.replace(&first, &format!("\"{other}\"")));
            }
        }
    }
    found
}
