//! What a target or a group gets, gains, loses or has for a while: "Target creature gets +1/+1 and gains flying until
//! end of turn.", "Until your next turn, creatures target player controls have base power and toughness 1/1.", "Target
//! creature gets +3/-3 and loses all creature types until end of turn." Parts joined by "and" read into one
//! `Continuous` effect, in the order of the words, and the writer says them back in the order of the data.

use mtg_core::card::Subtype;

use super::amounts::counted_stats;
use super::becomes::{colors, colors_text};
use super::effects::{power_toughness, pump_grants, subject as plain_subject};
use super::render::stats_text;
use super::selectors::{Form, granted_keywords, granted_text, phrase};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, TargetSpec, Targetable};
use crate::value::Amount;

const END_OF_TURN: &str = "until end of turn";
const NEXT_TURN: &str = "until your next turn";

/// The words that end a change: the duration it lasts.
fn duration_words(duration: Duration) -> Option<&'static str> {
    match duration {
        Duration::UntilEndOfTurn => Some(END_OF_TURN),
        Duration::UntilYourNextTurn => Some(NEXT_TURN),
        _ => None,
    }
}

fn duration(r: &mut Reader<'_>) -> Option<Duration> {
    if r.words(END_OF_TURN) {
        Some(Duration::UntilEndOfTurn)
    } else if r.words(NEXT_TURN) {
        Some(Duration::UntilYourNextTurn)
    } else {
        None
    }
}

/// The verbs of a subject: "get", "gain", "lose", "have" for several, "gets", "gains", "loses", "has" for one.
struct Verbs {
    gets: &'static str,
    gains: &'static str,
    loses: &'static str,
    has: &'static str,
    becomes: &'static str,
}

impl Verbs {
    const fn of(plural: bool) -> Self {
        if plural {
            Self {
                gets: "get",
                gains: "gain",
                loses: "lose",
                has: "have",
                becomes: "become",
            }
        } else {
            Self {
                gets: "gets",
                gains: "gains",
                loses: "loses",
                has: "has",
                becomes: "becomes",
            }
        }
    }
}

/// "creatures target player controls", "each creature target player controls": the group of the player the spell
/// targets.
fn players_group(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    r.targets.is_empty().then_some(())?;
    let (mut selector, plural) = if r.word("each") {
        (phrase(r, Form::Each)?, false)
    } else {
        (phrase(r, Form::All)?, true)
    };
    r.words("target player controls").then_some(())?;
    r.targets.push(TargetSpec {
        what: Targetable::Player,
        bounds: Bounds::default(),
        divide: None,
    });
    selector.0.push(Pred::ControlledBy(Ref::Target(0)));
    Some((Ref::Each(selector), plural))
}

/// "up to four target permanents you control", "up to two target creatures": several targets chosen together.
fn several_targets(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    let what = super::selectors::target(r)?;
    let spec = r.targets.first()?;
    (matches!(spec.what, Targetable::Object(_)) && !spec.bounds.is_one()).then_some((what, true))
}

fn subject(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if let Some(found) = r.attempt(players_group) {
        return Some(found);
    }
    if let Some(found) = r.attempt(several_targets) {
        return Some(found);
    }
    plain_subject(r)
}

/// "all abilities" or "all creature types" after "loses", and the keywords it names.
fn loses(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    if r.words("all abilities") {
        return Some(vec![Modification::LoseAbilities]);
    }
    if r.words("all creature types") {
        return Some(vec![Modification::SetCreatureTypes(Vec::new())]);
    }
    let mut mods: Vec<Modification> = granted_keywords(r)?
        .into_iter()
        .map(Modification::RemoveKeyword)
        .collect();
    while let Some(next) = r.attempt(|r| {
        r.need("and")?;
        granted_keywords(r)
    }) {
        mods.extend(next.into_iter().map(Modification::RemoveKeyword));
    }
    Some(mods)
}

/// "base power and toughness 1/1" after "have": the size it is set to.
fn base_size(r: &mut Reader<'_>) -> Option<Modification> {
    r.need("base power and toughness")?;
    let side = |r: &mut Reader<'_>| {
        if r.x() {
            Some(Amount::X)
        } else {
            Some(Amount::Fixed(i32::try_from(r.number()?).ok()?))
        }
    };
    let power = side(r)?;
    r.punct('/').then_some(())?;
    let toughness = side(r)?;
    Some(Modification::SetPT { power, toughness })
}

