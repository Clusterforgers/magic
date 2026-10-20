//! Goal: payments an effect offers play whole at tables of two, three and four seats, as their cards say: Izzet Charm counters a noncreature spell unless its controller pays {2} (yes, no, cannot pay), deals 2 damage to a creature and loots; Rhystic Study asks the opponent who cast a spell, whichever seat that is, to pay {1} and only then lets its controller draw; a seat that is not the active one decides; a payment may use several sources, a partial choice finished by the engine and floating mana; an effect that names several opponents asks them in turn order until one pays, skipping those who cannot; a player is never asked what they cannot pay; the payment cannot be backed out of; an engine rebuilt from the log asks the same question at every decision, including the payments, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{name_of, names, pid};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::choose::{CardsChoice, ModeChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::mana::Payment;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{forest, island, mountain, plains};
use mtg_testkit::pay_cards::{leak, tithe, toll};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn yes_no(yes: bool) -> Answer {
    Answer::YesNo(YesNoChoice(yes))
}

/// Passes priority while something is on the stack and the holder is asked only for it.
fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

/// Passes priority until `seat` holds it with nothing asked of anyone else.
fn pass_until_held_by(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(seat)
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn on_stack(driver: &Driver, name: &str) -> Target {
    let id = driver
        .state()
        .stack()
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap();
    Target::Object(driver.state().object(id).unwrap().reference())
}

/// The question the table waits on: who answers it and what kind it is.
fn waiting(driver: &Driver) -> (PlayerId, &'static str) {
    let decision = driver.decision().unwrap();
    (decision.player, decision.kind.name())
}

fn question(driver: &Driver) -> String {
    match &driver.decision().unwrap().kind {
        DecisionKind::YesNo(options) => options.text.clone(),
        other => panic!("a yes-or-no question was expected, not {}", other.name()),
    }
}

/// Seat 0 holds Izzet Charm with an Island and a Mountain; seat 1 holds a Lightning Bolt (aimed at seat 0) with a
/// Mountain for it and `spare` Forests to pay with; the other seats have nothing. Seat 0 passes, seat 1 casts the Bolt
/// and seat 0 holds priority with it on the stack.
fn bolt_on_the_stack(seats: usize, spare: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Izzet Charm"])
            .battlefield(0, [ready(island()), ready(mountain())])
            .hand_named(1, ["Lightning Bolt", "Grizzly Bears"])
            .battlefield(
                1,
                [vec![ready(mountain())], vec![ready(forest()); spare]].concat(),
            ),
    )?;
    driver.pass()?;
    let target = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[target])?;
    pass_until_held_by(&mut driver, 0)?;
    Ok(driver)
}

/// Seat 0 casts Izzet Charm in its counter mode at the Bolt.
fn charm_the_bolt(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_cast("Izzet Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))?;
    let bolt = on_stack(driver, "Lightning Bolt");
    driver.choose_targets(&[bolt])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    passes_until_asked(driver)
}

#[rstest]
fn izzet_charm_counters_a_spell_unless_its_controller_pays_two(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = bolt_on_the_stack(seats, 2)?;
    charm_the_bolt(&mut driver)?;
    // The controller of the Bolt decides, though it is not their turn.
    assert_eq!(waiting(&driver), (pid(1), "yes-no"));
    assert_eq!(question(&driver), "Pay {2}?");
    driver.answer(yes_no(pays))?;
    if pays {
        assert_eq!(waiting(&driver), (pid(1), "pay-mana"));
        let options = driver.pay_options()?;
        assert_eq!(options.cost.to_string(), "{2}");
        assert!(!options.can_cancel && options.auto_available);
        assert_eq!(
            options.sources.len(),
            2,
            "the Mountain was tapped for the Bolt"
        );
        driver.pay_tapping(&["Forest", "Forest"], 0)?;
    }
    passes_until_asked(&mut driver)?;
    let state = driver.state();
    assert_eq!(names(state, Zone::Graveyard(pid(0))), ["Izzet Charm"]);
    if pays {
        assert_eq!(life(&driver, 0), 17, "the Bolt resolved");
        assert_eq!(names(state, Zone::Graveyard(pid(1))), ["Lightning Bolt"]);
        let tapped = |name: &str| {
            state
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| name_of(state, **id) == name)
                .filter(|id| state.object(**id).unwrap().permanent().unwrap().tapped())
                .count()
        };
        assert_eq!(tapped("Forest"), 2);
    } else {
        assert_eq!(life(&driver, 0), 20, "the Bolt was countered");
        assert_eq!(names(state, Zone::Graveyard(pid(1))), ["Lightning Bolt"]);
    }
    let countered = driver.engine().events().iter().any(|event| {
        matches!(
            event,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Countered,
                ..
            }
        )
    });
    assert_eq!(countered, !pays);
    Ok(())
}

