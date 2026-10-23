//! Goal: the ways a Vehicle becomes an artifact creature other than its crew ability play as the rules say at tables of two, three and four seats: an ability that animates another Vehicle offers only Vehicles of its controller besides its source, the Vehicle keeps its printed size while it is a creature and is none again after the turn, a Vehicle that is a creature while it holds enough counters follows the counters live, the cost of tapping two untapped tokens and of paying energy is offered only while it can be paid, a Mount an ability saddles is saddled until end of turn at sorcery speed, and the log of whole games with such cards replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{cost, free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn corpus(name: &str) -> Arc<CardDefinition> {
    card(name).unwrap()
}

/// A free 4/4 Vehicle with Crew 9 that a table can have to animate.
fn skiff() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Skiff")
            .with_subtype("Vehicle")
            .with_type(CardType::Artifact)
            .with_power_toughness(4, 4)
            .with_rules(Ability::Keyword(Keyword::Crew(9))),
    )
}

/// A free creature that makes `untapped` Treasure tokens and `tapped` tapped ones when it enters.
fn tokenmaker(untapped: u32, tapped: u32) -> Arc<CardDefinition> {
    let make = |count: u32, tapped: bool| {
        format!(
            r#"{{"CreateToken":{{"token":{{"Named":"treasure"}},"count":{count},"tapped":{tapped}}}}}"#
        )
    };
    let effects = [(untapped, false), (tapped, true)]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, tapped)| make(count, tapped))
        .collect::<Vec<_>>()
        .join(",");
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{effects}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Tokenmaker")
            .with_mana_cost(cost(0, &[]))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules),
    )
}

fn energy() -> CounterKind {
    CounterKind::Other("energy".into())
}

/// Answers passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Answers the questions of an activation in progress (the first creatures it takes, the mana by itself) and lets it
/// resolve.
fn finish(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..20 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let ids = options.from.iter().take(options.min as usize).copied();
                driver.answer(Answer::ChooseCards(CardsChoice(ids.collect())))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => break,
        }
    }
    settle(driver)
}

/// Activates ability `index` of `source` against these targets and lets it resolve.
fn activate(
    driver: &mut Driver,
    source: &str,
    index: usize,
    targets: &[&str],
) -> Result<(), ScriptError> {
    driver.begin_ability(source, index)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let chosen = targets
            .iter()
            .map(|name| driver.target_named(name))
            .collect::<Result<Vec<_>, _>>()?;
        driver.choose_targets(&chosen)?;
    }
    finish(driver)
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .is_creature()
}

fn is_artifact(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .types
        .contains(CardType::Artifact)
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

fn is_tapped(driver: &Driver, id: mtg_core::ids::ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn saddled(driver: &Driver, name: &str) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap()).contains(&&Keyword::Saddled)
}

#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, scenario: &Scenario) -> Result<(), ScriptError> {
    let replayed = replay(&scenario.clone().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

// An ability that animates another Vehicle

#[rstest]
#[case::peacewalker_colossus("Peacewalker Colossus", 0, "Plains")]
#[case::guidelight_matrix("Guidelight Matrix", 2, "Mountain")]
fn an_ability_makes_the_vehicle_it_targets_an_artifact_creature_until_end_of_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] source: &str,
    #[case] index: usize,
    #[case] land: &str,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(corpus(source)), ready(skiff())];
    mine.extend((0..2).map(|_| {
        ready(if land == "Plains" {
            plains()
        } else {
            mountain()
        })
    }));
    let scenario = Scenario::new(seats).battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert!(!is_creature(&driver, "Skiff"));
    activate(&mut driver, source, index, &["Skiff"])?;
    assert!(is_creature(&driver, "Skiff") && is_artifact(&driver, "Skiff"));
    assert_eq!(stats(&driver, "Skiff"), (4, 4), "the printed size");
    assert!(!is_creature(&driver, source), "only the target is animated");
    driver.idle_until(2, Step::Upkeep)?;
    assert!(!is_creature(&driver, "Skiff"), "it lasts until end of turn");
    assert_replays(&driver, &scenario)
}

