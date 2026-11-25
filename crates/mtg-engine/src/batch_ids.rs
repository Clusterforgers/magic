//! The ids a batch of events hands out, made unique as the batch is folded (`FreshIds`).
//!
//! The replacement effects of one batch of simultaneous events read the state before the batch (CR 614.1a: a shield
//! exiling each creature a wipe destroys, two Firestorm Phoenixes returning to hand with a play rule each, copies
//! entering together), so the effects of two of them can name the same next delayed trigger, continuous effect or rule
//! change. The tokens they make get fresh ids in the replacement step itself (`w9c_fresh_tokens`).

use std::collections::BTreeMap;

use mtg_core::ids::EffectId;

use crate::events::Event;
use crate::state::GameState;
use crate::triggers::DelayedId;

/// Gives each delayed trigger, continuous effect and rule change a batch creates an id no other has, as the batch is
/// folded: one that names an id already taken gets the next free one, and the events of the batch after it that name
/// the old id (its trigger, its removal, a shield's use) name the new one.
#[derive(Debug, Default)]
pub(crate) struct FreshIds {
    delayed: BTreeMap<DelayedId, DelayedId>,
    effects: BTreeMap<EffectId, EffectId>,
}

impl FreshIds {
    /// The event with its ids renamed, when it names one to rename; `None` when it stays as it is.
    pub(crate) fn renamed(&mut self, state: &GameState, event: &Event) -> Option<Event> {
        match event {
            Event::DelayedTriggerCreated(created) if created.id.0 < state.next_delayed => {
                let fresh = DelayedId(state.next_delayed);
                self.delayed.insert(created.id, fresh);
                let mut created = created.clone();
                created.id = fresh;
                Some(Event::DelayedTriggerCreated(created))
            }
            Event::AbilityTriggered(triggered) => {
                let fresh = self.delayed_of(triggered.delayed)?;
                let mut triggered = triggered.clone();
                triggered.delayed = Some(fresh);
                Some(Event::AbilityTriggered(triggered))
            }
            Event::TriggerRemoved { delayed, .. } => {
                let fresh = self.delayed_of(*delayed)?;
                let mut event = event.clone();
                if let Event::TriggerRemoved { delayed, .. } = &mut event {
                    *delayed = Some(fresh);
                }
                Some(event)
            }
            Event::DelayedTriggerExpired { id } => Some(Event::DelayedTriggerExpired {
                id: self.delayed_of(Some(*id))?,
            }),
            Event::EffectCreated { effect } if taken(state, effect.id) => {
                let mut effect = effect.clone();
                effect.id = self.fresh_effect(state, effect.id);
                Some(Event::EffectCreated { effect })
            }
            Event::RuleCreated { effect } if taken(state, effect.id) => {
                let mut effect = effect.clone();
                effect.id = self.fresh_effect(state, effect.id);
                Some(Event::RuleCreated { effect })
            }
            Event::EffectExpired { id } => Some(Event::EffectExpired {
                id: *self.effects.get(id)?,
            }),
            Event::RuleExpired { id } => Some(Event::RuleExpired {
                id: *self.effects.get(id)?,
            }),
            Event::ShieldUsed { id, left } => Some(Event::ShieldUsed {
                id: *self.effects.get(id)?,
                left: *left,
            }),
            _ => None,
        }
    }

    /// The new id of a renamed delayed trigger.
    fn delayed_of(&self, id: Option<DelayedId>) -> Option<DelayedId> {
        if self.delayed.is_empty() {
            return None;
        }
        self.delayed.get(&id?).copied()
    }

    /// The next free effect id, noted as the new name of `old`.
    fn fresh_effect(&mut self, state: &GameState, old: EffectId) -> EffectId {
        let fresh = EffectId(state.next_effect);
        self.effects.insert(old, fresh);
        fresh
    }
}

/// Whether a continuous effect or a rule change in force already holds `id` (they share one counter).
fn taken(state: &GameState, id: EffectId) -> bool {
    id.0 < state.next_effect
        && (state.effects.iter().any(|effect| effect.id == id)
            || state.rule_effects.iter().any(|effect| effect.id == id))
}

