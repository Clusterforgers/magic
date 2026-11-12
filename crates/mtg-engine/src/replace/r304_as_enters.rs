//! `ReplaceBy::AsEnters` (CR 614.1c, 614.12): effects that happen as a permanent enters, before it is on the
//! battlefield ("As ~ enters, discard your hand"). Their events come first in the batch and the permanent then
//! enters as it would have; other replacements still apply to both.

use mtg_cards::effect::Effect;

use mtg_cards::value::{Amount, CounterKind};

use super::w5d_instead::Ran;
use super::{Applied, Candidate, Item, Origin, Pass, Slot, Unit, slots_of};
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Inline, r304_run_then_count};

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

impl Pass<'_> {
    /// Runs `effects` for the permanent entering at `at`, in front of its entering, then has it enter with `count`
    /// counters of the kind, the count read after the effects; the counters go on the permanent it becomes (CR 400.7).
    pub(super) fn r304_as_enters_with(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (effects, counter, count): (&[Effect], &CounterKind, &Amount),
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Applied {
        let cause = self.w5d_cause(at, unit);
        let state = self.state;
        let answers = &mut self.answers;
        let (events, number) = match r304_run_then_count(
            state,
            candidate.controller,
            candidate.object,
            &cause,
            (effects, count),
            &mut || answers.next().cloned(),
        ) {
            Ok((Inline::Events(events), number)) => (events, number),
            Ok((Inline::Ask(decision), _)) => return Applied::Ask(decision),
            Ok((Inline::Declined, _)) | Err(_) => (Vec::new(), 0),
        };
        let events = self.w9c_fresh_tokens(events);
        let mut entering = self.slots[at].clone();
        entering.applied.clone_from(&applied);
        let mut slots = slots_of(events);
        slots.push(entering);
        let object = match unit {
            Unit::Enter { object, .. } => Some(*object),
            _ => None,
        };
        if let (Some(card), Ok(delta)) = (
            object.and_then(|id| self.state.object(id)),
            i32::try_from(number),
        ) && delta > 0
        {
            slots.push(Slot {
                item: Item::One(Event::CountersChanged(CountersChanged {
                    on: Target::Object(ObjectRef {
                        id: card.id,
                        counter: card.counter() + 1,
                    }),
                    counter: counter.clone(),
                    delta,
                })),
                applied,
                frozen: false,
            });
        }
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

    const PHYLACTERY: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEnters":[{"Choose":{"from":{"Each":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"as":"chosen"}},{"PutCounters":{"on":{"Bound":"chosen"},"counter":{"Other":"phylactery"}}}]}}}"#;

    #[rstest]
    #[case::the_first(0)]
    #[case::the_second(1)]
    fn the_chosen_artifact_gets_the_counter_before_it_enters(#[case] pick: usize) {
        let rule: Ability = serde_json::from_str(PHYLACTERY).unwrap();
        let lich = CardDefinition::new("Lich")
            .with_type(CardType::Creature)
            .with_rules(rule);
        let relic = || Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
        let game = state(
            vec![vec![Arc::new(lich)], vec![]],
            vec![vec![relic(), relic()], vec![relic()]],
        );
        let (_, entering) = enters(&game, 0);
        let mine: Vec<ObjectId> = game.zone(Zone::Battlefield)[..2].to_vec();
        let decision = match process(&game, vec![entering.clone()], &[]).unwrap() {
            Processed::Ask { decision, .. } => decision,
            Processed::Events(events) => panic!("the controller chooses: {events:?}"),
        };
        assert_eq!(decision.player, PlayerId(0));
        let journal = [(PlayerId(0), crate::choose::Choice::Cards(vec![mine[pick]]))];
        let out = match process(&game, vec![entering.clone()], &journal).unwrap() {
            Processed::Events(events) => events,
            Processed::Ask { decision, .. } => panic!("asked again: {decision:?}"),
        };
        assert_eq!(out.last(), Some(&entering), "{out:?}");
        let countered: Vec<_> = out
            .iter()
            .filter_map(|event| match event {
                Event::CountersChanged(changed) => Some(changed.on),
                _ => None,
            })
            .collect();
        assert_eq!(
            countered,
            vec![crate::choose::Target::Object(
                game.object(mine[pick]).unwrap().reference()
            )],
            "{out:?}"
        );
    }

    const THIEF: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEntersWith":{"effects":[{"Let":{"as":"removed","amount":{"AnyCounters":{"Each":[]}}}},{"RemoveAllCounters":{"from":{"Each":[]}}}],"counter":"+1/+1","count":{"Bound":"removed"}}}}}"#;
    const THRESHER: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEntersWith":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Artifact"},"Other",{"In":"Hand"},{"OwnedBy":"You"}]},"count":{"min":0,"max":null},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}}],"counter":"+1/+1","count":{"CountOf":{"of":{"Bound":"revealed"},"filter":[]}}}}}}"#;

    /// The +1/+1 counters the entering card gets as the permanent it becomes, after it enters.
    fn counters_after(out: &[Event], entering: &Event, card: ObjectId, game: &GameState) -> i32 {
        let at = out
            .iter()
            .position(|event| event == entering)
            .expect("it enters");
        let becomes = crate::object::ObjectRef {
            id: card,
            counter: game.object(card).unwrap().counter() + 1,
        };
        out[at..]
            .iter()
            .filter_map(|event| match event {
                Event::CountersChanged(changed)
                    if changed.on == crate::choose::Target::Object(becomes) =>
                {
                    Some(changed.delta)
                }
                _ => None,
            })
            .sum()
    }

    #[rstest]
    #[case::none(&[], 0)]
    #[case::one_permanent(&[3], 3)]
    #[case::three_permanents(&[1, 2, 4], 7)]
    fn the_counters_removed_from_every_permanent_go_on_it(
        #[case] each: &[i32],
        #[case] total: i32,
    ) {
        let rule: Ability = serde_json::from_str(THIEF).unwrap();
        let thief = CardDefinition::new("Thief")
            .with_type(CardType::Creature)
            .with_rules(rule);
        let mut game = state(
            vec![vec![Arc::new(thief)], vec![]],
            vec![
                each.iter().map(|_| creature("Bear", 2, 2)).collect(),
                vec![creature("Bear", 2, 2)],
            ],
        );
        for (at, delta) in each.iter().enumerate() {
            let id = game.zone(Zone::Battlefield)[at];
            let on = crate::choose::Target::Object(game.object(id).unwrap().reference());
            let put = Event::CountersChanged(crate::counters::CountersChanged {
                on,
                counter: mtg_cards::value::CounterKind::Other("charge".into()),
                delta: *delta,
            });
            crate::fold::apply_event(&mut game, &put).unwrap();
        }
        let (card, entering) = enters(&game, 0);
        let out = events(&game, vec![entering.clone()]);
        let removed: i32 = out
            .iter()
            .filter_map(|event| match event {
                Event::CountersChanged(changed) if changed.delta < 0 => Some(-changed.delta),
                _ => None,
            })
            .sum();
        assert_eq!(removed, total, "{out:?}");
        assert_eq!(
            counters_after(&out, &entering, card, &game),
            total,
            "{out:?}"
        );
    }

    #[rstest]
    #[case::none(0)]
    #[case::one(1)]
    #[case::both(2)]
    fn each_artifact_card_revealed_is_a_counter(#[case] shown: usize) {
        let rule: Ability = serde_json::from_str(THRESHER).unwrap();
        let thresher = CardDefinition::new("Thresher")
            .with_type(CardType::Artifact)
            .with_rules(rule);
        let relic = || Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
        let game = state(
            vec![
                vec![Arc::new(thresher), relic(), relic(), creature("Bear", 2, 2)],
                vec![],
            ],
            vec![vec![], vec![]],
        );
        let (card, entering) = enters(&game, 0);
        let relics: Vec<ObjectId> = game.zone(Zone::Hand(PlayerId(0)))[1..3].to_vec();
        let decision = match process(&game, vec![entering.clone()], &[]).unwrap() {
            Processed::Ask { decision, .. } => decision,
            Processed::Events(events) => panic!("the controller chooses: {events:?}"),
        };
        assert_eq!(decision.player, PlayerId(0));
        let journal = [(
            PlayerId(0),
            crate::choose::Choice::Cards(relics[..shown].to_vec()),
        )];
        let out = match process(&game, vec![entering.clone()], &journal).unwrap() {
            Processed::Events(events) => events,
            Processed::Ask { decision, .. } => panic!("asked again: {decision:?}"),
        };
        let expected = i32::try_from(shown).unwrap();
        assert_eq!(
            counters_after(&out, &entering, card, &game),
            expected,
            "{out:?}"
        );
    }

    const SPHINX: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsEntersWith":{"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Not":{"Type":"Land"}},{"ControlledBy":"You"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}],"counter":"+1/+1","count":{"CountOf":{"of":{"Bound":"returned"},"filter":[]}}}}}}"#;

    #[rstest]
    #[case::returned(true, 1)]
    #[case::declined(false, 0)]
    fn a_permanent_returned_is_a_counter(#[case] yes: bool, #[case] counters: i32) {
        let rule: Ability = serde_json::from_str(SPHINX).unwrap();
        let sphinx = CardDefinition::new("Sphinx")
            .with_type(CardType::Creature)
            .with_rules(rule);
        let game = state(
            vec![vec![Arc::new(sphinx)], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        let bear = game.zone(Zone::Battlefield)[0];
        let (card, entering) = enters(&game, 0);
        let mut journal = vec![(PlayerId(0), crate::choose::Choice::YesNo(yes))];
        let out = loop {
            match process(&game, vec![entering.clone()], &journal).unwrap() {
                Processed::Events(events) => break events,
                Processed::Ask { .. } if journal.len() < 3 => {
                    journal.push((PlayerId(0), crate::choose::Choice::Cards(vec![bear])));
                }
                Processed::Ask { decision, .. } => panic!("asked again: {decision:?}"),
            }
        };
        let back = out.iter().any(|event| {
            matches!(event, Event::ZoneChanged { object, to: Zone::Hand(_), .. } if *object == bear)
        });
        assert_eq!(back, yes, "{out:?}");
        assert_eq!(
            counters_after(&out, &entering, card, &game),
            counters,
            "{out:?}"
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
