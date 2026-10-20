//! Abilities: a clause's sentences become a spell ability, and a cost, a colon and mana a mana ability.

use mtg_core::mana::ManaType;

use super::effects;
use super::tokens::Reader;
use crate::ability::{Ability, Timing};
use crate::cost::{Cost, CostItem};
use crate::effect::{Effect, Produce};
use crate::refs::{Ref, ZoneKind};
use crate::value::Amount;

/// The sentences of an instant or sorcery, in order; every one must be read.
pub fn spell(r: &mut Reader<'_>) -> Option<Ability> {
    let mut effects = Vec::new();
    while !r.end() {
        if !effects::sentence(r, &mut effects) {
            return None;
        }
    }
    if effects.is_empty() {
        return None;
    }
    Some(Ability::Spell {
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