#[rstest]
fn a_player_who_cannot_pay_is_not_asked_and_the_spell_is_countered(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] spare: usize,
) -> Result<(), ScriptError> {
    let mut driver = bolt_on_the_stack(seats, spare)?;
    charm_the_bolt(&mut driver)?;
    assert_eq!(
        life(&driver, 0),
        20,
        "the Bolt was countered without a word"
    );
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack().len(), 0);
    Ok(())
}

#[test]
fn a_payment_the_player_said_yes_to_cannot_be_backed_out_of_or_paid_wrongly()
-> Result<(), ScriptError> {
    let mut driver = bolt_on_the_stack(2, 2)?;
    charm_the_bolt(&mut driver)?;
    driver.answer(yes_no(true))?;
    let log = driver.engine().events().len();
    let refused = driver
        .answer(Answer::PayMana(PayChoice::Cancel))
        .unwrap_err();
    assert!(
        matches!(
            refused,
            ScriptError::Engine(EngineError::Illegal(IllegalAnswer::NotOffered("cancel")))
        ),
        "{refused:?}"
    );
    // One land is not the two mana the payment asks for, and another player's land is not offered.
    assert!(driver.pay_tapping(&["Forest"], 0).is_err());
    assert!(driver.pay_tapping(&["Island"], 0).is_err());
    assert_eq!(driver.engine().events().len(), log, "nothing was logged");
    assert_eq!(waiting(&driver), (pid(1), "pay-mana"));
    Ok(())
}

#[rstest]
fn izzet_charm_cannot_counter_a_creature_spell_and_the_other_modes_do_what_the_card_says(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Grizzly Bears are on the stack: no spell is a legal target for the counter mode, and there is no creature to
    // damage yet, so only the loot is offered.
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Izzet Charm", "Grizzly Bears"])
            .battlefield(0, [island(), mountain(), forest(), forest()].map(ready)),
    )?;
    driver.cast_tapping("Grizzly Bears", &["Forest", "Forest"])?;
    driver.begin_cast("Izzet Charm")?;
    let DecisionKind::ChooseMode(options) = &driver.decision()?.kind else {
        panic!("the modes were expected");
    };
    let offered: Vec<usize> = options.modes.iter().map(|mode| mode.index).collect();
    assert_eq!(offered, [2], "only the loot is possible");
    driver.answer(Answer::ChooseMode(ModeChoice::Cancel))?;

    // The Bears resolve; now the damage mode has a creature to hit and the counter mode has no spell.
    passes_until_asked(&mut driver)?;
    pass_until_held_by(&mut driver, 0)?;
    driver.begin_cast("Izzet Charm")?;
    let DecisionKind::ChooseMode(options) = &driver.decision()?.kind else {
        panic!("the modes were expected");
    };
    let offered: Vec<usize> = options.modes.iter().map(|mode| mode.index).collect();
    assert_eq!(offered, [1, 2]);
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Izzet Charm", "Grizzly Bears"],
        "the creature died to 2 damage after the spell was done"
    );
    Ok(())
}