/// What it becomes after "becomes": "a blue Frog with base power and toughness 1/1" (colors, creature types, a size).
fn becomes_part(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    (r.word("a") || r.word("an")).then_some(())?;
    let found = colors(r);
    let mut types = Vec::new();
    while let Some(name) = r.name() {
        types.push(Subtype(name.to_owned()));
    }
    // The data keeps the creature types before the colors, as the layers apply them.
    let mut mods = Vec::new();
    if !types.is_empty() {
        mods.push(Modification::SetCreatureTypes(types));
    }
    if let Some(found) = found {
        mods.push(Modification::SetColors(found));
    }
    (!mods.is_empty()).then_some(())?;
    if r.word("with") {
        mods.push(base_size(r)?);
    }
    Some(mods)
}

/// The words after "becomes" for colors, creature types and a size at the front of the modifications, and how many
/// modifications they used.
fn becoming_text(mods: &[Modification]) -> Option<(String, usize)> {
    let mut used = 0;
    let mut words = Vec::new();
    let mut names = Vec::new();
    if let Some(Modification::SetCreatureTypes(types)) = mods.get(used)
        && !types.is_empty()
    {
        names.extend(types.iter().map(|found| found.0.clone()));
        used += 1;
    }
    if let Some(Modification::SetColors(found)) = mods.get(used)
        && !found.is_empty()
    {
        words.push(colors_text(found));
        used += 1;
    }
    words.extend(names);
    (used > 0).then_some(())?;
    let mut text = words.join(" ");
    if let Some(Modification::SetPT { power, toughness }) = mods.get(used) {
        text = format!(
            "{text} with base power and toughness {}/{}",
            size_text(power)?,
            size_text(toughness)?
        );
        used += 1;
    }
    Some((format!("{} {text}", article(&text)), used))
}

/// One part after the subject: "gets +1/+1", "gains flying", "loses all abilities", "has base power and toughness 0/2".
fn part(r: &mut Reader<'_>, verbs: &Verbs) -> Option<Vec<Modification>> {
    if r.word(verbs.gets) {
        return power_toughness(r).map(|stats| vec![stats]);
    }
    if r.word(verbs.gains) {
        if r.words("all creature types") {
            return Some(vec![Modification::AddAllCreatureTypes]);
        }
        return pump_grants(r);
    }
    if r.word(verbs.loses) {
        return loses(r);
    }
    if r.word(verbs.has) {
        return base_size(r).map(|size| vec![size]);
    }
    if r.word(verbs.becomes) {
        return becomes_part(r);
    }
    None
}

fn parts(r: &mut Reader<'_>, verbs: &Verbs) -> Option<Vec<Modification>> {
    let mut mods = part(r, verbs)?;
    while let Some(next) = r.attempt(|r| {
        r.need("and")?;
        part(r, verbs)
    }) {
        mods.extend(next);
    }
    Some(mods)
}

/// "Target creature gets +3/+3 until end of turn.", "Until end of turn, creatures you control gain trample and get
/// +X/+X.", "Humans you control gain indestructible until your next turn."
pub(super) fn pump(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let leading = r.attempt(|r| {
        let found = duration(r)?;
        r.punct(',').then_some(found)
    });
    let (affected, plural) = subject(r)?;
    let mut mods = parts(r, &Verbs::of(plural))?;
    let duration = match leading {
        Some(found) => found,
        None => duration(r)?,
    };
    // "Target creature gets -1/-1 until end of turn for each Swamp you control."
    if let Some(stats) = mods
        .iter_mut()
        .find(|part| matches!(part, Modification::ModifyPT { .. }))
    {
        *stats = counted_stats(r, stats.clone())?;
    }
    // The one target that changed is what the next sentence's "it" means ("It fights target creature").
    if affected == Ref::Target(0) && r.targets.first().is_some_and(|spec| spec.bounds.is_one()) {
        r.it = Some(affected.clone());
    }
    Some(vec![Effect::Continuous {
        affected,
        mods,
        duration,
    }])
}

/// The size a base power and toughness is written as: a number or X.
fn size_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(number) if *number >= 0 => Some(number.to_string()),
        Amount::X => Some("X".to_owned()),
        _ => None,
    }
}

