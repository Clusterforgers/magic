//! What changes an object now and where each change comes from: the record of the layers (`Held::applied`), the counters
//! on it and the copy effect it is under, read for the views (`ObjectView.modifiers`). Nothing is changed or worked out
//! a second time: an effect that ended, a condition that fails or an ability its source lost leaves nothing.

use mtg_cards::modify::Duration;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::Grant;
use crate::effects::Modification;
use crate::keywords::keywords_of;
use crate::layers::Applied;
use crate::object::ObjectRef;
use crate::state::GameState;

pub use crate::announce::{CostChangeBy, cost_changes_by_source};

/// Where a modifier comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModifierSource {
    /// An object: the permanent or emblem with the static ability, the spell or ability that began the effect, the
    /// card whose abilities it borrows, the object it copies.
    Object(ObjectId),
    /// The Ring of `player` at level `level` (CR 701.54), which is no object.
    Ring {
        /// Whose Ring it is.
        player: PlayerId,
        /// The level that gives it.
        level: u8,
    },
    /// The counters on the object itself.
    Counters,
    /// An effect no object made (a test's or a rule's).
    Game,
}

/// One change a modifier makes.
#[derive(Debug, Clone, PartialEq)]
pub enum ModifierChange {
    /// A modification the layers applied.
    Modification(Modification),
    /// `count` counters of `kind`, which change its power and toughness or give it a keyword (CR 122.1).
    Counters {
        /// The kind of counter.
        kind: CounterKind,
        /// How many there are.
        count: u32,
    },
    /// It is a copy of the card named `name` (layer 1, CR 707).
    Copy {
        /// The name of what it copies.
        name: String,
    },
}

/// The changes of one origin (one static ability, one resolved spell or ability, one kind of counter, the copy effect)
/// to an object, in the order the layers applied them.
#[derive(Debug, Clone, PartialEq)]
pub struct Modifier {
    /// Where it comes from.
    pub source: ModifierSource,
    /// What it does.
    pub changes: Vec<ModifierChange>,
    /// How long a resolved spell or ability's effect lasts; none for a static ability, counters and a copy.
    pub duration: Option<Duration>,
}

/// Which origin an applied change belongs to: the changes of one origin are one modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Origin {
    Static(ObjectId, usize),
    Effect(Option<ObjectRef>, Duration),
    Borrowed(ObjectId),
    Ring(PlayerId, u8),
}

/// Everything that changes the object `id` now, by origin: the copy effect first, then the effects and static abilities
/// in the order the layers applied them, then its counters that change power and toughness or give a keyword. Empty for
/// an object no effect changes and for one not in the game.
pub fn modifiers(state: &GameState, id: ObjectId) -> Vec<Modifier> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(copy) = object.copy_effect() {
        out.push(Modifier {
            source: ModifierSource::Object(copy.of().id),
            changes: vec![ModifierChange::Copy {
                name: copy.shown().name.clone(),
            }],
            duration: None,
        });
    }
    let mut origins: Vec<Origin> = Vec::new();
    let layers = state.layers();
    let applied: &[Applied] = layers.get(id).map_or(&[], |held| held.applied.as_slice());
    for change in applied {
        let Some((origin, source, duration)) = origin_of(state, change) else {
            continue;
        };
        let modification = ModifierChange::Modification(change.modification.clone());
        if let Some(at) = origins.iter().position(|seen| *seen == origin) {
            // The copy effect, if any, sits before the origins.
            let at = at + usize::from(object.copy_effect().is_some());
            out[at].changes.push(modification);
        } else {
            origins.push(origin);
            out.push(Modifier {
                source,
                changes: vec![modification],
                duration,
            });
        }
    }
    if let Some(permanent) = object.permanent() {
        let keywords = keywords_of(state, id);
        for (kind, count) in permanent.counters() {
            let counts = kind.power_toughness() != (0, 0)
                || kind
                    .keyword()
                    .is_some_and(|keyword| keywords.contains(&&keyword));
            if counts && count > 0 {
                out.push(Modifier {
                    source: ModifierSource::Counters,
                    changes: vec![ModifierChange::Counters {
                        kind: kind.clone(),
                        count,
                    }],
                    duration: None,
                });
            }
        }
    }
    out
}