#[rstest]
fn izzet_charm_draws_two_cards_and_then_discards_two(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Izzet Charm", "Lightning Bolt", "Giant Growth"])
            .library_named(0, ["Opt", "Brainstorm", "Doom Blade"])
            .battlefield(0, [ready(island()), ready(mountain())]),
    )?;
    driver.begin_cast("Izzet Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![2])))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    passes_until_asked(&mut driver)?;
    // Two cards came in, then the controller chooses the two to discard from four.
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(0));
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a discard was expected, not {}", decision.kind.name());
    };
    assert_eq!((options.from.len(), options.min, options.max), (4, 2, 2));
    let wanted: Vec<ObjectId> = options
        .from
        .iter()
        .copied()
        .filter(|id| ["Opt", "Brainstorm"].contains(&name_of(driver.state(), *id).as_str()))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(wanted)))?;
    passes_until_asked(&mut driver)?;
    let mut hand = names(driver.state(), Zone::Hand(pid(0)));
    hand.sort();
    assert_eq!(hand, ["Giant Growth", "Lightning Bolt"]);
    Ok(())
}

/// Seat `owner` controls Rhystic Study; seat `caster` casts a Lightning Bolt at it with a Mountain and `spare` Forests;
/// the trigger waits on the stack above the Bolt, held by the active seat 0 (the owner or not).
fn rhystic_trigger(
    seats: usize,
    owner: usize,
    caster: usize,
    spare: usize,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield_named(owner, ["Rhystic Study"])
        .library_named(owner, ["Opt", "Brainstorm", "Doom Blade"]);
    let mut lands = vec![ready(mountain())];
    lands.extend(vec![ready(forest()); spare]);
    scenario = scenario.hand_named(caster, ["Lightning Bolt"]);
    scenario = scenario.battlefield(caster, lands);
    let mut driver = Driver::from_scenario(scenario)?;
    pass_until_held_by(&mut driver, caster)?;
    let target = driver.target_player(owner);
    driver.cast_at("Lightning Bolt", &[target])?;
    pass_until_held_by(&mut driver, 0)?;
    assert_eq!(
        driver.state().stack().len(),
        2,
        "the Bolt and the trigger it set off"
    );
    passes_until_asked(&mut driver)?;
    Ok(driver)
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats_the_next_seat(3, 1)]
#[case::three_seats_the_last_seat(3, 2)]
#[case::four_seats_the_next_seat(4, 1)]
#[case::four_seats_the_third_seat(4, 2)]
#[case::four_seats_the_last_seat(4, 3)]
fn rhystic_study_lets_its_controller_draw_unless_the_opponent_who_cast_pays_one(
    #[case] seats: usize,
    #[case] caster: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = rhystic_trigger(seats, 0, caster, 1)?;
    // Only the opponent who cast is asked, though it is another seat's turn.
    assert_eq!(waiting(&driver), (pid(caster), "yes-no"));
    assert_eq!(question(&driver), "Pay {1}?");
    driver.answer(yes_no(pays))?;
    if pays {
        assert_eq!(waiting(&driver), (pid(caster), "pay-mana"));
        driver.pay_tapping(&["Forest"], 0)?;
        passes_until_asked(&mut driver)?;
        assert_eq!(
            names(driver.state(), Zone::Hand(pid(0))),
            Vec::<String>::new()
        );
    } else {
        // The controller may draw: asked in the words of the card.
        assert_eq!(waiting(&driver), (pid(0), "yes-no"));
        assert_eq!(question(&driver), "Draw a card?");
        driver.answer(yes_no(true))?;
        passes_until_asked(&mut driver)?;
        assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Opt"]);
    }
    assert_eq!(life(&driver, 0), 17, "the Bolt resolved after the trigger");
    Ok(())
}

#[rstest]
fn rhystic_study_draws_only_when_its_controller_wants_to_and_the_opponent_cannot_pay(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] draws: bool,
) -> Result<(), ScriptError> {
    // The opponent has no land left after the Bolt, so is not asked: the controller is asked at once.
    let mut driver = rhystic_trigger(seats, 0, 1, 0)?;
    assert_eq!(waiting(&driver), (pid(0), "yes-no"));
    assert_eq!(question(&driver), "Draw a card?");
    driver.answer(yes_no(draws))?;
    passes_until_asked(&mut driver)?;
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(
        hand,
        if draws {
            vec!["Opt"]
        } else {
            Vec::<&str>::new()
        }
    );
    Ok(())
}