#[cfg(test)]
mod tests {
    //! Goal: continuous effects and rule changes of one batch that name the same id each get one of their own, in
    //! either order and mixed, the batch's later events that name the old id follow the new one, and a batch whose ids
    //! are free folds as it is.
    use mtg_cards::keyword::Keyword;
    use mtg_core::ids::{EffectId, PlayerId};
    use rstest::rstest;

    use super::FreshIds;
    use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::rules::{Rule, RuleEffect};
    use crate::state::GameState;

    fn table() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        )
    }

    /// An effect (`rule` false) or a rule change created with `id`.
    fn created(id: u64, rule: bool) -> Event {
        if rule {
            Event::RuleCreated {
                effect: Box::new(RuleEffect {
                    id: EffectId(id),
                    rule: Rule::CantDraw(vec![PlayerId(1)]),
                    duration: Duration::UntilEndOfTurn,
                    source: None,
                    controller: PlayerId(0),
                    turn: 1,
                }),
            }
        } else {
            Event::EffectCreated {
                effect: Box::new(ContinuousEffect::new(
                    EffectId(id),
                    Selector::AllCreatures,
                    Modification::AddKeyword(Keyword::Flying),
                    Duration::UntilEndOfTurn,
                    1,
                )),
            }
        }
    }

    fn id_of(event: &Event) -> Option<u64> {
        match event {
            Event::EffectCreated { effect } => Some(effect.id.0),
            Event::RuleCreated { effect } => Some(effect.id.0),
            Event::EffectExpired { id } | Event::RuleExpired { id } => Some(id.0),
            _ => None,
        }
    }

    /// Folds `events` as the engine commits a batch; `Err` when one cannot be folded.
    fn fold_batch(
        game: &mut GameState,
        events: Vec<Event>,
        rename: bool,
    ) -> Result<Vec<u64>, String> {
        let mut fresh = FreshIds::default();
        let mut ids = Vec::new();
        for event in events {
            let event = if rename {
                fresh.renamed(game, &event).unwrap_or(event)
            } else {
                event
            };
            apply_event(game, &event).map_err(|error| error.to_string())?;
            ids.extend(id_of(&event));
        }
        Ok(ids)
    }

    #[rstest]
    fn each_creation_of_a_batch_that_names_one_id_gets_its_own(
        #[values(false, true)] first_rule: bool,
        #[values(false, true)] second_rule: bool,
        #[values(2, 3)] copies: usize,
    ) {
        let game = table();
        let next = game.next_effect;
        let batch: Vec<Event> = (0..copies)
            .map(|at| created(next, if at == 0 { first_rule } else { second_rule }))
            .collect();
        let mut plain = game.clone();
        if first_rule == second_rule {
            assert!(
                fold_batch(&mut plain, batch.clone(), false).is_err(),
                "a duplicate is refused"
            );
        }
        let mut renamed = game;
        let ids = fold_batch(&mut renamed, batch, true).unwrap();
        let wanted: Vec<u64> = (next..next + copies as u64).collect();
        assert_eq!(ids, wanted);
        assert_eq!(renamed.next_effect, next + copies as u64);
    }

    #[rstest]
    fn the_end_of_a_renamed_effect_follows_its_new_id(#[values(false, true)] rule: bool) {
        let mut game = table();
        let next = game.next_effect;
        let ended = if rule {
            Event::RuleExpired { id: EffectId(next) }
        } else {
            Event::EffectExpired { id: EffectId(next) }
        };
        let ids = fold_batch(
            &mut game,
            vec![created(next, rule), created(next, rule), ended],
            true,
        )
        .unwrap();
        assert_eq!(ids, [next, next + 1, next + 1]);
        let left: Vec<u64> = if rule {
            game.rule_effects.iter().map(|effect| effect.id.0).collect()
        } else {
            game.effects.iter().map(|effect| effect.id.0).collect()
        };
        assert_eq!(left, [next], "the second ended, the first stays");
    }

    #[test]
    fn free_ids_stay_as_they_are() {
        let mut game = table();
        let next = game.next_effect;
        let batch = vec![
            created(next, false),
            created(next + 1, true),
            created(next + 5, false),
        ];
        assert_eq!(
            fold_batch(&mut game, batch, true).unwrap(),
            [next, next + 1, next + 5]
        );
    }
}
