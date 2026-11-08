//! A permanent with a finality counter that would be put into a graveyard from the battlefield is exiled instead (CR 122.1g).

use mtg_cards::refs::ZoneKind;
use mtg_cards::trigger::ReplaceBy;
use mtg_cards::value::CounterKind;

use super::{Action, Candidate, Origin, Pass, Slot, Unit};
use crate::zones::Zone;

impl Pass<'_> {
    /// The exile in place of the graveyard for a permanent that holds a finality counter, when it has not been applied.
    pub(super) fn finality_candidate(&self, slot: &Slot) -> Option<Candidate> {
        let Unit::Move {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(_),
            ..
        } = self.unit(&slot.item)
        else {
            return None;
        };
        let permanent = self.state.object(object)?;
        let marked = permanent
            .permanent
            .as_ref()
            .is_some_and(|held| held.counters_of(&CounterKind::Other("finality".to_owned())) > 0);
        if slot.applied.contains(&Origin::Finality) || !marked {
            return None;
        }
        Some(Candidate {
            origin: Origin::Finality,
            object: permanent.reference(),
            controller: permanent.controller(),
            action: Action::Replace(ReplaceBy::MoveTo(ZoneKind::Exile)),
            // The rule is no effect of the permanent, so a replacement of the graveyard move (Rest in Peace) is ordered with it (CR 616.1).
            is_self: false,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a permanent with a finality counter that would be put into a graveyard from the battlefield (destroyed, sacrificed, dead of damage or of zero toughness) is exiled instead, a creature card or a token alike; a permanent without the counter or with another counter, and a move to any zone but a graveyard, stay as they are.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::value::CounterKind;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::choose::Target;
    use crate::counters::CountersChanged;
    use crate::events::{DeathCause, Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::state::GameState;
    use crate::tokens::TokenCreated;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn finality() -> CounterKind {
        CounterKind::Other("finality".to_owned())
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    /// A Bear and a Token on the battlefield of seat 0, each with `kind` counters.
    fn game(kind: Option<CounterKind>) -> GameState {
        let mut table = state(
            vec![Vec::new(), Vec::new()],
            vec![vec![creature("Bear", 2, 2)], Vec::new()],
        );
        let token = ObjectId(table.next_object);
        let definition = CardDefinition::new("Token").with_type(CardType::Creature);
        apply_event(
            &mut table,
            &Event::TokenCreated(TokenCreated {
                object: token,
                controller: ME,
                name: "Token".into(),
                tapped: false,
                attacking: None,
                definition: Arc::new(definition),
                copy_of: None,
            }),
        )
        .unwrap();
        if let Some(counter) = kind {
            for name in ["Bear", "Token"] {
                let on = Target::Object(table.object(id(&table, name)).unwrap().reference());
                apply_event(
                    &mut table,
                    &Event::CountersChanged(CountersChanged {
                        on,
                        counter: counter.clone(),
                        delta: 1,
                    }),
                )
                .unwrap();
            }
        }
        table
    }

    fn run(game: &GameState, event: Event) -> Vec<Event> {
        match process(game, vec![event], &[]).unwrap() {
            Processed::Events(events) => events,
            other @ Processed::Ask { .. } => panic!("events were expected, got {other:?}"),
        }
    }

    fn moved(game: &GameState, name: &str, to: Zone, cause: ZoneCause) -> Event {
        Event::ZoneChanged {
            object: id(game, name),
            from: Zone::Battlefield,
            to,
            cause,
        }
    }

    #[rstest]
    #[case::destroyed(ZoneCause::Destroyed)]
    #[case::sacrificed(ZoneCause::Sacrificed)]
    #[case::state_based(ZoneCause::StateBased)]
    fn a_permanent_with_a_finality_counter_that_would_go_to_the_graveyard_is_exiled(
        #[values("Bear", "Token")] name: &str,
        #[case] cause: ZoneCause,
    ) {
        let table = game(Some(finality()));
        let graveyard = Zone::Graveyard(ME);
        let result = run(&table, moved(&table, name, graveyard, cause));
        assert_eq!(result, [moved(&table, name, Zone::Exile, cause)]);
    }

    #[test]
    fn a_creature_that_dies_with_a_finality_counter_is_exiled() {
        let table = game(Some(finality()));
        let died = Event::CreatureDied {
            object: id(&table, "Bear"),
            cause: DeathCause::LethalDamage,
        };
        let result = run(&table, died);
        assert_eq!(
            result,
            [moved(&table, "Bear", Zone::Exile, ZoneCause::StateBased)]
        );
    }

    #[rstest]
    #[case::no_counter(None)]
    #[case::another_counter(Some(CounterKind::PlusOnePlusOne))]
    fn a_creature_without_a_finality_counter_dies_as_always(
        #[values("Bear", "Token")] name: &str,
        #[case] kind: Option<CounterKind>,
    ) {
        let table = game(kind);
        let event = moved(&table, name, Zone::Graveyard(ME), ZoneCause::Destroyed);
        assert_eq!(run(&table, event.clone()), [event]);
    }

    #[test]
    fn a_move_to_another_zone_is_not_replaced() {
        let table = game(Some(finality()));
        let event = moved(&table, "Bear", Zone::Hand(ME), ZoneCause::Moved);
        assert_eq!(run(&table, event.clone()), [event]);
    }
}
