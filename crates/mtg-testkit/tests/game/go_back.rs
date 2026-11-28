//! Goal: a player goes back to an earlier step over passes and turn-based steps that reveal and decide nothing (two to four seats, their own turn or another's, step by step), the game returning to exactly the state of their pass with a `WentBack` note and a log that replays to it; each irreversible kind since the pass (a draw, a spell, a mana ability, an attack, an action under way, a table without take-backs, nothing passed yet) refuses it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::go_back::GoBackRefusal;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::Scenario;
use mtg_testkit::cards::{creature, forest, free_creature, instant};
use mtg_testkit::scenario::ready;
use rstest::rstest;

const SEAT: PlayerId = PlayerId(0);

fn spark() -> Arc<CardDefinition> {
    instant("Spark", 0, &[])
}

/// `players` seats on turn 2 at `step` of `active`'s turn; seat 0 holds a Forest, Spark and a two-mana creature over
/// an untapped Forest; every library holds Forests.
fn table(players: usize, active: usize, step: Step, rules: HouseRules) -> (GameSetup, Engine) {
    let mut scenario = Scenario::new(players)
        .at(2, active, step)
        .house_rules(rules)
        .hand(0, [forest(), spark(), creature("Ogre", 1, &[], 3, 3)])
        .battlefield(0, [ready(forest())]);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let setup = scenario.setup().unwrap();
    (setup.clone(), Engine::new(setup).unwrap())
}

fn holder(engine: &Engine) -> PlayerId {
    engine.decision().unwrap().player
}

fn step(engine: &Engine) -> Step {
    engine.state().turn().step()
}

fn priority(engine: &mut Engine, action: PriorityAction) {
    engine.apply(Answer::Priority(action)).unwrap();
}

/// Every seat passes until the game reaches `to` with seat 0 to act.
fn pass_to(engine: &mut Engine, to: Step) {
    for _ in 0..200 {
        if step(engine) == to && holder(engine) == SEAT {
            return;
        }
        match &engine.decision().unwrap().kind {
            DecisionKind::Priority(_) => priority(engine, PriorityAction::Pass),
            other => panic!("only priority is asked on the way, not {}", other.name()),
        }
    }
    panic!("the game never reached {to:?}");
}

/// Seat 0 passes once, then every seat passes until seat 0 is asked in another step.
fn pass_on(engine: &mut Engine) {
    let from = step(engine);
    priority(engine, PriorityAction::Pass);
    for _ in 0..200 {
        if step(engine) != from && holder(engine) == SEAT {
            return;
        }
        match &engine.decision().unwrap().kind {
            DecisionKind::Priority(_) => priority(engine, PriorityAction::Pass),
            _ => return,
        }
    }
}

fn hand_card(engine: &Engine, name: &str) -> mtg_core::ids::ObjectId {
    engine
        .state()
        .zone(Zone::Hand(SEAT))
        .iter()
        .copied()
        .find(|id| engine.state().name_of(*id).as_deref() == Some(name))
        .unwrap()
}

/// The state and decision a go-back must return to.
fn fingerprint(engine: &Engine) -> (u64, String, PlayerId, Step) {
    (
        engine.state_hash(),
        engine.decision().unwrap().kind.name().to_owned(),
        holder(engine),
        step(engine),
    )
}

/// Goes back and checks the result: the state of the pass, the note last, the log replaying to the live state and a
/// rebuilt engine asking the same.
fn go_back_and_check(
    setup: &GameSetup,
    engine: &mut Engine,
    before: &(u64, String, PlayerId, Step),
) {
    let offer = engine.go_back_offer(SEAT, |_| true).unwrap();
    assert_eq!(offer.step, before.3);
    let at = usize::try_from(offer.at).unwrap();
    let events = engine.go_back(SEAT, offer.at).unwrap();
    assert!(
        matches!(events.first(), Some(Event::WentBack { player, step, .. }) if *player == SEAT && *step == before.3)
    );
    assert_eq!(engine.events().len(), at + events.len());
    let (hash, kind, who, at_step) = before.clone();
    assert_eq!(engine.decision().unwrap().kind.name(), kind);
    assert_eq!((holder(engine), step(engine)), (who, at_step));
    assert_eq!(engine.state_hash(), hash, "the note changes nothing");
    assert_eq!(
        replay(setup, engine.events()).unwrap().state_hash(),
        engine.state_hash()
    );
    let rebuilt = Engine::resume(setup.clone(), engine.events().to_vec()).unwrap();
    assert_eq!(rebuilt.decision(), engine.decision());
}

#[rstest]
#[case::own_main_to_combat(2, 0, Step::Main1)]
#[case::own_second_main_to_end(2, 0, Step::Main2)]
#[case::four_seats(4, 0, Step::Main1)]
#[case::another_seats_turn(2, 1, Step::Main1)]
#[case::four_seats_another_turn(4, 2, Step::BeginCombat)]
#[case::end_step_into_the_next_turns_upkeep(2, 1, Step::End)]
fn a_seat_goes_back_over_passes_to_the_step_it_passed_in(
    #[case] players: usize,
    #[case] active: usize,
    #[case] start: Step,
) {
    let (setup, mut engine) = table(players, active, start, HouseRules::default());
    pass_to(&mut engine, start);
    let before = fingerprint(&engine);
    pass_on(&mut engine);
    assert_ne!(step(&engine), start);
    go_back_and_check(&setup, &mut engine, &before);
}