#[rstest]
fn the_active_seat_pays_when_it_is_the_one_who_cast_and_the_study_belongs_to_another(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    // Seat 1 controls the Study; seat 0 (active) casts the Bolt at it.
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield_named(1, ["Rhystic Study"])
            .library_named(1, ["Opt", "Brainstorm"])
            .hand_named(0, ["Lightning Bolt"])
            .battlefield(0, [ready(mountain()), ready(forest())]),
    )?;
    let target = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[target])?;
    passes_until_asked(&mut driver)?;
    assert_eq!(waiting(&driver), (pid(0), "yes-no"));
    driver.answer(yes_no(pays))?;
    if pays {
        driver.pay_tapping(&["Forest"], 0)?;
    } else {
        assert_eq!(
            waiting(&driver),
            (pid(1), "yes-no"),
            "its controller may draw"
        );
        driver.answer(yes_no(true))?;
    }
    passes_until_asked(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(1))),
        if pays { vec![] } else { vec!["Opt"] }
    );
    Ok(())
}

#[test]
fn the_controller_of_the_study_casting_a_spell_is_not_asked_anything() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .battlefield_named(0, ["Rhystic Study"])
            .hand_named(0, ["Lightning Bolt"])
            .battlefield(0, [ready(mountain())])
            .battlefield(1, [ready(forest())]),
    )?;
    let target = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[target])?;
    assert_eq!(driver.state().stack().len(), 1, "no trigger");
    passes_until_asked(&mut driver)?;
    assert_eq!(life(&driver, 1), 17);
    Ok(())
}

#[rstest]
fn a_payment_may_use_several_sources_a_partial_choice_the_engine_finishes_and_floating_mana(
    #[values(2, 3, 4)] seats: usize,
    #[values("all by hand", "completed", "floating")] how: &str,
) -> Result<(), ScriptError> {
    // The Bolt's controller (seat 0) holds a Mountain and three Plains to pay the Leak's {3}.
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Lightning Bolt"])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(plains()),
                    ready(plains()),
                    ready(forest()),
                    ready(island()),
                ],
            )
            .hand(1, [leak("Leak", 3)])
            .battlefield(1, [ready(island()), ready(island())]),
    )?;
    let target = driver.target_player(1);
    if how == "floating" {
        // One Plains is tapped ahead of time: its mana floats and is spent first.
        driver.tap_for_mana("Plains")?;
    }
    driver.cast_at("Lightning Bolt", &[target])?;
    pass_until_held_by(&mut driver, 1)?;
    driver.begin_cast("Leak")?;
    let bolt = on_stack(&driver, "Lightning Bolt");
    driver.choose_targets(&[bolt])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(waiting(&driver), (pid(0), "yes-no"));
    assert_eq!(question(&driver), "Pay {3}?");
    driver.answer(yes_no(true))?;
    let options = driver.pay_options()?.clone();
    assert!(!options.can_cancel);
    match how {
        "all by hand" => {
            driver.pay_tapping(&["Plains", "Plains", "Forest"], 0)?;
        }
        "completed" => {
            // One Plains by hand; the engine pays the other two.
            let plains_source = options
                .sources
                .iter()
                .find(|o| name_of(driver.state(), o.source.object) == "Plains")
                .unwrap()
                .source;
            driver.answer(Answer::PayMana(PayChoice::Complete(Payment {
                taps: vec![plains_source],
                ..Payment::default()
            })))?;
        }
        _ => {
            assert_eq!(options.pool.total(), 1, "the floating mana is offered");
            driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
                pool: vec![ManaType::White],
                taps: options
                    .sources
                    .iter()
                    .filter(|o| name_of(driver.state(), o.source.object) != "Plains")
                    .map(|o| o.source)
                    .take(2)
                    .collect(),
                life: 0,
            })))?;
        }
    }
    passes_until_asked(&mut driver)?;
    assert_eq!(life(&driver, 1), 17, "the Bolt was paid for and resolved");
    let state = driver.state();
    let untapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).unwrap().controller() == pid(0))
        .filter(|id| !state.object(**id).unwrap().permanent().unwrap().tapped())
        .count();
    // Five lands: the Mountain for the Bolt and three sources for the Leak; one is left.
    assert_eq!(untapped, 1, "{how}");
    Ok(())
}

