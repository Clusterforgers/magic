//! Changes of control (CR 108.4, 701.12): "gain control of target creature until end of turn", of the enchanted
//! permanent or of all of a group, for good or for as long as the source stays, with the untap and the keywords that
//! follow ("Untap that creature. It gains haste until end of turn."), and "exchange control of two target creatures" or
//! of "target land you control and target land an opponent controls".

use super::effects::target_object;
use super::selectors::{all, granted_keywords, second_target, target};
use super::tokens::Reader;
use super::triggers::pattern_text;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Targetable};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// The nouns "enchanted ..." names the host of an Aura by, and that "untap that ..." names it by again.
const W6E_HOSTS: [&str; 3] = ["creature", "permanent", "artifact"];

/// "gain control of <what>[ <duration>]", then "untap that creature" (or those creatures) and the keywords it gains
/// until end of turn when the control lasts the turn.
pub(super) fn w6e_gain_control(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("gain control of")?;
    let (what, noun) = w6e_taken(r)?;
    let duration = w6e_duration(r)?;
    let mut effects = vec![Effect::Continuous {
        affected: what.clone(),
        mods: vec![Modification::SetController(Ref::You)],
        duration: duration.clone(),
    }];
    if let Some(noun) = noun.filter(|_| duration == Duration::UntilEndOfTurn)
        && let Some(steps) = r.attempt(|r| {
            r.punct('.').then_some(())?;
            w6e_untapped(r, &what, noun)
        })
    {
        effects.extend(steps);
    }
    Some(effects)
}

/// What is taken, and the noun "that ..." names it by again: a target object, the enchanted permanent, or all of a group.
pub(super) fn w6e_taken(r: &mut Reader<'_>) -> Option<(Ref, Option<&'static str>)> {
    if let Some(group) = all(r) {
        return Some((group, None));
    }
    if r.ability && r.word("enchanted") {
        let noun = W6E_HOSTS.into_iter().find(|noun| r.word(noun))?;
        return Some((Ref::Attached, Some(noun)));
    }
    let what = r.attempt(|r| {
        let what = target(r)?;
        matches!(r.targets.first()?.what, Targetable::Object(_)).then_some(what)
    })?;
    let Targetable::Object(selector) = &r.targets.first()?.what else {
        return None;
    };
    let noun = if selector.0.contains(&Pred::Type(CardType::Creature)) {
        Some("creature")
    } else {
        selector.0.is_empty().then_some("permanent")
    };
    Some((what, noun))
}

/// How long the control lasts: the turn, while the source stays or is controlled, or for good.
pub(super) fn w6e_duration(r: &mut Reader<'_>) -> Option<Duration> {
    if r.words("until end of turn") {
        return Some(Duration::UntilEndOfTurn);
    }
    if r.words("for as long as you control") {
        return r.source().then_some(Duration::WhileYouControlSource);
    }
    if r.words("for as long as") {
        r.source().then_some(())?;
        r.need("remains on the battlefield")?;
        return Some(Duration::WhileSourceOnBattlefield);
    }
    Some(Duration::Forever)
}

/// ". Untap that creature. It gains haste until end of turn" (or "those creatures" that "gain" it, a target of each
/// opponent): the untap and the keywords of the taken permanents.
fn w6e_untapped(r: &mut Reader<'_>, what: &Ref, noun: &str) -> Option<Vec<Effect>> {
    r.need("untap")?;
    let plural = r.word("those");
    if plural {
        r.need(&format!("{noun}s"))?;
    } else {
        r.need("that")?;
        r.need(noun)?;
    }
    r.punct('.').then_some(())?;
    r.need(if plural { "they gain" } else { "it gains" })?;
    let mods = w6e_keywords(r)?;
    r.need("until end of turn")?;
    Some(vec![
        Effect::Untap { what: what.clone() },
        Effect::Continuous {
            affected: what.clone(),
            mods,
            duration: Duration::UntilEndOfTurn,
        },
    ])
}

/// "haste", "trample and haste", "trample, haste, and annihilator 1".
pub(super) fn w6e_keywords(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let mut mods: Vec<Modification> = granted_keywords(r)?
        .into_iter()
        .map(Modification::AddKeyword)
        .collect();
    while let Some(more) = r.attempt(|r| {
        let comma = r.punct(',');
        let and = r.word("and");
        (comma || and).then_some(())?;
        granted_keywords(r)
    }) {
        mods.extend(more.into_iter().map(Modification::AddKeyword));
    }
    Some(mods)
}

/// "exchange control of two target creatures" and "exchange control of target land you control and target land an
/// opponent controls", for good.
pub(super) fn w6e_exchange(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need("exchange control of")?;
    let what = if let Some(pair) = r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        let two = spec.bounds.min == crate::value::Amount::Fixed(2)
            && spec.bounds.max == Some(crate::value::Amount::Fixed(2));
        (two && matches!(spec.what, Targetable::Object(_))).then_some(vec![what])
    }) {
        pair
    } else {
        let first = target_object(r)?;
        r.need("and")?;
        let second = second_target(r)?;
        let spec = r.targets.get(1)?;
        (spec.bounds.is_one() && matches!(spec.what, Targetable::Object(_))).then_some(())?;
        vec![first, second]
    };
    Some(vec![Effect::ExchangeControl {
        what,
        duration: Duration::Forever,
    }])
}

/// "When ~ enters and at the beginning of your upkeep": an event and the beginning of a step in one trigger, which the
/// reader joins with "and".
pub(super) fn w6e_joined_step(patterns: &[TriggerPattern]) -> Option<String> {
    let [first, second] = patterns else {
        return None;
    };
    let (first, second) = (pattern_text(first)?, pattern_text(second)?);
    let step = second.strip_prefix("At the beginning ")?;
    first
        .starts_with("When ")
        .then(|| format!("{first} and at the beginning {step}"))
}

/// "Untap those creatures. They gain haste until end of turn." after a target for each opponent was taken: the
/// sentences after the one that "for each opponent" opens, which name its targets.
pub(super) fn w6e_those_untapped(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let Targetable::PerPlayer { what, .. } = &r.targets.first()?.what else {
        return None;
    };
    let noun = if what.0.contains(&Pred::Type(CardType::Creature)) {
        "creature"
    } else {
        what.0.is_empty().then_some("permanent")?
    };
    r.lookahead(|r| r.words("untap those").then_some(()))?;
    w6e_untapped(r, &Ref::Target(0), noun)
}
