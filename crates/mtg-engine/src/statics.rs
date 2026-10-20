//! Static abilities (CR 604) as effects of the layers: what the permanents' static abilities do to the game now.
//!
//! The effects are gathered once for a state and kept with it until the next event changes it, so deriving the
//! characteristics of every permanent reads one list. Each modification of a static ability that works is one
//! [`StaticEffect`], stamped with the timestamp of its source and applied in the walk of `characteristics::derive` among
//! the effects of resolved spells and abilities (`docs/engine.md`, Continuous effects).

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use mtg_cards::ability::Ability;
use mtg_cards::modify::Modification as Wanted;
use mtg_cards::refs::Ref;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::Grant;
use crate::ability_reader::static_abilities;
use crate::characteristics::{Characteristics, controller_of, walk};
use crate::effects::{Layer, Modification};
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Scope, lower};
use crate::state::GameState;
use crate::zones::Zone;

thread_local! {
    /// How many readings of the game on this thread are looking at it without static abilities.
    static WITHOUT: Cell<u32> = const { Cell::new(0) };
}

/// Ends a reading without static abilities, whether the reading returned or unwound.
struct Reading;

impl Drop for Reading {
    fn drop(&mut self) {
        WITHOUT.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Runs `read` with the static abilities of every state switched off on this thread: the characteristics it asks for
/// are those the registered effects make, as if no static ability were in play.
///
/// A static ability's condition, amount and selector read the game, and the game is made of what the static abilities
/// do, so they read it this way: it is what keeps a derivation from asking for itself. (`docs/engine.md`, Continuous
/// effects: a static ability reads other objects before the static abilities have changed them.)
pub(crate) fn without_statics<T>(read: impl FnOnce() -> T) -> T {
    WITHOUT.with(|depth| depth.set(depth.get() + 1));
    let _reading = Reading;
    read()
}

fn switched_off() -> bool {
    WITHOUT.with(|depth| depth.get() > 0)
}

/// The static effects of a state, built when first asked for and dropped when the state changes.
#[derive(Default)]
pub(crate) struct StaticsCache(OnceLock<Vec<StaticEffect>>);

impl StaticsCache {
    /// Forgets the effects: the state they were built for has changed.
    pub(crate) fn reset(&mut self) {
        self.0 = OnceLock::new();
    }
}

// A clone is a state of its own that builds its own list when it first needs one.
impl Clone for StaticsCache {
    fn clone(&self) -> Self {
        StaticsCache::default()
    }
}

impl fmt::Debug for StaticsCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StaticsCache")
    }
}

impl GameState {
    /// The effects of the static abilities that work now, in no order, or none while no static ability can be in play
    /// (a game of cards with none costs nothing) and while a static ability's own reading of the game is under way.
    pub(crate) fn statics(&self) -> &[StaticEffect] {
        if !self.statics_possible || switched_off() {
            return &[];
        }
        self.statics.0.get_or_init(|| gather(self))
    }
}

/// One modification of a static ability that works now.
#[derive(Debug, Clone)]
pub(crate) struct StaticEffect {
    /// Which modification of which ability of which object it is.
    pub(crate) grant: Grant,
    /// The permanent with the ability.
    pub(crate) source: ObjectRef,
    /// Who controls it.
    pub(crate) controller: PlayerId,
    /// The layer of the modification, or none for a restriction.
    pub(crate) layer: Option<Layer>,
    /// The time the source entered the battlefield (CR 613.7).
    pub(crate) timestamp: u64,
    /// What it does, its amounts worked out for this state.
    pub(crate) modification: Modification,
    /// The static ability, which says what it affects.
    rules: Arc<Ability>,
    /// What the source is attached to, for an ability that affects it.
    host: Option<ObjectRef>,
}