#[rstest]
fn an_effect_that_names_the_opponents_asks_them_in_turn_order_skipping_those_who_cannot_pay_until_one_does(
    #[values(3, 4)] seats: usize,
    #[values("nobody pays", "the first able pays", "the last pays")] how: &str,
) -> Result<(), ScriptError> {
    // "Unless an opponent pays {1}, you draw a card; if one does, you gain 1 life." Seat 1 has no land, so is never
    // asked; the seats after it have one Forest each.
    let mut scenario = Scenario::new(seats)
        .hand(0, [toll("Toll")])
        .battlefield(0, [ready(island())]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(forest())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Toll")?;
    passes_until_asked(&mut driver)?;
    let able: Vec<usize> = (2..seats).collect();
    let payer = match how {
        "nobody pays" => None,
        "the first able pays" => able.first().copied(),
        _ => able.last().copied(),
    };
    for &seat in &able {
        assert_eq!(waiting(&driver), (pid(seat), "yes-no"), "{how}");
        assert_eq!(question(&driver), "Pay {1}?");
        if payer == Some(seat) {
            driver.answer(yes_no(true))?;
            driver.pay_tapping(&["Forest"], 0)?;
            break;
        }
        driver.answer(yes_no(false))?;
    }
    passes_until_asked(&mut driver)?;
    // The paid effect gains a life; the unpaid one draws a card. Seats after the one who paid were never asked.
    let (life_of_seat_0, drawn) = if payer.is_some() { (21, 0) } else { (20, 1) };
    assert_eq!(life(&driver, 0), life_of_seat_0, "{how}");
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len(),
        drawn,
        "{how}"
    );
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    Ok(())
}

#[rstest]
fn a_may_pay_needs_the_colours_and_pays_with_the_right_sources(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] white: bool,
) -> Result<(), ScriptError> {
    // "You may pay {1}{W}": without a white source the controller is not asked.
    let lands = if white {
        vec![ready(plains()), ready(plains()), ready(forest())]
    } else {
        vec![ready(forest()), ready(forest()), ready(forest())]
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [tithe("Tithe")])
            .battlefield(0, [vec![ready(plains())], lands].concat()),
    )?;
    driver.cast("Tithe")?;
    passes_until_asked(&mut driver)?;
    if white {
        assert_eq!(waiting(&driver), (pid(0), "yes-no"));
        assert_eq!(question(&driver), "Pay {1}{W}?");
        driver.answer(yes_no(true))?;
        driver.pay_tapping(&["Plains", "Forest"], 0)?;
        passes_until_asked(&mut driver)?;
        assert_eq!(life(&driver, 0), 23);
    } else {
        assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
        assert_eq!(life(&driver, 0), 20);
    }
    Ok(())
}

fn priority_pass() -> Answer {
    Answer::Priority(PriorityAction::Pass)
}

/// What a player of these plays answers: cast what is castable at the first target, pay automatically, say yes to a
/// payment and to a draw, and pass.
fn answer_for(decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::Priority(options) => match options.castable.first() {
            Some(card) if decision.player == pid(0) => {
                Answer::Priority(PriorityAction::CastSpell(*card))
            }
            _ => priority_pass(),
        },
        DecisionKind::ChooseMode(options) => Answer::ChooseMode(ModeChoice::Modes(vec![
            options.modes.first().map_or(0, |mode| mode.index),
        ])),
        DecisionKind::ChooseTargets(options) => {
            Answer::ChooseTargets(mtg_engine::choose::TargetChoice::Targets(
                options.legal.iter().copied().take(1).collect(),
            ))
        }
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::YesNo(_) => yes_no(true),
        DecisionKind::ChooseCards(options) => Answer::ChooseCards(CardsChoice(
            options
                .from
                .iter()
                .copied()
                .take(options.min as usize)
                .collect(),
        )),
        other => panic!("the script does not expect {}", other.name()),
    }
}

