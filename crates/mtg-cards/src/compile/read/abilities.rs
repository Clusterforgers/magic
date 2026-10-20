//! Abilities: a clause's sentences become a spell ability, an event, a comma and sentences a
//! triggered ability, and a cost, a colon and mana a mana ability.

use mtg_core::mana::ManaType;

use super::tokens::Reader;
use super::{effects, modal, triggers};
use crate::ability::{Ability, Timing};
use crate::cost::{Cost, CostItem};
use crate::effect::{Effect, Produce};
use crate::refs::{Ref, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// The sentences of an ability, in order, every one read; `None` when one is not, or when a `Modal`
/// is not the only effect (the modes keep their own targets, so the ability has none).
fn sentences(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = Vec::new();
    while !r.end() {
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
    let effects = sentences(r)?;
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
    let effects = sentences(r)?;
    Some(Ability::Triggered {
        when,
        intervening_if,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}

/// The mana of a symbol, when it is one a land or creature can produce.
fn mana_type(symbol: &str) -> Option<ManaType> {
    ManaType::ALL
        .into_iter()
        .find(|mana| symbol.len() == 1 && symbol.starts_with(mana.symbol()))
}

/// "Add {G}{G}." or "Add one mana of any color."
fn add_mana(r: &mut Reader<'_>) -> Option<Effect> {
    r.need("add")?;
    let mana = if r.words("one mana of any color") {
        Produce::AnyColor(1)
    } else {
        let mut list = Vec::new();
        while let Some(mana) = r.attempt(|r| mana_type(r.symbol()?)) {
            list.push(mana);
        }
        if list.is_empty() {
            return None;
        }
        Produce::Fixed(list)
    };
    Some(Effect::AddMana {
        who: Ref::You,
        mana,
        times: Amount::one(),
    })
}

/// The cost before the colon: `{T}` alone.
fn cost(r: &mut Reader<'_>) -> Option<Cost> {
    (r.symbol()? == "T").then(|| Cost(vec![CostItem::Tap]))
}

/// A plain mana ability: `{T}: Add {G}.`
pub fn mana_ability(r: &mut Reader<'_>) -> Option<Ability> {
    let cost = cost(r)?;
    r.punct(':').then_some(())?;
    let mana = add_mana(r)?;
    r.sentence_end().then_some(())?;
    Some(Ability::Activated {
        cost,
        timing: Timing::Instant,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![mana],
    })
}
