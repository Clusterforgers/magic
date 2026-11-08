//! Per-ability yields: a seat lets a triggered or activated ability resolve unasked for the rest of the game.
//!
//! A yield is keyed by what every player reads on the stack ([`AutoYield::of`]), never by a hidden card, and is held
//! with the stops ([`Stops::yields_to`]). At a priority decision of the seat, out of Full Control, an ability with a
//! held key on top of the stack is passed for at once, whatever the seat holds and wherever the window is: the pass
//! shows the seat's choice, as a stop does, and never its hand. Full Control asks anyway, and the yield waits until the
//! mode is off again.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::{Decision, DecisionKind, GameState};
use mtg_protocol::{AutoYield, ErrorCode, FullControlMode};

use crate::actor::{Fatal, GameActor, Phase};
use crate::running::Rejection;
use crate::stops::Stops;
use crate::store::EventStore;

/// Whether the table passes for the holder of `decision` because it yields to the ability on top of the stack.
pub(crate) fn passes_for(decision: &Decision, stops: &Stops, state: &GameState) -> bool {
    let seat = decision.player;
    matches!(decision.kind, DecisionKind::Priority(_))
        && stops.full_control(seat) == FullControlMode::Off
        && state
            .stack()
            .last()
            .and_then(|top| AutoYield::of(state, *top))
            .is_some_and(|key| stops.yields_to(seat, &key))
}

