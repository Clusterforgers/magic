//! Abilities: a clause's sentences become a spell ability, an event, a comma and sentences a
//! triggered ability, and a cost, a colon, sentences and the restrictions on when it may be
//! activated an activated ability.

use super::tokens::Reader;
use super::{costs, effects, modal, triggers};
use crate::ability::{Ability, Limit, Timing};
use crate::effect::Effect;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The sentences of an ability, in order, every one read; `None` when one is not, or when a `Modal`
/// is not the only effect (the modes keep their own targets, so the ability has none). With
/// `restricted` the sentences end where an "Activate only ..." sentence begins.
fn sentences(r: &mut Reader<'_>, restricted: bool) -> Option<Vec<Effect>> {
    let mut effects = Vec::new();
    let restriction = |r: &Reader<'_>| restricted && r.peek_word() == Some("activate");
    while !r.end() && !restriction(r) {
        if !effects::sentence(r, &mut effects) {
            return None;
        }
    }
    let alone = effects.len() == 1 || !effects.iter().any(|e| matches!(e, Effect::Modal { .. }));
    // A name an effect binds is bound once in an ability.
    let binds: Vec<&str> = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Choose { bind, .. } => Some(bind.as_str()),
            _ => None,
        })
        .collect();
    let unique = binds
        .iter()
        .enumerate()
        .all(|(at, name)| !binds[..at].contains(name));
    (!effects.is_empty() && alone && unique).then_some(effects)
}

/// The sentences of an instant or sorcery, in order; every one must be read.
pub fn spell(r: &mut Reader<'_>) -> Option<Ability> {
    let effects = sentences(r, false)?;
    Some(Ability::Spell {
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}

/// The modes of an instant or sorcery: "Choose one —" and its bullets, as the only effect.
pub fn modal_spell(r: &mut Reader<'_>) -> Option<Ability> {
    let effects = modal::modal(r)?;
    r.end().then_some(Ability::Spell {
        targets: Vec::new(),
        effects,
    })
}

/// A triggered ability: an event, a comma, an optional intervening "if" and then sentences, every
/// one of which must be read.
pub fn triggered(r: &mut Reader<'_>) -> Option<Ability> {
    let when = triggers::pattern(r)?;
    r.that_player = matches!(when, TriggerPattern::SpellCast { .. });
    r.punct(',').then_some(())?;
    let intervening_if = r.attempt(|r| {
        r.word("if").then_some(())?;
        let condition = triggers::condition(r)?;
        r.punct(',').then_some(condition)
    });
    let effects = sentences(r, false)?;
    Some(Ability::Triggered {
        when,
        intervening_if,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}

/// "Activate only as a sorcery." and "Activate only once each turn.", alone or joined by "and only".
fn restrictions(r: &mut Reader<'_>) -> Option<(Timing, Option<Limit>)> {
    if !r.words("activate only") {
        return Some((Timing::Instant, None));
    }
    let (mut timing, mut limit) = (Timing::Instant, None);
    loop {
        if timing == Timing::Instant && r.words("as a sorcery") {
            timing = Timing::Sorcery;
        } else if limit.is_none() && r.words("once each turn") {
            limit = Some(Limit::OncePerTurn);
        } else {
            return None;
        }
        if !r.words("and only") {
            break;
        }
    }
    r.sentence_end().then_some((timing, limit))
}

/// An activated ability: a cost, a colon, sentences and the restrictions on when it may be
/// activated: `{T}: Add {G}.`, `{2}, {T}, Sacrifice ~: Draw a card.`
pub fn activated(r: &mut Reader<'_>) -> Option<Ability> {
    let cost = costs::cost(r)?;
    r.punct(':').then_some(())?;
    // An X in the cost is the payer's to choose, so the effects may use it.
    r.x_free |= cost.has_x();
    let effects = sentences(r, true)?;
    let (timing, limit) = restrictions(r)?;
    Some(Ability::Activated {
        cost,
        timing,
        condition: None,
        limit,
        from: ZoneKind::Battlefield,
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}
