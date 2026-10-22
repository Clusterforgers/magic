//! Abilities: a clause's sentences become a spell ability, an event, a comma and sentences a
//! triggered ability, and a cost, a colon, sentences and the restrictions on when it may be
//! activated an activated ability.

use super::tokens::Reader;
use super::{costs, effects, modal, statics, triggers};
use crate::ability::{Ability, Limit, Timing};
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

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
    r.that_player = about_a_player(&when);
    r.that_permanent = matches!(
        when,
        TriggerPattern::Tapped { .. } | TriggerPattern::Untapped { .. }
    );
    if triggers::targeted_itself(&when) {
        r.it = Some(Ref::This);
    }
    r.at_monarch = matches!(
        when,
        TriggerPattern::AttacksPlayer {
            at: Ref::Monarch,
            ..
        }
    );
    r.cast_spell = casts_a_spell(&when);
    r.attacker = matches!(&when, TriggerPattern::Attacks { what } if *what == Selector::this());
    // The creature that attacks is "it" and "its controller" is the player attacking.
    if matches!(when, TriggerPattern::AttacksPlayer { .. }) {
        r.it = Some(Ref::Triggering);
    }
    // A permanent entering is not the card itself, which may wait in the graveyard for it.
    r.from_graveyard = matches!(
        &when,
        TriggerPattern::ZoneChange { what, to: Some(ZoneKind::Battlefield), .. } if what.0 != [Pred::This]
    );
    r.punct(',').then_some(())?;
    let intervening_if = r.attempt(|r| {
        r.word("if").then_some(())?;
        let condition = triggers::condition(r)?;
        r.punct(',').then_some(condition)
    });
    let effects = sentences(r, false)?;
    // A card cycled is in its owner's graveyard when its own ability triggers, and a card that returns itself from the
    // graveyard has its ability there.
    let from = if triggers::cycled_itself(&when) || effects::returns_itself(&effects) {
        ZoneKind::Graveyard
    } else {
        ZoneKind::Battlefield
    };
    Some(Ability::Triggered {
        when,
        intervening_if,
        limit: None,
        from,
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}

/// Whether the event is a spell being cast, in every one of the events joined by "or", also the nth in a turn.
fn casts_a_spell(when: &TriggerPattern) -> bool {
    match when {
        TriggerPattern::AnyOf(events) => events.iter().all(casts_a_spell),
        TriggerPattern::Nth { event, .. } => casts_a_spell(event),
        _ => matches!(when, TriggerPattern::SpellCast { .. }),
    }
}

/// Whether the event is about a player ("that player" in the effects is who cast the spell, drew the card, lost the
/// life or was dealt the damage), in every one of the events joined by "or".
fn about_a_player(when: &TriggerPattern) -> bool {
    if let TriggerPattern::AnyOf(events) = when {
        return events.iter().all(about_a_player);
    }
    matches!(
        when,
        TriggerPattern::SpellCast { .. }
            | TriggerPattern::Drew { .. }
            | TriggerPattern::Discarded { .. }
            | TriggerPattern::LifeGained { .. }
            | TriggerPattern::LifeLost { .. }
            | TriggerPattern::BecomesMonarch { .. }
            | TriggerPattern::Proliferated { .. }
            | TriggerPattern::Cycled { .. }
            | TriggerPattern::TappedForMana { .. }
            | TriggerPattern::DealsDamage {
                to: Some(Ref::EachPlayer | Ref::Opponents),
                ..
            }
    )
}

/// "Activate only as a sorcery." and "Activate only once each turn.", alone or joined by "and only"; also "Activate only
/// if you control a Swamp." alone.
fn restrictions(r: &mut Reader<'_>) -> Option<(Timing, Option<Limit>, Option<Condition>)> {
    if !r.words("activate only") {
        return Some((Timing::Instant, None, None));
    }
    if r.word("if") {
        let condition = statics::control(r)?;
        return r
            .sentence_end()
            .then_some((Timing::Instant, None, Some(condition)));
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
    r.sentence_end().then_some((timing, limit, None))
}

/// An activated ability: a cost, a colon, sentences and the restrictions on when it may be
/// activated: `{T}: Add {G}.`, `{2}, {T}, Sacrifice ~: Draw a card.`
pub fn activated(r: &mut Reader<'_>) -> Option<Ability> {
    let cost = costs::cost(r)?;
    r.punct(':').then_some(())?;
    // An X in the cost is the payer's to choose, so the effects may use it.
    r.x_free |= cost.has_x();
    let effects = sentences(r, true)?;
    let (timing, limit, condition) = restrictions(r)?;
    let targets = std::mem::take(&mut r.targets);
    // An ability without targets that adds mana is a mana ability (CR 605.1a) whatever else it does and plays
    // without the stack; the data holds it whole, and the engine plays the shapes it reads (`mana_shape`).
    Some(Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    })
}
