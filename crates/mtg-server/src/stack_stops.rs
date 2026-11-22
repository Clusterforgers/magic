//! Stack stops: a seat chooses which spells and abilities on top of the stack it is asked at.
//!
//! [`passes_for`] reads the seat's [`StackStops`] and the object on top of the stack: who controls it and what it
//! targets, which every player sees. It never reads the seat's hand or what the seat could do, so two seats with one
//! setting are passed for at the same objects whatever they hold, and the pass at once shows the setting alone, as a
//! yield does. An object the setting asks at goes on to the windows ([`crate::windows`]): asked when the seat can act,
//! passed for after the dwell when it cannot.
//! - `All` passes nothing here.
//! - `Opponents` passes an object of the seat or of a teammate.
//! - `Aimed` also passes an opponent's object unless it targets the seat, a teammate or an object one of them controls
//!   or owns (a permanent, a spell or ability on the stack, a card in a graveyard).
//!
//! Full Control asks anyway, and so does a seat that owes the table clauses by hand ([`Stops::by_hand`]).

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::Target;
use mtg_engine::{Decision, DecisionKind, GameState};
use mtg_protocol::{FullControlMode, StackStops};

use crate::stops::Stops;

/// Whether the table passes for the holder of `decision` at once because its stack stops leave out the object on top.
pub(crate) fn passes_for(decision: &Decision, stops: &Stops, state: &GameState) -> bool {
    let seat = decision.player;
    let level = stops.auto_choices(seat).stack_stops;
    let Some(top) = state.stack().last().copied() else {
        return false;
    };
    level != StackStops::All
        && matches!(decision.kind, DecisionKind::Priority(_))
        && stops.full_control(seat) == FullControlMode::Off
        && !stops.by_hand(seat)
        && !asks_at(level, state, seat, top)
}

/// Whether `level` asks `seat` at the stack object `top`.
fn asks_at(level: StackStops, state: &GameState, seat: PlayerId, top: ObjectId) -> bool {
    let Some(object) = state.object(top) else {
        return true;
    };
    let theirs = state.is_opponent(seat, object.controller());
    match level {
        StackStops::All => true,
        StackStops::Opponents => theirs,
        StackStops::Aimed => {
            let aimed = object.choices().targets.iter().flatten();
            theirs && aimed.into_iter().any(|target| is_ours(state, seat, *target))
        }
    }
}