/// The parts of a change in the order of the data: "gets +1/+1", "gains flying and haste", "loses all abilities".
/// Returns the parts and the words that count the change of size ("for each ...").
fn part_texts(mods: &[Modification], plural: bool) -> Option<(Vec<String>, String)> {
    let verbs = Verbs::of(plural);
    let mut parts = Vec::new();
    let mut tail = String::new();
    let mut rest = mods;
    while let Some((first, after)) = rest.split_first() {
        if let Some((text, used)) = becoming_text(rest) {
            parts.push(format!("{} {text}", verbs.becomes));
            rest = &rest[used..];
            continue;
        }
        let same = |a: &Modification, b: &Modification| {
            matches!(
                (a, b),
                (
                    Modification::AddKeyword(_) | Modification::AddProtectionFromChosenColor,
                    Modification::AddKeyword(_) | Modification::AddProtectionFromChosenColor
                ) | (
                    Modification::RemoveKeyword(_),
                    Modification::RemoveKeyword(_)
                )
            )
        };
        let run = 1 + after.iter().take_while(|next| same(first, next)).count();
        let (group, after) = rest.split_at(run);
        rest = after;
        match first {
            Modification::ModifyPT { power, toughness } => {
                let (stats, counted) = stats_text(power, toughness)?;
                parts.push(format!("{} {stats}", verbs.gets));
                tail = counted;
            }
            Modification::AddKeyword(_) | Modification::AddProtectionFromChosenColor => {
                parts.push(format!("{} {}", verbs.gains, granted_text(group)?));
            }
            Modification::RemoveKeyword(_) => {
                let kept: Vec<Modification> = group
                    .iter()
                    .map(|gone| match gone {
                        Modification::RemoveKeyword(keyword) => {
                            Modification::AddKeyword(keyword.clone())
                        }
                        other => other.clone(),
                    })
                    .collect();
                parts.push(format!("{} {}", verbs.loses, granted_text(&kept)?));
            }
            Modification::AddAllCreatureTypes => {
                parts.push(format!("{} all creature types", verbs.gains));
            }
            Modification::LoseAbilities => parts.push(format!("{} all abilities", verbs.loses)),
            Modification::SetCreatureTypes(types) if types.is_empty() => {
                parts.push(format!("{} all creature types", verbs.loses));
            }
            Modification::SetPT { power, toughness } => parts.push(format!(
                "{} base power and toughness {}/{}",
                verbs.has,
                size_text(power)?,
                size_text(toughness)?
            )),
            _ => return None,
        }
    }
    Some((parts, tail))
}

/// The sentence of a change of a subject, without its period: "creatures you control get +1/+1 until end of turn".
pub(super) fn text(
    subject: &str,
    plural: bool,
    mods: &[Modification],
    duration: Duration,
) -> Option<String> {
    let (parts, tail) = part_texts(mods, plural)?;
    (!parts.is_empty()).then_some(())?;
    Some(format!(
        "{subject} {} {}{tail}",
        parts.join(" and "),
        duration_words(duration)?
    ))
}

/// The wordings with the duration first: "Until end of turn, creatures you control gain trample." beside the
/// duration last.
pub(super) fn wordings(texts: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for text in texts {
        for words in [END_OF_TURN, NEXT_TURN] {
            let inside = format!(" {words}");
            let Some(at) = text.find(&inside) else {
                continue;
            };
            let (before, after) = (&text[..at], &text[at + inside.len()..]);
            // The change begins after a cost, or after the event of a trigger.
            let head = before
                .rfind(": ")
                .map(|colon| colon + 2)
                .or_else(|| {
                    (before.starts_with("When") || before.starts_with("At "))
                        .then(|| before.find(", ").map(|comma| comma + 2))
                        .flatten()
                })
                .or_else(|| before.rfind(". ").map(|stop| stop + 2))
                .unwrap_or(0);
            let (lead, body) = before.split_at(head);
            let mut chars = body.chars();
            let Some(first) = chars.next() else {
                continue;
            };
            let lowered = format!("{}{}", first.to_lowercase(), chars.as_str());
            let capital = format!("{}{}", words[..1].to_uppercase(), &words[1..]);
            for body in [body.to_owned(), lowered] {
                found.push(format!("{lead}{capital}, {body}{after}"));
            }
        }
    }
    found
}
