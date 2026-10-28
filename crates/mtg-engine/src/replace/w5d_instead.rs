//! `ReplaceBy::Instead` (CR 614.1a): effects that happen in place of the event. They read the event as a trigger reads
//! its cause: the player or object it is about (`Triggering`), its amount and the source of its damage (`Dealer`).

use mtg_cards::effect::Effect;

use super::{Candidate, Item, Origin, Pass, Unit, slots_of};
use crate::abilities::Cause;
use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::w5d_run_instead;

impl Pass<'_> {
    /// What the effects of a replacement read of the event at `at`: who or what it is about, how much and its source.
    pub(super) fn w5d_cause(&self, at: usize, unit: &Unit) -> Cause {
        let object = |id| self.reference(id).map(Target::Object);
        let (subject, source) = match unit {
            Unit::Tokens { controller } => (Some(Target::Player(*controller)), None),
            Unit::Damage { source, target } => (
                match target {
                    DamageTarget::Player(player) => Some(Target::Player(*player)),
                    DamageTarget::Object(id) => object(*id),
                },
                self.reference(*source),
            ),
            Unit::Counters { on, .. } => (Some(*on), None),
            Unit::Life { player } | Unit::Draw { player } => (Some(Target::Player(*player)), None),
            Unit::Move { object: id, .. } | Unit::Enter { object: id, .. } => (object(*id), None),
            Unit::Other => (None, None),
        };
        let amount = match &self.slots[at].item {
            Item::Tokens(run) => u32::try_from(run.len()).unwrap_or(u32::MAX),
            Item::One(Event::DamageDealt { amount, .. } | Event::LifeGained { amount, .. }) => {
                *amount
            }
            Item::One(Event::CountersChanged(CountersChanged { delta, .. })) => {
                delta.unsigned_abs()
            }
            Item::One(Event::CardDrawn { .. } | Event::DrewFromEmptyLibrary { .. }) => 1,
            Item::One(_) => 0,
        };
        Cause {
            subjects: subject.into_iter().collect(),
            snapshots: Vec::new(),
            amount,
            source,
        }
    }

    /// The effects run in place of the event at `at`: their events take its place and carry the effects already applied
    /// to it, this one included, so it does not apply to them again (CR 614.5). Effects that cannot run without a
    /// question leave the event as it is.
    pub(super) fn w5d_instead(
        &mut self,
        at: usize,
        candidate: &Candidate,
        effects: &[Effect],
        unit: &Unit,
        applied: Vec<Origin>,
    ) {
        let cause = self.w5d_cause(at, unit);
        let Ok(events) = w5d_run_instead(
            self.state,
            candidate.controller,
            candidate.object,
            &cause,
            effects,
        ) else {
            self.slots[at].applied = applied;
            return;
        };
        if matches!(unit, Unit::Draw { .. }) {
            self.draws_changed = true;
        }
        let slots: Vec<_> = slots_of(events)
            .into_iter()
            .map(|mut slot| {
                slot.applied.clone_from(&applied);
                slot
            })
            .collect();
        self.slots.splice(at..=at, slots);
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an effect that happens instead takes the place of the event and reads its player, its amount and the
    //! source of its damage (life gained becomes life lost, damage to a player is dealt to a creature by the same source,
    //! tokens become as many other tokens, a death becomes an exile), applies once to what replaces it, and leaves the
    //! event as it is when its steps would ask a question.
    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::ObjectId;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::combat::DamageTarget;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::state::GameState;
    use crate::zones::Zone;

    const DRONE: &str = r#"{"Replacement":{"when":{"LifeGain":{"who":"Opponents"}},"by":{"Instead":[{"LoseLife":{"who":"Triggering","amount":"Triggering"}}]}}}"#;
    const REDIRECT: &str = r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Instead":[{"DealDamage":{"source":"Dealer","to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]},"amount":"Triggering"}}]}}}"#;
    const EXILE: &str = r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"by":{"Instead":[{"Exile":{"what":"Triggering"}}]}}}"#;
    const ASKS: &str = r#"{"Replacement":{"when":{"LifeGain":{"who":"Opponents"}},"by":{"Instead":[{"May":{"effects":[{"Draw":{}}]}}]}}}"#;

    /// Seat 0 controls a Source with `rules` and a Wall; seat 1 controls a Bear.
    fn game(rules: &str) -> GameState {
        let rule: Ability = serde_json::from_str(rules).unwrap();
        let source = CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(rule);
        state(
            vec![vec![], vec![]],
            vec![
                vec![std::sync::Arc::new(source), creature("Wall", 0, 4)],
                vec![creature("Bear", 2, 2)],
            ],
        )
    }

    /// The permanent at this place on the battlefield: the Source, the Wall, the Bear.
    fn at(game: &GameState, index: usize) -> ObjectId {
        game.zone(Zone::Battlefield)[index]
    }

    fn events(game: &GameState, batch: Vec<Event>) -> Vec<Event> {
        match process(game, batch, &[]).unwrap() {
            Processed::Events(events) => events,
            Processed::Ask { .. } => panic!("nothing is asked"),
        }
    }

    #[rstest]
    #[case::one(1)]
    #[case::five(5)]
    fn life_an_opponent_would_gain_is_lost(#[case] amount: u32) {
        let game = game(DRONE);
        let gained = Event::LifeGained {
            player: PlayerId(1),
            amount,
        };
        assert_eq!(
            events(&game, vec![gained]),
            [Event::LifeLost {
                player: PlayerId(1),
                amount
            }]
        );
        let mine = Event::LifeGained {
            player: PlayerId(0),
            amount,
        };
        assert_eq!(events(&game, vec![mine.clone()]), [mine]);
    }

    #[rstest]
    #[case::two(2)]
    #[case::three(3)]
    fn damage_to_the_controller_is_dealt_to_the_creature_by_the_same_source(#[case] amount: u32) {
        let game = game(REDIRECT);
        let bear = at(&game, 2);
        let wall = at(&game, 1);
        let dealt = events(
            &game,
            vec![Event::DamageDealt {
                source: bear,
                target: DamageTarget::Player(PlayerId(0)),
                amount,
                deathtouch: false,
                form: crate::combat::DamageForm::default(),
                toxic: 0,
            }],
        );
        assert!(
            matches!(&dealt[..], [Event::DamageDealt { source, target: DamageTarget::Object(to), amount: a, .. }]
                if *source == bear && *to == wall && *a == amount),
            "{dealt:?}"
        );
    }

    #[test]
    fn a_death_becomes_an_exile() {
        let game = game(EXILE);
        let bear = at(&game, 2);
        let died = Event::ZoneChanged {
            object: bear,
            from: Zone::Battlefield,
            to: Zone::Graveyard(PlayerId(1)),
            cause: ZoneCause::Destroyed,
        };
        let moved = events(&game, vec![died]);
        assert!(
            matches!(&moved[..], [Event::ZoneChanged { object, to: Zone::Exile, .. }] if *object == bear),
            "{moved:?}"
        );
    }

    #[test]
    fn steps_that_would_ask_leave_the_event_as_it_is() {
        let game = game(ASKS);
        let gained = Event::LifeGained {
            player: PlayerId(1),
            amount: 2,
        };
        assert_eq!(events(&game, vec![gained.clone()]), [gained]);
    }
}