/// Whether `target` is `seat`, a teammate or an object one of them controls or owns.
fn is_ours(state: &GameState, seat: PlayerId, target: Target) -> bool {
    match target {
        Target::Player(player) => state.are_teammates(seat, player),
        Target::Object(reference) => state.object(reference.id).is_some_and(|object| {
            state.are_teammates(seat, object.controller())
                || state.are_teammates(seat, object.owner())
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: for every stack scenario (whose object is on top, what it targets) and every setting, the table passes at once exactly where the setting leaves the object out, from public facts alone: the same for a seat that can act and one that cannot, never in Full Control, at a by-hand window, over an empty stack or at a question that is not priority.
    use super::*;
    use mtg_core::house::HouseRules;
    use mtg_core::teams::{TeamMode, Teams};
    use mtg_engine::Event;
    use mtg_engine::choose::YesNoOptions;
    use mtg_engine::turn::Step;
    use mtg_protocol::AutoChoices;
    use mtg_testkit::cards::{burn, free_creature, island, mountain, pinger, study};
    use mtg_testkit::{Driver, Scenario, ready};
    use rstest::rstest;

    /// The seat asked: it plays with seat 3 against seats 0 and 1.
    const ME: usize = 2;
    const MATE: usize = 3;
    const FOE: usize = 0;
    const OTHER_FOE: usize = 1;

    /// What the object on top of the stack is aimed at.
    #[derive(Debug, Clone, Copy)]
    enum Aim {
        /// A player.
        Seat(usize),
        /// The Bear of a seat.
        Bear(usize),
        /// Nothing: a spell without targets.
        Nothing,
    }

    /// How the object on top got there.
    #[derive(Debug, Clone, Copy)]
    enum Top {
        /// A Bolt cast by a seat.
        Spell(usize, Aim),
        /// A Pinger activated by a seat.
        Ability(usize, Aim),
        /// Nothing is on the stack.
        Empty,
    }

    fn bear_name(seat: usize) -> String {
        format!("Bear {seat}")
    }

    /// Four seats in two teams (0 and 1 against 2 and 3), each with a Bear, in the main phase of the seat that puts
    /// `top` on the stack.
    fn table(top: Top) -> Driver {
        let caster = match top {
            Top::Spell(seat, _) | Top::Ability(seat, _) => seat,
            Top::Empty => FOE,
        };
        let teams = Teams {
            mode: TeamMode::Teams,
            seats: vec![0, 0, 1, 1],
        };
        let mut scenario = Scenario::new(4)
            .house_rules(HouseRules {
                teams: Some(teams),
                ..HouseRules::default()
            })
            .at(1, caster, Step::Main1)
            .hand(caster, [burn("Bolt", 1), study("Study")]);
        for seat in 0..4 {
            let mut permanents = vec![ready(free_creature(&bear_name(seat), 2, 2))];
            if seat == caster {
                permanents.extend([pinger("Pinger", 1), mountain(), island()].map(ready));
            }
            scenario = scenario.battlefield(seat, permanents);
        }
        let mut driver = Driver::from_scenario(scenario).unwrap();
        let target = |driver: &Driver, aim: Aim| match aim {
            Aim::Seat(seat) => Some(driver.target_player(seat)),
            Aim::Bear(seat) => Some(driver.target_named(&bear_name(seat)).unwrap()),
            Aim::Nothing => None,
        };
        match top {
            Top::Spell(_, aim) => match target(&driver, aim) {
                Some(at) => drop(driver.cast_at("Bolt", &[at]).unwrap()),
                None => drop(driver.cast("Study").unwrap()),
            },
            Top::Ability(_, aim) => {
                let at = target(&driver, aim).unwrap();
                driver.begin_activation("Pinger").unwrap();
                driver.choose_targets(&[at]).unwrap();
            }
            Top::Empty => {}
        }
        driver
    }

    /// The priority decision `driver` waits on, given to seat `ME` and told whether it can act.
    fn priority(driver: &Driver, can_act: bool) -> Decision {
        let mut kind = driver.engine().decision().unwrap().kind.clone();
        let DecisionKind::Priority(options) = &mut kind else {
            panic!("the table waits on priority");
        };
        options.can_act = can_act;
        Decision {
            player: PlayerId::from_index(ME),
            kind,
        }
    }

    fn holding(level: StackStops) -> Stops {
        let mut stops = Stops::new(4);
        let choices = AutoChoices {
            stack_stops: level,
            ..AutoChoices::default()
        };
        stops.set_auto_choices(PlayerId::from_index(ME), choices);
        stops
    }

    /// Each row: the stack, then whether `Opponents` and `Aimed` pass for the seat there. `All` never passes.
    #[rstest]
    #[case::my_spell_at_a_foe(Top::Spell(ME, Aim::Seat(FOE)), true, true)]
    #[case::my_spell_at_myself(Top::Spell(ME, Aim::Seat(ME)), true, true)]
    #[case::my_spell_without_targets(Top::Spell(ME, Aim::Nothing), true, true)]
    #[case::my_ability(Top::Ability(ME, Aim::Bear(FOE)), true, true)]
    #[case::a_teammates_spell_at_me(Top::Spell(MATE, Aim::Seat(ME)), true, true)]
    #[case::a_teammates_ability_at_a_foe(Top::Ability(MATE, Aim::Seat(FOE)), true, true)]
    #[case::a_foes_spell_at_me(Top::Spell(FOE, Aim::Seat(ME)), false, false)]
    #[case::a_foes_spell_at_my_teammate(Top::Spell(FOE, Aim::Seat(MATE)), false, false)]
    #[case::a_foes_spell_at_my_creature(Top::Spell(FOE, Aim::Bear(ME)), false, false)]
    #[case::a_foes_spell_at_my_teammates_creature(Top::Spell(FOE, Aim::Bear(MATE)), false, false)]
    #[case::a_foes_ability_at_me(Top::Ability(FOE, Aim::Seat(ME)), false, false)]
    #[case::a_foes_ability_at_my_creature(Top::Ability(OTHER_FOE, Aim::Bear(ME)), false, false)]
    #[case::a_foes_spell_at_its_teammate(Top::Spell(FOE, Aim::Seat(OTHER_FOE)), false, true)]
    #[case::a_foes_spell_at_its_own_creature(Top::Spell(FOE, Aim::Bear(FOE)), false, true)]
    #[case::a_foes_spell_without_targets(Top::Spell(OTHER_FOE, Aim::Nothing), false, true)]
    #[case::a_foes_ability_at_a_foe(Top::Ability(FOE, Aim::Bear(OTHER_FOE)), false, true)]
    #[case::an_empty_stack(Top::Empty, false, false)]
    fn a_setting_passes_exactly_the_objects_it_leaves_out(
        #[case] top: Top,
        #[case] opponents: bool,
        #[case] aimed: bool,
    ) {
        let driver = table(top);
        let state = driver.state();
        let levels = [
            (StackStops::All, false),
            (StackStops::Opponents, opponents),
            (StackStops::Aimed, aimed),
        ];
        for (level, passes) in levels {
            let stops = holding(level);
            // What the seat could do never moves the verdict: the pace is the same for a full hand and an empty one.
            for can_act in [true, false] {
                let decision = priority(&driver, can_act);
                assert_eq!(
                    passes_for(&decision, &stops, state),
                    passes,
                    "{level:?}, can act: {can_act}"
                );
            }
        }
    }

    #[rstest]
    fn full_control_a_by_hand_window_and_other_questions_are_asked(
        #[values(StackStops::Opponents, StackStops::Aimed)] level: StackStops,
    ) {
        let me = PlayerId::from_index(ME);
        let driver = table(Top::Spell(ME, Aim::Seat(FOE)));
        let state = driver.state();
        let decision = priority(&driver, true);
        assert!(passes_for(&decision, &holding(level), state));

        for mode in [FullControlMode::NextWindow, FullControlMode::Held] {
            let mut stops = holding(level);
            stops.set_full_control(me, mode);
            assert!(!passes_for(&decision, &stops, state), "{mode:?}");
        }

        let mut owing = holding(level);
        let resolved = ObjectId(9_999);
        owing.resolved(&[(resolved, me)], &[Event::SpellResolved { object: resolved }]);
        assert!(owing.by_hand(me));
        assert!(!passes_for(&decision, &owing, state));

        let question = Decision {
            player: me,
            kind: DecisionKind::YesNo(YesNoOptions::new(ObjectId(1), "Pay {1}?")),
        };
        assert!(!passes_for(&question, &holding(level), state));
    }

    #[test]
    fn another_seats_setting_passes_nothing_for_the_seat() {
        let driver = table(Top::Spell(ME, Aim::Seat(FOE)));
        let mut stops = Stops::new(4);
        let choices = AutoChoices {
            stack_stops: StackStops::Aimed,
            ..AutoChoices::default()
        };
        stops.set_auto_choices(PlayerId::from_index(MATE), choices);
        assert!(!passes_for(&priority(&driver, true), &stops, driver.state()));
    }
}