#[rstest]
#[case::no_other_vehicle(vec![], false)]
#[case::another_vehicle(vec!["mine"], true)]
#[case::a_vehicle_of_an_opponent(vec!["theirs"], false)]
#[case::a_creature_only(vec!["bear"], false)]
#[case::the_right_one_among_others(vec!["bear", "theirs", "mine"], true)]
fn peacewalker_colossus_offers_only_another_vehicle_of_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: Vec<&str>,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![
        ready(corpus("Peacewalker Colossus")),
        ready(plains()),
        ready(plains()),
    ];
    let mut theirs = Vec::new();
    for other in others {
        match other {
            "mine" => mine.push(ready(skiff())),
            "theirs" => theirs.push(ready(skiff())),
            _ => mine.push(ready(free_creature("Bear", 2, 2))),
        }
    }
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, mine)
            .battlefield(1, theirs),
    )?;
    assert_eq!(
        driver
            .offered_abilities("Peacewalker Colossus")?
            .contains(&0),
        offered
    );
    Ok(())
}

// The Vehicle that is a creature while it holds counters

#[rstest]
fn war_balloon_is_an_artifact_creature_exactly_while_it_holds_three_fire_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 7)] counters: u32,
) -> Result<(), ScriptError> {
    let fire = CounterKind::Other("fire".into());
    let balloon = with_counters(ready(corpus("War Balloon")), fire, counters);
    let scenario = Scenario::new(seats).battlefield(0, [balloon]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let animated = counters >= 3;
    assert_eq!(is_creature(&driver, "War Balloon"), animated);
    assert_eq!(is_artifact(&driver, "War Balloon"), true);
    if animated {
        assert_eq!(stats(&driver, "War Balloon"), (4, 3), "the printed size");
    }
    // The animation outlasts the turn: it is no effect of the turn.
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(is_creature(&driver, "War Balloon"), animated);
    assert_replays(&driver, &scenario)
}

#[rstest]
fn war_balloon_becomes_a_creature_as_its_third_counter_is_put_on_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let fire = CounterKind::Other("fire".into());
    let balloon = with_counters(ready(corpus("War Balloon")), fire, 2);
    let scenario = Scenario::new(seats).battlefield(0, [balloon, ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert!(!is_creature(&driver, "War Balloon"));
    activate(&mut driver, "War Balloon", 1, &[])?;
    assert!(is_creature(&driver, "War Balloon"));
    assert_replays(&driver, &scenario)
}

// Costs that tap tokens and pay energy

/// Seat 0 controls Tangle Tumbler and a Tokenmaker in hand that makes these tokens.
fn tumbler_table(seats: usize, untapped: u32, tapped: u32) -> Scenario {
    Scenario::new(seats)
        .battlefield(0, [ready(corpus("Tangle Tumbler"))])
        .hand(0, [tokenmaker(untapped, tapped)])
}

#[rstest]
#[case::no_token(0, 0, false)]
#[case::one_token(1, 0, false)]
#[case::two_tokens(2, 0, true)]
#[case::three_tokens(3, 0, true)]
#[case::one_untapped_and_one_tapped(1, 1, false)]
#[case::two_untapped_and_one_tapped(2, 1, true)]
fn tangle_tumbler_is_offered_while_two_untapped_tokens_can_be_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] untapped: u32,
    #[case] tapped: u32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(tumbler_table(seats, untapped, tapped))?;
    // The Tokenmaker is a creature, not a token: nothing to tap before it makes some.
    assert!(!driver.offered_abilities("Tangle Tumbler")?.contains(&2));
    driver.cast("Tokenmaker")?;
    settle(&mut driver)?;
    assert_eq!(
        driver.offered_abilities("Tangle Tumbler")?.contains(&2),
        offered
    );
    Ok(())
}

#[rstest]
fn tangle_tumbler_taps_two_tokens_and_becomes_an_artifact_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3)] tokens: u32,
) -> Result<(), ScriptError> {
    let scenario = tumbler_table(seats, tokens, 0);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Tokenmaker")?;
    settle(&mut driver)?;
    assert!(!is_creature(&driver, "Tangle Tumbler"));
    let treasures = |driver: &Driver| -> Vec<mtg_core::ids::ObjectId> {
        driver
            .state()
            .zone(mtg_engine::zones::Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| common::name_of(driver.state(), *id) == "Treasure")
            .collect()
    };
    assert_eq!(treasures(&driver).len(), usize::try_from(tokens).unwrap());
    activate(&mut driver, "Tangle Tumbler", 2, &[])?;
    assert!(is_creature(&driver, "Tangle Tumbler"));
    assert_eq!(stats(&driver, "Tangle Tumbler"), (6, 6), "the printed size");
    let tapped = treasures(&driver)
        .into_iter()
        .filter(|id| is_tapped(&driver, *id))
        .count();
    assert_eq!(tapped, 2, "exactly two tokens are tapped");
    assert_replays(&driver, &scenario)
}