impl<S: EventStore> GameActor<S> {
    /// Lets the ability `object` is on the stack, and every later one with its key, resolve unasked for `seat`.
    ///
    /// Anything but an ability on the stack whose source every player sees is refused with `Error{BadMessage}`. A
    /// seat that faces a priority decision the yield now passes is passed at once, as a unit with the new yield, like
    /// [`Self::set_yield`].
    pub(crate) async fn set_auto_yield(
        &mut self,
        seat: PlayerId,
        object: ObjectId,
    ) -> Result<(), Fatal> {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return Ok(());
        }
        let Phase::Running(running) = &mut self.phase else {
            let reason = "the game has not started, so nothing is on the stack";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        };
        let Some(ability) = AutoYield::of(running.engine.state(), object) else {
            let reason =
                "only an ability on the stack whose source every player sees can be yielded to";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        };
        let mut stops = self.stops.clone();
        stops.set_auto_yield(seat, ability, true);
        let facing = running.asks_priority_of(seat);
        if !(facing && running.passes_pending(seat, &stops, &self.yields)) {
            return self.take_stops(seat, stops).await;
        }
        match running.restart_decision(seat) {
            Ok(step) => {
                let yields = self.yields.clone();
                self.publish_with(step, yields, stops).await
            }
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Takes back `seat`'s yield to `ability`; one it does not hold changes nothing and is answered with silence.
    pub(crate) async fn clear_auto_yield(
        &mut self,
        seat: PlayerId,
        ability: AutoYield,
    ) -> Result<(), Fatal> {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return Ok(());
        }
        let mut stops = self.stops.clone();
        stops.set_auto_yield(seat, ability, false);
        self.take_stops(seat, stops).await
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the table passes for a seat exactly when it is out of Full Control at a priority decision and yields to the key of the ability on top of the stack; another seat's yield, a key that differs, an ability under the top, a spell and a question that is not priority are asked as before.
    use super::*;
    use mtg_engine::choose::YesNoOptions;
    use mtg_testkit::cards::{burn, mountain, pinger};
    use mtg_testkit::{Driver, Scenario, ready};
    use rstest::rstest;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// What lies on the stack, bottom first.
    #[derive(Debug, Clone, Copy)]
    enum Pile {
        Ability,
        Spell,
        SpellOnAbility,
        AbilityOnSpell,
    }

    /// A duel in which seat 0 puts `pile` on the stack: a Pinger activation and a Bolt.
    fn duel(pile: Pile) -> Driver {
        let mut driver =
            Driver::from_scenario(Scenario::new(2).hand(0, [burn("Bolt", 1)]).battlefield(
                0,
                [
                    ready(pinger("Pinger", 1)),
                    ready(mountain()),
                    ready(mountain()),
                ],
            ))
            .unwrap();
        let at = driver.target_player(1);
        let ping = |driver: &mut Driver| {
            driver.begin_activation("Pinger").unwrap();
            driver.choose_targets(&[at]).unwrap();
        };
        match pile {
            Pile::Ability => ping(&mut driver),
            Pile::Spell => {
                driver.cast_at("Bolt", &[at]).unwrap();
            }
            Pile::SpellOnAbility => {
                ping(&mut driver);
                driver.cast_at("Bolt", &[at]).unwrap();
            }
            Pile::AbilityOnSpell => {
                driver.cast_at("Bolt", &[at]).unwrap();
                ping(&mut driver);
            }
        }
        driver
    }

    /// A priority decision of `seat`, or a question that is not one.
    /// The priority decision `driver` waits on, given to `seat`, or a question to `seat` that is not priority.
    fn decision(driver: &Driver, seat: PlayerId, priority: bool) -> Decision {
        let pending = driver.engine().decision().unwrap();
        assert!(matches!(pending.kind, DecisionKind::Priority(_)));
        let kind = if priority {
            pending.kind.clone()
        } else {
            DecisionKind::YesNo(YesNoOptions::new(ObjectId(1), "Pay {1}?"))
        };
        Decision { player: seat, kind }
    }

    /// The key of the first ability on the stack of `driver`.
    fn pinger_key(driver: &Driver) -> AutoYield {
        let state = driver.state();
        let ability = state
            .stack()
            .iter()
            .find_map(|id| AutoYield::of(state, *id));
        ability.unwrap()
    }

    #[rstest]
    #[case::the_ability_on_top(Pile::Ability, ME, ME, true, FullControlMode::Off, true)]
    #[case::an_opponents_ability_on_top(Pile::Ability, FOE, FOE, true, FullControlMode::Off, true)]
    #[case::another_seats_yield(Pile::Ability, FOE, ME, true, FullControlMode::Off, false)]
    #[case::full_control_asks(Pile::Ability, ME, ME, true, FullControlMode::Held, false)]
    #[case::the_next_window_asks(Pile::Ability, ME, ME, true, FullControlMode::NextWindow, false)]
    #[case::not_a_priority_decision(Pile::Ability, ME, ME, false, FullControlMode::Off, false)]
    #[case::a_spell_on_top(Pile::Spell, ME, ME, true, FullControlMode::Off, false)]
    #[case::a_spell_over_the_ability(
        Pile::SpellOnAbility,
        ME,
        ME,
        true,
        FullControlMode::Off,
        false
    )]
    #[case::the_ability_over_a_spell(
        Pile::AbilityOnSpell,
        ME,
        ME,
        true,
        FullControlMode::Off,
        true
    )]
    fn a_yield_passes_for_its_seat_only_at_its_ability_on_top_out_of_full_control(
        #[case] pile: Pile,
        #[case] yielder: PlayerId,
        #[case] asked: PlayerId,
        #[case] priority: bool,
        #[case] mode: FullControlMode,
        #[case] passes: bool,
    ) {
        let driver = duel(pile);
        let mut stops = Stops::new(2);
        if let Some(ability) = driver
            .state()
            .stack()
            .iter()
            .find_map(|id| AutoYield::of(driver.state(), *id))
        {
            stops.set_auto_yield(yielder, ability, true);
        }
        stops.set_full_control(asked, mode);
        let decision = decision(&driver, asked, priority);
        assert_eq!(passes_for(&decision, &stops, driver.state()), passes);
    }

    #[rstest]
    #[case::another_name("Prodder", true)]
    #[case::other_words("Pinger", false)]
    fn a_key_that_differs_does_not_pass(#[case] name: &str, #[case] same_text: bool) {
        let driver = duel(Pile::Ability);
        let held = pinger_key(&driver);
        let key = AutoYield {
            name: name.to_owned(),
            text: if same_text {
                held.text
            } else {
                "Draw a card.".to_owned()
            },
        };
        let mut stops = Stops::new(2);
        stops.set_auto_yield(ME, key, true);
        assert!(!passes_for(
            &decision(&driver, ME, true),
            &stops,
            driver.state()
        ));
    }

    #[test]
    fn a_yield_taken_back_passes_no_more_and_the_seat_is_told_each_change() {
        let driver = duel(Pile::Ability);
        let key = pinger_key(&driver);
        let mut stops = Stops::new(2);
        let mut added = stops.clone();
        added.set_auto_yield(ME, key.clone(), true);
        let told = stops.replace(added);
        assert_eq!(
            told,
            vec![(
                ME,
                mtg_protocol::ServerMessage::AutoYields {
                    yields: vec![key.clone()]
                }
            )]
        );
        assert!(passes_for(
            &decision(&driver, ME, true),
            &stops,
            driver.state()
        ));
        let mut again = stops.clone();
        again.set_auto_yield(ME, key.clone(), true);
        assert_eq!(stops.replace(again), Vec::new());
        let mut cleared = stops.clone();
        cleared.set_auto_yield(ME, key, false);
        assert_eq!(
            stops.replace(cleared),
            vec![(
                ME,
                mtg_protocol::ServerMessage::AutoYields { yields: Vec::new() }
            )]
        );
        assert!(!passes_for(
            &decision(&driver, ME, true),
            &stops,
            driver.state()
        ));
    }
}