/// The plays: Izzet Charm's counter mode and Rhystic Study, each at a table of three, played to the end of the stack.
fn plays() -> Vec<(&'static str, Scenario)> {
    vec![
        (
            "Izzet Charm",
            Scenario::new(3)
                .hand_named(0, ["Izzet Charm"])
                .battlefield(0, [ready(island()), ready(mountain())])
                .hand_named(1, ["Lightning Bolt"])
                .battlefield(1, [ready(mountain()), ready(forest()), ready(forest())]),
        ),
        (
            "Rhystic Study",
            Scenario::new(3)
                .battlefield_named(0, ["Rhystic Study"])
                .library_named(0, ["Opt", "Brainstorm"])
                .hand_named(1, ["Lightning Bolt"])
                .battlefield(1, [ready(mountain()), ready(forest())]),
        ),
    ]
}

/// The script of a play: seat 1 casts the Bolt at seat 0 first, then everyone passes and answers as `answer_for` says.
fn play(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    if name == "Izzet Charm" {
        driver.pass()?;
    } else {
        pass_until_held_by(driver, 1)?;
    }
    let target = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[target])?;
    Ok(())
}

#[test]
fn an_engine_rebuilt_from_the_log_asks_the_same_question_at_every_decision_including_payments()
-> Result<(), ScriptError> {
    for (name, scenario) in plays() {
        let setup: GameSetup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        play(&mut driver, name)?;
        let mut payments = 0;
        for _ in 0..60 {
            let decision = driver.decision()?.clone();
            let resumed = Engine::resume(setup.clone(), driver.engine().events().to_vec())?;
            assert_eq!(resumed.state_hash(), driver.engine().state_hash(), "{name}");
            // A cast or activation that is not paid for yet lives outside the state; a payment an effect asks
            // for is state, so it is asked again exactly as it was.
            let in_resolution = driver.state().resolving().is_some();
            let unpaid_cast = matches!(
                decision.kind,
                DecisionKind::PayMana(_)
                    | DecisionKind::ChooseTargets(_)
                    | DecisionKind::ChooseMode(_)
            ) && !in_resolution;
            if !unpaid_cast {
                assert_eq!(resumed.decision(), Some(&decision), "{name}");
            }
            payments +=
                usize::from(matches!(decision.kind, DecisionKind::PayMana(_)) && in_resolution);
            let answer = answer_for(&decision);
            let live = driver.answer(answer.clone())?;
            if !unpaid_cast {
                let mut resumed = resumed;
                assert_eq!(resumed.apply(answer)?, live, "{name}");
                assert_eq!(resumed.state_hash(), driver.engine().state_hash(), "{name}");
            }
            if driver.state().stack().is_empty()
                && driver.state().resolving().is_none()
                && payments > 0
            {
                break;
            }
        }
        assert_eq!(
            payments, 1,
            "{name} asked for one payment while it resolved"
        );
    }
    Ok(())
}

#[test]
fn the_log_replays_to_the_state_it_was_played_to_at_every_decision() -> Result<(), ScriptError> {
    for (name, scenario) in plays() {
        let setup = scenario.clone().setup()?;
        let mut driver = Driver::from_scenario(scenario)?;
        play(&mut driver, name)?;
        let mut points = vec![(driver.engine().events().len(), driver.engine().state_hash())];
        for _ in 0..60 {
            let answer = answer_for(driver.decision()?);
            driver.answer(answer)?;
            points.push((driver.engine().events().len(), driver.engine().state_hash()));
            if driver.state().stack().is_empty()
                && driver.state().resolving().is_none()
                && points.len() > 8
            {
                break;
            }
        }
        let log = driver.engine().events().to_vec();
        for (length, hash) in points {
            assert_eq!(
                replay(&setup, &log[..length])?.state_hash(),
                hash,
                "{name} after {length} events"
            );
        }
    }
    Ok(())
}
