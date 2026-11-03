//! Goal: a `GameRule::Cant` read from a static ability keeps its subjects from the action it names in every place the engine does it: a land play, an activation, a target and a cost are not offered and the answer that takes them is refused, an effect that would sacrifice, discard, exile or put counters does nothing without asking for the impossible or leaving the game stuck, a trigger does not trigger, steps and turns are skipped, and `Effect::EndTurn` exiles the stack and goes to the cleanup step from any step; each game replays to the state it reached.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::Prohibited;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{burn, cost, edict, forest, free_creature, mind_rot, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact with the given static rule, "Warden", controlled by seat 0.
fn warden(rule: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#))),
    )
}

fn cant(who: &str, action: &str) -> String {
    format!(r#"{{"Cant":{{"who":{who},"action":{action}}}}}"#)
}

/// A free spell of the given type with one ability.
fn spell(name: &str, kind: CardType, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability(json)),
    )
}

fn artifact(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability(json)),
    )
}

/// Seat 0 holds `hand` in `step` of turn 2 with `mine` on the battlefield beside the Warden; seat 1 controls `theirs`
/// and holds `their_hand`.
fn table(
    rule: &str,
    step: Step,
    hand: Vec<Arc<CardDefinition>>,
    mine: Vec<Arc<CardDefinition>>,
    theirs: Vec<Arc<CardDefinition>>,
    their_hand: Vec<Arc<CardDefinition>>,
) -> Driver {
    let mut battlefield = vec![warden(rule)];
    battlefield.extend(mine);
    let scenario = Scenario::new(2)
        .at(2, 0, step)
        .hand(0, hand)
        .hand(1, their_hand)
        .battlefield(0, battlefield.into_iter().map(ready))
        .battlefield(1, theirs.into_iter().map(ready))
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    Driver::from_scenario(scenario).unwrap()
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {other:?}"),
    }
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) {
    for _ in 0..40 {
        if driver.state().stack().is_empty()
            || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            return;
        }
        driver.pass().unwrap();
    }
    panic!("the stack never emptied");
}

fn count(state: &GameState, zone: Zone) -> usize {
    state.zone(zone).len()
}

