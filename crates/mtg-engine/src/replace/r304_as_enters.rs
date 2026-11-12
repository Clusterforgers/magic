//! `ReplaceBy::AsEnters` (CR 614.1c, 614.12): effects that happen as a permanent enters, before it is on the
//! battlefield ("As ~ enters, discard your hand"). Their events come first in the batch and the permanent then
//! enters as it would have; other replacements still apply to both.

use mtg_cards::effect::Effect;

use super::w5d_instead::Ran;
use super::{Applied, Candidate, Origin, Pass, Unit, slots_of};

impl Pass<'_> {
    /// Runs `effects` for the permanent entering at `at`, putting their events in front of its entering.
    pub(super) fn r304_as_enters(
        &mut self,
        at: usize,
        candidate: &Candidate,
        effects: &[Effect],
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Applied {
        let events = match self.w5d_run(at, candidate, effects, unit) {
            Ran::Events(events) => events,
            Ran::Ask(decision) => return Applied::Ask(decision),
            Ran::Kept => Vec::new(),
        };
        let events = self.w9c_fresh_tokens(events);
        let mut entering = self.slots[at].clone();
        entering.applied = applied;
        let mut slots = slots_of(events);
        // The effects' events are new events: no replacement has applied to them yet (CR 614.5 is about the entering).
        slots.push(entering);
        self.slots.splice(at..=at, slots);
        Applied::Done
    }
}

#[cfg(test)]
mod tests {
    //! Goal: effects as a permanent enters happen before it enters, in the same batch (the lands are sacrificed, the
    //! life is lost), and only for the permanent with the ability; a permanent without it enters unchanged.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::state::GameState;
    use crate::zones::Zone;

    const SACRIFICE: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEnters":[{"Sacrifice":{"filter":[{"Type":"Land"},{"ControlledBy":"You"}],"count":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]}}}"#;
    const LOSE: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEnters":[{"LoseLife":{"amount":{"Life":"You"}}}]}}}"#;

    fn land() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Forest").with_type(CardType::Land))
    }

    /// Seat 0 holds the Source (with `rules`) and a Bear and controls `lands` Forests; seat 1 controls a Forest.
    fn game(rules: &str, lands: usize) -> GameState {
        let rule: Ability = serde_json::from_str(rules).unwrap();
        let source = CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(rule);
        state(
            vec![vec![Arc::new(source), creature("Bear", 2, 2)], vec![]],
            vec![(0..lands).map(|_| land()).collect(), vec![land()]],
        )
    }

    fn enters(game: &GameState, index: usize) -> (ObjectId, Event) {
        let card = game.zone(Zone::Hand(PlayerId(0)))[index];
        let event = Event::ZoneChanged {
            object: card,
            from: Zone::Hand(PlayerId(0)),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        };
        (card, event)
    }

    fn events(game: &GameState, batch: Vec<Event>) -> Vec<Event> {
        match process(game, batch, &[]).unwrap() {
            Processed::Events(events) => events,
            Processed::Ask { .. } => panic!("nothing is asked"),
        }
    }

    #[rstest]
    #[case::no_land(0)]
    #[case::one_land(1)]
    #[case::three_lands(3)]
    fn the_controllers_lands_are_sacrificed_before_it_enters(#[case] lands: usize) {
        let game = game(SACRIFICE, lands);
        let (source, entering) = enters(&game, 0);
        let out = events(&game, vec![entering.clone()]);
        assert_eq!(out.last(), Some(&entering), "{out:?}");
        let sacrificed: Vec<_> = out
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::ZoneChanged {
                        from: Zone::Battlefield,
                        to: Zone::Graveyard(PlayerId(0)),
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(sacrificed.len(), lands, "{out:?}");
        assert!(out.iter().all(|event| !matches!(
            event,
            Event::ZoneChanged {
                to: Zone::Graveyard(PlayerId(1)),
                ..
            }
        )));
        assert!(
            out.iter().any(
                |event| matches!(event, Event::ZoneChanged { object, .. } if *object == source)
            )
        );
    }

    #[rstest]
    #[case::source(0, true)]
    #[case::another_card(1, false)]
    fn only_the_permanent_with_the_ability_loses_the_life(
        #[case] index: usize,
        #[case] loses: bool,
    ) {
        let game = game(LOSE, 1);
        let life = game.player(PlayerId(0)).unwrap().life;
        let (_, entering) = enters(&game, index);
        let out = events(&game, vec![entering.clone()]);
        let expected = if loses {
            vec![
                Event::LifeLost {
                    player: PlayerId(0),
                    amount: u32::try_from(life).unwrap(),
                },
                entering,
            ]
        } else {
            vec![entering]
        };
        assert_eq!(out, expected);
    }
}