impl StaticEffect {
    /// Whether the effect applies to `object`, which has `so_far` and `controller` at this point of the walk.
    pub(crate) fn affects(
        &self,
        state: &GameState,
        object: &GameObject,
        so_far: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        let Ability::Static { affected, .. } = &*self.rules else {
            return false;
        };
        match affected {
            Ref::This => object.reference() == self.source,
            Ref::Attached => self.host == Some(object.reference()),
            Ref::Each(selector) => without_statics(|| {
                Scope::announcing(state, self.controller, self.source)
                    .selects_with(selector, object, so_far, controller)
                    .unwrap_or(false)
            }),
            // The data check keeps every other reference out of a static ability.
            _ => false,
        }
    }
}

/// Whether the ability changes no characteristic before layer 7 and no ability, so that it only exists for as long as
/// its source keeps it: layers 7 and the restrictions come after the layer that takes abilities away (CR 613.6).
fn works_late(effects: &[StaticEffect]) -> bool {
    effects
        .iter()
        .all(|effect| effect.layer.is_none_or(|layer| layer > Layer::Ability))
}

/// The static abilities of the permanents of `state` that work, as effects.
///
/// An ability works while its source is on the battlefield (`from` is not read yet), its condition holds and, when it only
/// acts after layer 6, its source still has it after the abilities that other effects give and take away.
fn gather(state: &GameState) -> Vec<StaticEffect> {
    without_statics(|| {
        let mut early = Vec::new();
        // The abilities that act only in layer 7 or after it, by source and ability.
        let mut late: BTreeMap<(ObjectId, usize), Vec<StaticEffect>> = BTreeMap::new();
        for &id in state.zone(Zone::Battlefield) {
            let Some(object) = state.object(id) else {
                continue;
            };
            for (rule, ability) in static_abilities(object) {
                let effects = effects_of(state, object, rule, ability);
                if works_late(&effects) {
                    late.insert((id, rule), effects);
                } else {
                    early.extend(effects);
                }
            }
        }
        let mut keeps: BTreeMap<ObjectId, bool> = BTreeMap::new();
        for ((id, _), effects) in late {
            let kept = *keeps.entry(id).or_insert_with(|| {
                state.object(id).is_some_and(|object| {
                    // What the source has after layer 6 is what the effects up to it leave it.
                    let derived = walk(state, object, Some(Layer::Ability), &early);
                    !derived
                        .applied
                        .iter()
                        .any(|applied| matches!(applied.modification, Modification::LoseAbilities))
                })
            });
            if kept {
                early.extend(effects);
            }
        }
        early
    })
}

/// The effects of the static ability `rule` of `object`: one for each modification, unless its condition is false or
/// there is nothing it could affect.
fn effects_of(
    state: &GameState,
    object: &GameObject,
    rule: usize,
    ability: &Arc<Ability>,
) -> Vec<StaticEffect> {
    let Ability::Static {
        affected,
        mods,
        condition,
        from: mtg_cards::refs::ZoneKind::Battlefield,
    } = &**ability
    else {
        return Vec::new();
    };
    let Some(permanent) = object.permanent() else {
        return Vec::new();
    };
    let source = object.reference();
    let controller = controller_of(state, object.id()).unwrap_or_else(|| object.controller());
    let scope = Scope::announcing(state, controller, source);
    if let Some(condition) = condition
        && !scope.holds(condition).unwrap_or(false)
    {
        return Vec::new();
    }
    let host = match affected {
        Ref::Attached => state
            .host_of(object.id())
            .and_then(|id| state.object(id))
            .map(GameObject::reference),
        _ => None,
    };
    if matches!(affected, Ref::Attached) && host.is_none() {
        return Vec::new();
    }
    mods.iter()
        .enumerate()
        .filter_map(|(index, wanted): (usize, &Wanted)| {
            let modification = lower(&scope, wanted).ok()?;
            Some(StaticEffect {
                grant: Grant::Static {
                    source: object.id(),
                    rule,
                    index,
                },
                source,
                controller,
                layer: modification.layer(),
                timestamp: permanent.timestamp(),
                modification,
                rules: Arc::clone(ability),
                host,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
