//! A permanent taken for the turn, worded other ways than `w6e_control`: "Untap target permanent and gain control of it
//! until end of turn. It gains haste until end of turn." and a taken creature that gets more after its untap: "Until end
//! of turn, it has base power and toughness 10/10 and gains trample, annihilator 2, and haste." or "Until end of turn, it
//! gains haste and becomes a Pirate in addition to its other types."

use super::selectors::target;
use super::tokens::Reader;
use super::w6e_control::{w6e_duration, w6e_keywords, w6e_taken};
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Ref, Targetable};
use crate::value::Amount;
use mtg_core::card::Subtype;

/// "untap target permanent and gain control of it until end of turn", then "It gains haste until end of turn".
pub(super) fn w20c_untap_and_take(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need("untap")?;
    let what = target(r)?;
    matches!(r.targets.first()?.what, Targetable::Object(_)).then_some(())?;
    r.need("and gain control of it until end of turn")?;
    let mut effects = vec![
        Effect::Untap { what: what.clone() },
        Effect::Continuous {
            affected: what.clone(),
            mods: vec![Modification::SetController(Ref::You)],
            duration: Duration::UntilEndOfTurn,
        },
    ];
    if let Some(mods) = r.attempt(|r| {
        r.punct('.').then_some(())?;
        r.need("it gains")?;
        let mods = w6e_keywords(r)?;
        r.need("until end of turn")?;
        Some(mods)
    }) {
        effects.push(Effect::Continuous {
            affected: what,
            mods,
            duration: Duration::UntilEndOfTurn,
        });
    }
    Some(effects)
}

/// "gain control of target creature until end of turn. Untap that creature." and the sentence of what it gets then.
pub(super) fn w20c_taken_and_changed(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("gain control of")?;
    let (what, noun) = w6e_taken(r)?;
    let noun = noun?;
    (w6e_duration(r)? == Duration::UntilEndOfTurn).then_some(())?;
    r.punct('.').then_some(())?;
    r.need("untap that")?;
    r.need(noun)?;
    r.punct('.').then_some(())?;
    let mods = w20c_tail(r)?;
    (!mods
        .iter()
        .all(|part| matches!(part, Modification::AddKeyword(_))))
    .then_some(())?;
    Some(vec![
        Effect::Continuous {
            affected: what.clone(),
            mods: vec![Modification::SetController(Ref::You)],
            duration: Duration::UntilEndOfTurn,
        },
        Effect::Untap { what: what.clone() },
        Effect::Continuous {
            affected: what,
            mods,
            duration: Duration::UntilEndOfTurn,
        },
    ])
}

/// "Until end of turn, it has base power and toughness 10/10 and gains haste": the parts joined by "and".
fn w20c_tail(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    r.need("until end of turn")?;
    r.punct(',').then_some(())?;
    r.need("it")?;
    let mut mods = w20c_part(r)?;
    while let Some(more) = r.attempt(|r| {
        r.need("and")?;
        w20c_part(r)
    }) {
        mods.extend(more);
    }
    Some(mods)
}

/// One part after "it": a base size, keywords it gains, or a creature type it becomes in addition to its others.
fn w20c_part(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    if r.words("has base power and toughness") {
        let power = i32::try_from(r.number()?).ok()?;
        r.punct('/').then_some(())?;
        let toughness = i32::try_from(r.number()?).ok()?;
        return Some(vec![Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        }]);
    }
    if r.word("gains") {
        return w6e_keywords(r);
    }
    r.word("becomes").then_some(())?;
    (r.word("a") || r.word("an")).then_some(())?;
    let kind = r.name()?;
    r.need("in addition to its other types")?;
    Some(vec![Modification::AddSubtype(Subtype(kind.to_owned()))])
}