#[rstest]
#[case::mine(r#""You""#, true)]
#[case::theirs(r#""Opponents""#, false)]
#[case::everyones(r#""EachPlayer""#, true)]
fn a_land_is_not_offered_and_not_played_while_land_plays_are_forbidden(
    #[case] who: &str,
    #[case] blocked: bool,
) {
    let mut driver = table(
        &cant(who, r#""PlayLand""#),
        Step::Main1,
        vec![forest()],
        vec![],
        vec![],
        vec![],
    );
    assert_eq!(options(&driver).playable_lands.is_empty(), blocked);
    assert_eq!(driver.play_land("Forest").is_err(), blocked);
    assert_replays(&driver);
}

#[rstest]
#[case::mine("\"You\"", "[]", true)]
#[case::theirs("\"Opponents\"", "[]", false)]
#[case::artifacts("\"You\"", r#"[{"Type":"Artifact"}]"#, true)]
#[case::creatures_only("\"You\"", r#"[{"Type":"Creature"}]"#, false)]
fn an_ability_is_not_offered_and_not_activated_while_its_activation_is_forbidden(
    #[case] who: &str,
    #[case] sources: &str,
    #[case] blocked: bool,
) {
    let totem = artifact(
        "Totem",
        r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let mut driver = table(
        &cant(who, &format!(r#"{{"Activate":{sources}}}"#)),
        Step::Main1,
        vec![],
        vec![totem],
        vec![],
        vec![],
    );
    assert_eq!(
        driver.offered_abilities("Totem").unwrap().is_empty(),
        blocked
    );
    assert_eq!(driver.begin_activation("Totem").is_err(), blocked);
}

#[rstest]
#[case::no_creature_at_all(r#"{"Each":[{"Type":"Creature"}]}"#, "[]", "Bear", true)]
#[case::from_my_sources_only(
    r#"{"Each":[{"Type":"Creature"}]}"#,
    r#"[{"ControlledBy":"Opponents"}]"#,
    "Bear",
    false
)]
#[case::from_their_sources(
    r#"{"Each":[{"Type":"Creature"}]}"#,
    r#"[{"ControlledBy":"You"}]"#,
    "Bear",
    true
)]
#[case::the_opponent_player(r#""Opponents""#, "[]", "player", true)]
#[case::me(r#""You""#, "[]", "player", false)]
fn a_target_is_not_legal_for_the_sources_a_prohibition_names(
    #[case] who: &str,
    #[case] by: &str,
    #[case] target: &str,
    #[case] blocked: bool,
) {
    let mut driver = table(
        &cant(who, &format!(r#"{{"BeTargeted":{by}}}"#)),
        Step::Main1,
        vec![burn("Bolt", 1)],
        vec![mountain()],
        vec![free_creature("Bear", 2, 2)],
        vec![],
    );
    driver.begin_cast("Bolt").unwrap();
    let wanted = if target == "player" {
        driver.target_player(1)
    } else {
        driver.target_named("Bear").unwrap()
    };
    let DecisionKind::ChooseTargets(offered) = &driver.decision().unwrap().kind else {
        panic!("no target decision");
    };
    assert_eq!(!offered.legal.contains(&wanted), blocked);
    assert_eq!(driver.choose_targets(&[wanted]).is_err(), blocked);
}

#[rstest]
#[case::theirs_cant_be_sacrificed("\"Opponents\"", 1)]
#[case::mine_cant_but_theirs_can("\"You\"", 0)]
fn an_edict_against_a_player_who_cant_sacrifice_does_nothing_and_asks_nothing(
    #[case] who: &str,
    #[case] left: usize,
) {
    let mut driver = table(
        &cant(who, r#""Sacrifice""#),
        Step::Main1,
        vec![edict("Edict")],
        vec![swamp(), swamp()],
        vec![free_creature("Bear", 2, 2)],
        vec![],
    );
    let target = driver.target_player(1);
    driver.cast_at("Edict", &[target]).unwrap();
    settle(&mut driver);
    let creatures = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Bear")
        .count();
    assert_eq!(creatures, left);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
    assert_replays(&driver);
}

#[rstest]
#[case::theirs_cant_discard("\"Opponents\"", 3)]
#[case::mine_cant_discard("\"You\"", 1)]
fn a_discard_effect_against_a_player_who_cant_discard_takes_no_card(
    #[case] who: &str,
    #[case] left: usize,
) {
    let mut driver = table(
        &cant(who, r#""Discard""#),
        Step::Main1,
        vec![mind_rot("Rot")],
        vec![swamp(), swamp()],
        vec![],
        vec![forest(), forest(), forest()],
    );
    let target = driver.target_player(1);
    driver.cast_at("Rot", &[target]).unwrap();
    // The player discards two of three cards when allowed; their choice is a decision of its own.
    for _ in 0..10 {
        match &driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(count(driver.state(), Zone::Hand(PlayerId(1))), left);
    assert_replays(&driver);
}

#[rstest]
#[case::counters(
    r#"{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}"#,
    r#""PutCounter""#,
    true
)]
#[case::exile(r#"{"Exile":{"what":{"Target":0}}}"#, r#""Exile""#, true)]
#[case::exile_unaffected_by_counter_rule(
    r#"{"Exile":{"what":{"Target":0}}}"#,
    r#""PutCounter""#,
    false
)]
#[case::counters_unaffected_by_exile_rule(
    r#"{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}"#,
    r#""Exile""#,
    false
)]
fn an_effect_on_a_creature_that_cant_be_its_subject_does_nothing(
    #[case] effect: &str,
    #[case] action: &str,
    #[case] kept: bool,
) {
    let probe = spell(
        "Probe",
        CardType::Instant,
        &format!(
            r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{effect}]}}}}"#
        ),
    );
    let mut driver = table(
        &cant(r#"{"Each":[{"Type":"Creature"}]}"#, action),
        Step::Main1,
        vec![probe],
        vec![],
        vec![free_creature("Bear", 2, 2)],
        vec![],
    );
    let bear = driver.target_named("Bear").unwrap();
    driver.cast_at("Probe", &[bear]).unwrap();
    settle(&mut driver);
    let state = driver.state();
    let in_play = state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| state.object(*id).unwrap().card().name == "Bear");
    let counters = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id).and_then(|o| o.permanent()))
        .map(|p| p.counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne))
        .sum::<u32>();
    if effect.contains("Exile") {
        assert_eq!(in_play, kept);
    } else {
        assert_eq!(counters == 0, kept);
    }
    assert_replays(&driver);
}