#[rstest]
#[case::none(0, false)]
#[case::three(3, false)]
#[case::four(4, true)]
#[case::five(5, true)]
fn bespoke_battlewagon_pays_four_energy_to_become_an_artifact_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: u32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(corpus("Bespoke Battlewagon"))])
        .counters(0, energy(), held);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert_eq!(
        driver
            .offered_abilities("Bespoke Battlewagon")?
            .contains(&3),
        offered
    );
    if offered {
        activate(&mut driver, "Bespoke Battlewagon", 3, &[])?;
        assert!(is_creature(&driver, "Bespoke Battlewagon"));
        assert_eq!(
            stats(&driver, "Bespoke Battlewagon"),
            (5, 6),
            "the printed size"
        );
        let left = driver
            .state()
            .player(pid(0))
            .unwrap()
            .counters_of(&energy());
        assert_eq!(left, held - 4);
        assert_replays(&driver, &scenario)?;
    }
    Ok(())
}

// A Mount that an ability saddles

#[rstest]
fn guidelight_matrix_saddles_a_mount_until_end_of_turn_at_sorcery_speed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![
        ready(corpus("Guidelight Matrix")),
        ready(corpus("Bridled Bighorn")),
        ready(mountain()),
        ready(mountain()),
    ];
    let scenario = Scenario::new(seats).battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert!(!saddled(&driver, "Bridled Bighorn"));
    assert!(driver.offered_abilities("Guidelight Matrix")?.contains(&1));
    // At sorcery speed: not in combat.
    driver.pass_until(Step::BeginCombat)?;
    assert!(!driver.offered_abilities("Guidelight Matrix")?.contains(&1));
    driver.idle_until(1, Step::Main2)?;
    assert!(driver.offered_abilities("Guidelight Matrix")?.contains(&1));
    activate(&mut driver, "Guidelight Matrix", 1, &["Bridled Bighorn"])?;
    assert!(saddled(&driver, "Bridled Bighorn"));
    driver.idle_until(2, Step::Upkeep)?;
    assert!(!saddled(&driver, "Bridled Bighorn"), "until end of turn");
    assert_replays(&driver, &scenario)
}

// Whole games

#[test]
fn bot_games_with_the_animating_vehicles_keep_every_invariant_and_replay() {
    use std::sync::Mutex;

    use mtg_engine::Event;
    use mtg_testkit::vehicle_cards::VEHICLE_CARDS;
    use mtg_testkit::{PlayOptions, play_game_with, vehicle_fuzz_setup};

    use crate::common::seeds::{for_each_seed, fuzz_games};

    let shapes = [
        "Peacewalker Colossus",
        "War Balloon",
        "Guidelight Matrix",
        "Tangle Tumbler",
    ];
    for name in shapes.iter().chain(&["Bespoke Battlewagon"]) {
        assert!(
            VEHICLE_CARDS.contains(name),
            "{name} is in the Vehicle decks"
        );
    }
    // Abilities of these cards that were activated, and spells cast, in all the games.
    let seen = Mutex::new((0u64, 0u64));
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                vehicle_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&vehicle_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                let (object, activated) = match event {
                    Event::AbilityActivated { source, .. } => (source.id, true),
                    Event::SpellCast { object, .. } => (*object, false),
                    _ => continue,
                };
                let name = replayed.object(object).map(|o| o.card().name.clone());
                if name.is_some_and(|name| shapes.contains(&name.as_str())) {
                    if activated {
                        seen.0 += 1;
                    } else {
                        seen.1 += 1;
                    }
                }
            }
        }
    });
    let (activated, cast) = seen.into_inner().unwrap();
    assert!(cast > 0, "some of these Vehicles were cast");
    assert!(
        activated > 0,
        "some ability of these Vehicles was activated"
    );
}
