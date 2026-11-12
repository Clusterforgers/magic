//! `ReplaceBy::Instead` (CR 614.1a): effects that happen in place of the event. They read the event as a trigger reads
//! its cause: the player or object it is about (`Triggering`), its amount and the source of its damage (`Dealer`). A
//! question of the effects ("you may ... instead", a choice of one way) is asked of the player the effect names as
//! the step's own decision (CR 614.1, 616); a single "may" declined leaves the event as it is.

use mtg_cards::effect::Effect;
use mtg_cards::trigger::ReplaceBy;

use super::{Candidate, Item, Origin, Pass, Slot, Unit, slots_of};
use crate::abilities::Cause;
use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::counters::CountersChanged;
use crate::decision::Decision;
use crate::events::Event;
use crate::resolve::{Inline, w5d_run_instead};

/// What running the effects of a replacement came to for the pass.
pub(super) enum Ran {
    /// The events of the effects.
    Events(Vec<Event>),
    /// The event stays as it is: the effects were declined or cannot run.
    Kept,
    /// A player has to answer first.
    Ask(Decision),
}

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
            Unit::Life { player } | Unit::Draw { player } | Unit::Loss { player } => {
                (Some(Target::Player(*player)), None)
            }
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
            dealers: Vec::new(),
            subjects: subject.into_iter().collect(),
            snapshots: Vec::new(),
            amount,
            source,
            against: None,
        }
    }

    /// Runs the effects of `candidate` in place of the event at `at`, taking the answers of the batch's journal in
    /// order for the questions they ask.
    pub(super) fn w5d_run(
        &mut self,
        at: usize,
        candidate: &Candidate,
        effects: &[Effect],
        unit: &Unit,
    ) -> Ran {
        let cause = self.w5d_cause(at, unit);
        let state = self.state;
        let answers = &mut self.answers;
        match w5d_run_instead(
            state,
            candidate.controller,
            candidate.object,
            &cause,
            effects,
            &mut || answers.next().cloned(),
        ) {
            Ok(Inline::Events(events)) => Ran::Events(events),
            Ok(Inline::Ask(decision)) => Ran::Ask(decision),
            Ok(Inline::Declined) | Err(_) => Ran::Kept,
        }
    }

    /// The effects run in place of the event at `at`: their events take its place and carry the effects already applied
    /// to it, this one included, so it does not apply to them again (CR 614.5). Effects that are declined or cannot run
    /// leave the event as it is. `Plus` keeps the event and adds the tokens (`ep21_plus`); `PreventAnd` prevents the
    /// damage and adds the events (`w9c_prevent_and`).
    pub(super) fn w5d_instead(
        &mut self,
        at: usize,
        candidate: &Candidate,
        by: &ReplaceBy,
        effects: &[Effect],
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Option<Decision> {
        if matches!(by, ReplaceBy::Plus(_)) {
            return self.ep21_plus(at, candidate, effects, unit, applied);
        }
        if matches!(by, ReplaceBy::PreventAnd(_)) {
            return self.w9c_prevent_and(at, candidate, effects, unit, &applied);
        }
        if let ReplaceBy::PreventCountedAnd { most, .. } = by {
            let most = match most {
                mtg_cards::value::Amount::Fixed(most) => u32::try_from(*most).unwrap_or(0),
                _ => 0,
            };
            return self.r283_prevent_counted_and(at, candidate, most, effects, unit, &applied);
        }
        let events = match self.w5d_run(at, candidate, effects, unit) {
            Ran::Events(events) => events,
            Ran::Ask(decision) => return Some(decision),
            Ran::Kept => {
                self.slots[at].applied = applied;
                return None;
            }
        };
        // Damage redirected to a recipient that is gone is dealt as it was.
        if events.is_empty()
            && matches!(unit, Unit::Damage { .. })
            && effects
                .iter()
                .any(|effect| matches!(effect, Effect::DealDamage { .. }))
        {
            self.slots[at].applied = applied;
            return None;
        }
        self.end_once_shield(candidate);
        if matches!(unit, Unit::Draw { .. }) {
            self.draws_changed = true;
        }
        let events = self.w9c_fresh_tokens(events);
        let slots: Vec<_> = slots_of(events)
            .into_iter()
            .map(|mut slot| {
                slot.applied.clone_from(&applied);
                slot
            })
            .collect();
        self.slots.splice(at..=at, slots);
        None
    }

    /// The tokens of the event at `at` are created and the tokens the effects make join them: one event, so the
    /// replacements still to apply see them all (CR 614.5: this one does not apply again). Effects that cannot run
    /// leave the event as it is.
    pub(super) fn ep21_plus(
        &mut self,
        at: usize,
        candidate: &Candidate,
        effects: &[Effect],
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Option<Decision> {
        let Item::Tokens(run) = self.slots[at].item.clone() else {
            self.slots[at].applied = applied;
            return None;
        };
        let events = match self.w5d_run(at, candidate, effects, unit) {
            Ran::Events(events) => events,
            Ran::Ask(decision) => return Some(decision),
            Ran::Kept => {
                self.slots[at].applied = applied;
                return None;
            }
        };
        let controller = run[0].controller;
        let mut made = run;
        let mut rest = Vec::new();
        for event in self.w9c_fresh_tokens(events) {
            match event {
                Event::TokenCreated(token) if token.controller == controller => made.push(token),
                other => rest.push(other),
            }
        }
        let mut slots = vec![Slot {
            item: Item::Tokens(made),
            applied: applied.clone(),
            frozen: false,
        }];
        slots.extend(slots_of(rest).into_iter().map(|mut slot| {
            slot.applied.clone_from(&applied);
            slot
        }));
        self.slots.splice(at..=at, slots);
        None
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an effect that happens instead takes the place of the event and reads its player, its amount and the
    //! source of its damage (life gained becomes life lost, damage to a player is dealt to a creature by the same source,
    //! tokens become as many other tokens, a death becomes an exile), applies once to what replaces it, and asks its
    //! controller when its steps ask a question, a no leaving the event as it is.
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
                unpreventable: false,
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

    #[rstest]
    #[case::no_question_yet(&[], None)]
    #[case::declined(&[false], Some(false))]
    #[case::accepted(&[true], Some(true))]
    fn steps_that_ask_ask_the_controller_and_a_no_keeps_the_event(
        #[case] said: &[bool],
        #[case] replaced: Option<bool>,
    ) {
        let game = game(ASKS);
        let gained = Event::LifeGained {
            player: PlayerId(1),
            amount: 2,
        };
        let journal: Vec<_> = said
            .iter()
            .map(|yes| (PlayerId(0), crate::choose::Choice::YesNo(*yes)))
            .collect();
        match (
            process(&game, vec![gained.clone()], &journal).unwrap(),
            replaced,
        ) {
            (Processed::Ask { decision, .. }, None) => assert_eq!(decision.player, PlayerId(0)),
            (Processed::Events(events), Some(false)) => assert_eq!(events, [gained]),
            (Processed::Events(events), Some(true)) => assert!(
                matches!(&events[..], [Event::CardDrawn { player, .. }] if *player == PlayerId(0)),
                "{events:?}"
            ),
            (other, _) => panic!("{other:?}"),
        }
    }
}