#[test]
fn a_land_played_before_the_pass_stays_and_the_seat_can_go_back_twice() {
    let (setup, mut engine) = table(2, 0, Step::Main1, HouseRules::default());
    let card = hand_card(&engine, "Forest");
    priority(&mut engine, PriorityAction::PlayLand(card));
    let main = fingerprint(&engine);
    pass_on(&mut engine);
    assert_eq!(step(&engine), Step::BeginCombat);
    let combat = fingerprint(&engine);
    pass_on(&mut engine);
    go_back_and_check(&setup, &mut engine, &combat);
    go_back_and_check(&setup, &mut engine, &main);
    assert_eq!(
        engine.state().zone(Zone::Battlefield).len(),
        2,
        "the land played stays"
    );
}

/// What happens after seat 0 passed its precombat main phase and is asked at beginning of combat.
#[derive(Debug, Clone, Copy)]
enum Since {
    CastSpark,
    TappedForMana,
}

#[rstest]
#[case::a_spell(Since::CastSpark, GoBackRefusal::Acted)]
#[case::a_mana_ability(Since::TappedForMana, GoBackRefusal::Acted)]
fn an_action_since_the_pass_refuses_going_back(
    #[case] since: Since,
    #[case] refusal: GoBackRefusal,
) {
    let (_, mut engine) = table(2, 0, Step::Main1, HouseRules::default());
    pass_on(&mut engine);
    assert!(engine.go_back_offer(SEAT, |_| true).is_ok());
    let DecisionKind::Priority(options) = &engine.decision().unwrap().kind else {
        panic!("seat 0 holds priority");
    };
    let source = options.mana_abilities[0].source;
    match since {
        Since::CastSpark => {
            let card = hand_card(&engine, "Spark");
            priority(&mut engine, PriorityAction::CastSpell(card));
        }
        Since::TappedForMana => priority(&mut engine, PriorityAction::ActivateMana(source)),
    }
    let mut answered = 0;
    while !matches!(engine.decision().unwrap().kind, DecisionKind::Priority(_)) && answered < 5 {
        engine
            .apply(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
            .unwrap();
        answered += 1;
    }
    assert_eq!(engine.go_back_offer(SEAT, |_| true), Err(refusal));
}

#[test]
fn a_draw_since_the_pass_refuses_going_back_past_it() {
    let (_, mut engine) = table(2, 0, Step::Upkeep, HouseRules::default());
    pass_on(&mut engine);
    assert_eq!(step(&engine), Step::Draw);
    assert_eq!(
        engine.go_back_offer(SEAT, |_| true),
        Err(GoBackRefusal::CardDrawn)
    );
}

#[test]
fn an_attack_declared_since_refuses_going_back() {
    let mut engine = Scenario::new(2)
        .at(2, 0, Step::BeginCombat)
        .battlefield(0, [ready(free_creature("Bear", 2, 2))])
        .build()
        .unwrap();
    priority(&mut engine, PriorityAction::Pass);
    priority(&mut engine, PriorityAction::Pass);
    let DecisionKind::DeclareAttackers(options) = &engine.decision().unwrap().kind else {
        panic!("seat 0 declares attackers");
    };
    let attack = vec![(
        options.attackers[0].object,
        options.attackers[0].defenders[0],
    )];
    engine.apply(Answer::DeclareAttackers(attack)).unwrap();
    assert_eq!(holder(&engine), SEAT);
    assert_eq!(
        engine.go_back_offer(SEAT, |_| true),
        Err(GoBackRefusal::Declared)
    );
}

#[test]
fn an_action_under_way_a_table_without_take_backs_and_no_pass_refuse() {
    let (_, mut engine) = table(2, 0, Step::Main1, HouseRules::default());
    assert_eq!(
        engine.go_back_offer(SEAT, |_| true),
        Err(GoBackRefusal::Nothing)
    );
    let card = hand_card(&engine, "Ogre");
    priority(&mut engine, PriorityAction::CastSpell(card));
    assert_eq!(
        engine.go_back_offer(SEAT, |_| true),
        Err(GoBackRefusal::Busy)
    );

    let off = HouseRules {
        take_backs: Some(false),
        ..HouseRules::default()
    };
    let (_, mut engine) = table(2, 0, Step::Main1, off);
    pass_on(&mut engine);
    assert_eq!(
        engine.go_back_offer(SEAT, |_| true),
        Err(GoBackRefusal::Off)
    );
    assert!(engine.go_back(SEAT, 0).is_err());
}

#[test]
fn only_a_pass_the_caller_chose_is_gone_back_to() {
    let (_, mut engine) = table(2, 0, Step::Main1, HouseRules::default());
    pass_on(&mut engine);
    let offer = engine.go_back_offer(SEAT, |_| true).unwrap();
    assert_eq!(
        engine.go_back_offer(SEAT, |at| at != offer.at),
        Err(GoBackRefusal::Nothing)
    );
    assert!(
        engine.go_back(SEAT, offer.at + 1).is_err(),
        "a position that is no pass is refused"
    );
    assert_eq!(
        engine
            .go_back_offer(PlayerId(1), |_| true)
            .map(|o| o.player),
        Ok(PlayerId(1))
    );
}