#[rstest]
#[case::mine_is_stopped("\"You\"", 0)]
#[case::theirs_is_not_mine("\"Opponents\"", 1)]
fn a_trigger_the_rule_names_does_not_trigger(#[case] who: &str, #[case] gained: i32) {
    let warden_of_souls = artifact(
        "Soul Keeper",
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let pattern = r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"to":"Battlefield"}}"#;
    let mut driver = table(
        &cant(who, &format!(r#"{{"Trigger":{pattern}}}"#)),
        Step::Main1,
        vec![free_creature("Bear", 2, 2)],
        vec![warden_of_souls],
        vec![],
        vec![],
    );
    let before = driver.state().player(PlayerId(0)).unwrap().life();
    driver.cast("Bear").unwrap();
    settle(&mut driver);
    settle(&mut driver);
    let after = driver.state().player(PlayerId(0)).unwrap().life();
    assert_eq!(after - before, gained);
    assert_replays(&driver);
}

#[rstest]
#[case::draw(Step::Upkeep, Step::Draw, Step::Main1)]
#[case::beginning_of_combat(Step::Main1, Step::BeginCombat, Step::EndCombat)]
#[case::not_the_next_step(Step::Upkeep, Step::Main2, Step::Draw)]
#[case::the_end_step(Step::Main2, Step::End, Step::Cleanup)]
fn a_skipped_step_does_not_begin(#[case] from: Step, #[case] skipped: Step, #[case] reaches: Step) {
    let step = match skipped {
        Step::Draw => r#"{"SkipStep":"Draw"}"#,
        Step::BeginCombat => r#"{"SkipStep":"BeginCombat"}"#,
        Step::Main2 => r#"{"SkipStep":"Main2"}"#,
        _ => r#"{"SkipStep":"End"}"#,
    };
    let mut driver = table(&cant("\"You\"", step), from, vec![], vec![], vec![], vec![]);
    let library = count(driver.state(), Zone::Library(PlayerId(0)));
    let turn = driver.state().turn().number();
    driver.pass().unwrap();
    driver.pass().unwrap();
    if reaches == Step::Cleanup {
        // The end step is skipped: the turn is over and the next begins.
        assert_eq!(driver.state().turn().number(), turn + 1);
    } else {
        assert_eq!(driver.state().turn().step(), reaches);
    }
    if skipped == Step::Draw {
        assert_eq!(count(driver.state(), Zone::Library(PlayerId(0))), library);
    }
    assert_replays(&driver);
}

#[rstest]
#[case::nobody(&[], 1)]
#[case::the_next_seat(&[1], 2)]
#[case::two_seats(&[1, 2], 0)]
#[case::every_seat_takes_its_turn_when_all_skip(&[0, 1, 2], 1)]
fn a_player_whose_turns_are_skipped_does_not_take_the_next(
    #[case] skipping: &[usize],
    #[case] next: usize,
) {
    let scenario = Scenario::new(3)
        .at(2, 0, Step::Main2)
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()))
        .library(2, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if !skipping.is_empty() {
        let who = skipping
            .iter()
            .map(|s| Target::Player(PlayerId(*s as u64)))
            .collect();
        driver
            .add_rule(
                Rule::Cant {
                    who,
                    action: Box::new(Prohibited::SkipTurn),
                },
                Duration::Forever,
            )
            .unwrap();
    }
    driver.idle_until(3, Step::Upkeep).unwrap();
    assert_eq!(driver.state().turn().active(), PlayerId(next as u64));
    assert_replays(&driver);
}

#[rstest]
#[case::upkeep(Step::Upkeep)]
#[case::main(Step::Main1)]
#[case::beginning_of_combat(Step::BeginCombat)]
#[case::second_main(Step::Main2)]
#[case::end(Step::End)]
fn ending_the_turn_exiles_the_stack_and_goes_to_the_next_turn(#[case] step: Step) {
    let time_stop = spell(
        "Time Stop",
        CardType::Instant,
        r#"{"Spell":{"effects":["EndTurn"]}}"#,
    );
    let trick = spell(
        "Trick",
        CardType::Instant,
        r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let scenario = Scenario::new(2)
        .at(2, 0, step)
        .hand(0, vec![trick, time_stop])
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.cast("Trick").unwrap();
    driver.cast("Time Stop").unwrap();
    driver.pass().unwrap();
    driver.pass().unwrap();
    let state = driver.state();
    assert_eq!(state.stack().len(), 0);
    assert_eq!(
        count(state, Zone::Exile),
        1,
        "the spell it answered is exiled"
    );
    assert_eq!(
        state.player(PlayerId(0)).unwrap().life(),
        life,
        "and did nothing"
    );
    assert_eq!(count(state, Zone::Graveyard(PlayerId(0))), 1);
    driver.idle_until(3, Step::Upkeep).unwrap();
    assert_eq!(driver.state().turn().active(), PlayerId(1));
    assert_replays(&driver);
}

#[test]
fn ending_the_turn_ends_combat_and_its_attackers() {
    let time_stop = spell(
        "Time Stop",
        CardType::Instant,
        r#"{"Spell":{"effects":["EndTurn"]}}"#,
    );
    let scenario = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .hand(0, vec![time_stop])
        .battlefield(0, vec![ready(free_creature("Bear", 2, 2))])
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.attack(&[("Bear", 1)]).unwrap();
    driver.cast("Time Stop").unwrap();
    driver.pass().unwrap();
    driver.pass().unwrap();
    assert_eq!(
        driver.state().player(PlayerId(1)).unwrap().life(),
        20,
        "the attack never reached the damage step"
    );
    driver.idle_until(3, Step::Upkeep).unwrap();
    assert_eq!(driver.state().combat().attackers().len(), 0);
}

#[test]
fn a_prohibited_color_of_mana_ability_is_unaffected() {
    // Mana abilities are not activated through priority: a rule against activation leaves the mana of lands alone.
    let mut driver = table(
        &cant("\"You\"", r#"{"Activate":[]}"#),
        Step::Main1,
        vec![burn("Bolt", 1)],
        vec![mountain()],
        vec![],
        vec![],
    );
    let target = driver.target_player(1);
    driver.cast_at("Bolt", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(driver.state().player(PlayerId(1)).unwrap().life(), 19);
}