/// The origin of an applied change, with its source and duration; none for an effect no longer in the game.
fn origin_of(
    state: &GameState,
    applied: &Applied,
) -> Option<(Origin, ModifierSource, Option<Duration>)> {
    Some(match applied.origin {
        Grant::Static { source, rule, .. } => (
            Origin::Static(source, rule),
            ModifierSource::Object(source),
            None,
        ),
        Grant::Borrowed { donor, .. } | Grant::R111Lent { donor, .. } => {
            (Origin::Borrowed(donor), ModifierSource::Object(donor), None)
        }
        Grant::R114Sticker(at) => {
            let donor = crate::un34_stickers::r114_sticker_holder(state, at)?;
            (Origin::Borrowed(donor), ModifierSource::Object(donor), None)
        }
        Grant::Ring { player, level } => (
            Origin::Ring(player, level),
            ModifierSource::Ring { player, level },
            None,
        ),
        Grant::Effect(effect) | Grant::R6701Nested { effect, .. } => {
            let effect = state.effects().iter().find(|e| e.id == effect)?;
            // The effects one resolution began share its source and duration; those of no object go together likewise.
            (
                Origin::Effect(effect.source, effect.duration.clone()),
                effect.source.map_or(ModifierSource::Game, |source| {
                    ModifierSource::Object(source.id)
                }),
                Some(effect.duration.clone()),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    //! Goal: an object's modifiers are read off what the layers applied, one per origin with that origin's changes, a
    //! resolved effect with its duration, the copy effect first and only counters that change it, and an object no
    //! effect changes has none.
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::effects::Selector;
    use crate::engine::Engine;
    use crate::fixtures::{creature, setup};
    use crate::setup::PresetPermanent;

    /// A game where seat 0 controls a Bear with `counters` and the cards `more`.
    fn engine(counters: &[(CounterKind, u32)], more: Vec<Arc<CardDefinition>>) -> Engine {
        let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        setup.players[0].battlefield = vec![PresetPermanent {
            counters: counters.to_vec(),
            ..PresetPermanent::new(creature("Bear", 2, 2))
        }];
        setup.players[0]
            .battlefield
            .extend(more.into_iter().map(PresetPermanent::new));
        Engine::new(setup).unwrap()
    }

    fn named(engine: &Engine, name: &str) -> ObjectId {
        engine
            .state()
            .objects()
            .find(|o| o.card().name == name)
            .unwrap()
            .id()
    }

    fn anthem(name: &str) -> Arc<CardDefinition> {
        let ability = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}]}}"#;
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Enchantment)
                .with_rules(serde_json::from_str(ability).unwrap()),
        )
    }

    #[rstest]
    #[case::nothing(vec![], vec![], 0)]
    #[case::one_anthem(vec![], vec![anthem("A")], 1)]
    #[case::two_anthems(vec![], vec![anthem("A"), anthem("B")], 2)]
    #[case::counters(vec![(CounterKind::PlusOnePlusOne, 3)], vec![], 1)]
    #[case::charge_counters_change_nothing(vec![(CounterKind::Charge, 2)], vec![], 0)]
    #[case::both(vec![(CounterKind::MinusOneMinusOne, 1)], vec![anthem("A")], 2)]
    fn one_modifier_per_origin(
        #[case] counters: Vec<(CounterKind, u32)>,
        #[case] more: Vec<Arc<CardDefinition>>,
        #[case] expected: usize,
    ) {
        let engine = engine(&counters, more);
        let bear = named(&engine, "Bear");
        let found = modifiers(engine.state(), bear);
        assert_eq!(found.len(), expected, "{found:?}");
        for modifier in &found {
            if let ModifierSource::Object(_) = modifier.source {
                // An anthem's two modifications are one modifier.
                assert_eq!(modifier.changes.len(), 2);
                assert_eq!(modifier.duration, None);
            }
        }
    }

    #[test]
    fn a_resolved_effect_keeps_its_source_and_duration() {
        let mut engine = engine(&[], vec![anthem("Pump")]);
        let bear = named(&engine, "Bear");
        let pump = named(&engine, "Pump");
        let on = Selector::objects(engine.state(), [bear]);
        engine
            .add_continuous_effect_from(
                pump,
                on,
                Modification::ModifyPowerToughness {
                    power: 3,
                    toughness: 3,
                },
                Duration::UntilEndOfTurn,
            )
            .unwrap();
        let found = modifiers(engine.state(), bear);
        let effect = found.iter().find(|m| m.duration.is_some()).unwrap();
        assert_eq!(effect.source, ModifierSource::Object(pump));
        assert_eq!(effect.duration, Some(Duration::UntilEndOfTurn));
        assert_eq!(
            effect.changes,
            vec![ModifierChange::Modification(
                Modification::ModifyPowerToughness {
                    power: 3,
                    toughness: 3
                }
            )]
        );
    }
}
