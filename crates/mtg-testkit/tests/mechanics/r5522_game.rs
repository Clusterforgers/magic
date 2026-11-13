//! Goal: the facts of the turn and the game the r55-22 cards read hold exactly when the history makes them true, at two
//! to four seats: Yidaro enters on the fourth cycle of its name, Diseased Vermin targets only an opponent it dealt damage,
//! Gollum drains only the opponents it hit in combat, Churning Reservoir and Izzet Generatorium activate only after oil
//! or energy left, Oblivious Bookworm keeps the card after a permanent entered face down, Mana Maze stops a spell of the
//! last spell's color, and Power Surge counts the lands untapped as the turn began.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, instant, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, tapped, with_counters};
use rstest::rstest;

/// A permanent of the type with the abilities, each given as JSON.
fn r5522_custom(name: &str, kind: CardType, abilities: &[&str]) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(name)
        .with_type(kind)
        .with_power_toughness(2, 2);
    for ability in abilities {
        definition = definition.with_rules(serde_json::from_str(ability).unwrap());
    }
    Arc::new(definition)
}

/// Answers until the stack is empty with priority pending: "yes" to every yes-or-no question, `targets` picks targets.
fn r5522_run(driver: &mut Driver, targets: &mut dyn FnMut(&[Target]) -> Vec<Target>) {
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::ChooseTargets(options) => {
                let chosen = targets(&options.legal);
                driver.choose_targets(&chosen).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// The first legal target.
fn r5522_first(legal: &[Target]) -> Vec<Target> {
    legal.iter().take(1).copied().collect()
}

/// The life of the seat.
fn r5522_life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

/// How many permanents of the name are on the battlefield.
fn r5522_count(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == name)
        .count()
}

/// Activates the first ability of the named object and pays its mana automatically, then lets it resolve.
fn r5522_activate(driver: &mut Driver, name: &str) {
    driver.begin_activation(name).unwrap();
    if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    }
    r5522_run(driver, &mut r5522_first);
}

#[rstest]
fn r5522_yidaro_enters_on_the_fourth_cycle(
    #[values(2, 3, 4)] seats: usize,
    #[values(3, 4)] cycles: usize,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, (0..8).map(|_| ready(mountain())))
        .hand(
            0,
            (0..cycles).map(|_| card("Yidaro, Wandering Monster").unwrap()),
        )
        .library(0, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for _ in 0..cycles {
        r5522_activate(&mut driver, "Yidaro, Wandering Monster");
    }
    let entered = r5522_count(&driver, "Yidaro, Wandering Monster");
    assert_eq!(entered, usize::from(cycles == 4));
    let graveyard = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(0)));
    assert!(
        graveyard.is_empty(),
        "each cycled Yidaro left the graveyard"
    );
}

#[rstest]
fn r5522_vermin_targets_only_an_opponent_it_hurt(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] attacked: bool,
) {
    let vermin = with_counters(
        ready(card("Diseased Vermin").unwrap()),
        CounterKind::Other("infection".to_owned()),
        2,
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(0, [vermin]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let attacks: &[(&str, usize)] = if attacked {
        &[("Diseased Vermin", 1)]
    } else {
        &[]
    };
    driver.attack(attacks).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    let next = u32::try_from(3 + seats).unwrap();
    driver.idle_until(next, Step::Upkeep).unwrap();
    let mut offered: Vec<Target> = Vec::new();
    r5522_run(&mut driver, &mut |legal| {
        offered = legal.to_vec();
        r5522_first(legal)
    });
    let foe = driver.target_player(1);
    if attacked {
        assert_eq!(offered, [foe]);
        assert_eq!(r5522_life(&driver, 1), 20 - 1 - 3);
    } else {
        assert_eq!(offered, [] as [mtg_engine::choose::Target; 0]);
        assert_eq!(r5522_life(&driver, 1), 20);
    }
    for seat in 2..seats {
        assert_eq!(r5522_life(&driver, seat), 20);
    }
}

#[rstest]
fn r5522_gollum_drains_the_opponents_it_hit(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 3)] gained: i32,
) {
    let balm = Arc::new(
        CardDefinition::new("Balm")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Spell":{{"effects":[{{"GainLife":{{"amount":{gained}}}}}]}}}}"#
                ))
                .unwrap(),
            ),
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(card("Gollum, Obsessed Stalker").unwrap()),
                ready(mountain()),
            ],
        )
        .hand(0, [balm]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.attack(&[("Gollum, Obsessed Stalker", 1)]).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    driver.idle_until(3, Step::Main2).unwrap();
    if gained > 0 {
        driver.cast("Balm").unwrap();
        r5522_run(&mut driver, &mut r5522_first);
    }
    driver.idle_until(3, Step::End).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    assert_eq!(r5522_life(&driver, 1), 20 - 1 - gained);
    for seat in 2..seats {
        assert_eq!(r5522_life(&driver, seat), 20, "seat {seat} was not hit");
    }
}

#[rstest]
#[case::nothing("", false)]
#[case::oil_removed("Oiler", true)]
#[case::oiled_permanent_died("Lightning Bolt", true)]
fn r5522_reservoir_waits_for_oil(
    #[values(2, 3, 4)] seats: usize,
    #[case] act: &str,
    #[case] offered: bool,
) {
    let oiler = r5522_custom(
        "Oiler",
        CardType::Artifact,
        &[
            r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":{"Other":"oil"}}}],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ],
    );
    let oil = CounterKind::Other("oil".to_owned());
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Churning Reservoir").unwrap()),
                with_counters(ready(oiler), oil.clone(), 1),
                with_counters(ready(bear()), oil, 1),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(0, [card("Lightning Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    match act {
        "Oiler" => r5522_activate(&mut driver, "Oiler"),
        "Lightning Bolt" => {
            let bears = driver.target_named("Grizzly Bears").unwrap();
            driver.cast_at("Lightning Bolt", &[bears]).unwrap();
            r5522_run(&mut driver, &mut r5522_first);
        }
        _ => {}
    }
    let abilities = driver.offered_abilities("Churning Reservoir").unwrap();
    assert_eq!(!abilities.is_empty(), offered, "{abilities:?}");
}

#[rstest]
#[case::three_paid(&[3], false)]
#[case::four_paid(&[4], true)]
#[case::two_and_two(&[2, 2], true)]
fn r5522_generatorium_counts_energy_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] payments: &[u32],
    #[case] offered: bool,
) {
    let batteries: Vec<_> = payments
        .iter()
        .enumerate()
        .map(|(i, paid)| {
            ready(r5522_custom(
                &format!("Battery {i}"),
                CardType::Artifact,
                &[&format!(
                    r#"{{"Activated":{{"cost":[{{"PayEnergy":{paid}}}],"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
                )],
            ))
        })
        .collect();
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            std::iter::once(ready(card("Izzet Generatorium").unwrap())).chain(batteries),
        )
        .counters(0, CounterKind::Other("energy".to_owned()), 8);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for i in 0..payments.len() {
        r5522_activate(&mut driver, &format!("Battery {i}"));
    }
    let abilities = driver.offered_abilities("Izzet Generatorium").unwrap();
    assert_eq!(!abilities.is_empty(), offered, "{abilities:?}");
}

#[rstest]
fn r5522_bookworm_keeps_the_card_after_a_face_down_entry(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] manifested: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Oblivious Bookworm").unwrap()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .hand(0, [card("Manifest Dread").unwrap()])
        .library(0, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if manifested {
        driver.cast("Manifest Dread").unwrap();
        r5522_run(&mut driver, &mut r5522_first);
    }
    let hand = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Hand(PlayerId::from_index(0)))
            .len()
    };
    let before = hand(&driver);
    driver.idle_until(3, Step::End).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    assert_eq!(hand(&driver), before + usize::from(manifested));
}

#[rstest]
fn r5522_mana_maze_stops_the_last_color(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Mana Maze").unwrap()),
                ready(mountain()),
                ready(mountain()),
                ready(forest()),
            ],
        )
        .hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Lightning Bolt").unwrap(),
                instant("Green Trick", 0, &[mtg_core::color::Color::Green]),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe]).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    assert!(
        driver.cast_at("Lightning Bolt", &[foe]).is_err(),
        "a red spell after a red spell"
    );
    driver.cast("Green Trick").unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    assert_eq!(r5522_life(&driver, 1), 17);
}

#[rstest]
fn r5522_power_surge_counts_lands_untapped_as_the_turn_began(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] tapped_lands: usize,
) {
    let mut lands: Vec<_> = (0..3 - tapped_lands).map(|_| ready(mountain())).collect();
    lands.extend((0..tapped_lands).map(|_| tapped(mountain())));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Power Surge").unwrap())])
        .battlefield(1, lands);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(4, Step::Upkeep).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    let untapped = i32::try_from(3 - tapped_lands).unwrap();
    assert_eq!(r5522_life(&driver, 1), 20 - untapped);
}

#[rstest]
fn r5522_season_spares_attackers_and_creatures_that_couldnt_attack(
    #[values(2, 3, 4)] seats: usize,
) {
    use mtg_testkit::cards::{elf, free_creature, giant};
    use mtg_testkit::sick;
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(card("Season of the Witch").unwrap()),
                ready(giant()),
                ready(bear()),
                sick(elf()),
            ],
        )
        .battlefield(1, [ready(free_creature("Foe Guard", 0, 4))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.attack(&[("Craw Wurm", 1)]).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    driver.idle_until(3, Step::End).unwrap();
    r5522_run(&mut driver, &mut r5522_first);
    assert_eq!(r5522_count(&driver, "Craw Wurm"), 1, "it attacked");
    assert_eq!(
        r5522_count(&driver, "Grizzly Bears"),
        0,
        "it could attack and didn't"
    );
    assert_eq!(
        r5522_count(&driver, "Llanowar Elf"),
        1,
        "summoning sickness held it back"
    );
    assert_eq!(
        r5522_count(&driver, "Foe Guard"),
        1,
        "only the active player attacks"
    );
}

#[rstest]
fn r5522_glyph_of_delusion_marks_what_the_wall_blocked(#[values(2, 3, 4)] seats: usize) {
    use mtg_testkit::cards::{giant, island};
    let wall = Arc::new(
        CardDefinition::new("Stone Wall")
            .with_type(CardType::Creature)
            .with_subtype("Wall")
            .with_power_toughness(0, 8),
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(giant()), ready(bear()), ready(island())])
        .battlefield(1, [ready(wall)])
        .hand(0, [card("Glyph of Delusion").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Craw Wurm", 1), ("Grizzly Bears", 1)])
        .unwrap();
    for _ in 0..80 {
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::DeclareBlockers(_)) {
            driver.block(&[("Stone Wall", "Craw Wurm")]).unwrap();
        } else {
            driver.answer_passively().unwrap();
        }
    }
    driver.begin_cast("Glyph of Delusion").unwrap();
    let wall = driver.target_named("Stone Wall").unwrap();
    let mut offered: Vec<String> = Vec::new();
    for _ in 0..6 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) if options.legal.contains(&wall) => {
                driver.choose_targets(&[wall]).unwrap();
            }
            DecisionKind::ChooseTargets(options) => {
                offered = options
                    .legal
                    .iter()
                    .filter_map(|t| t.object())
                    .map(|o| driver.state().object(o.id).unwrap().card().name.clone())
                    .collect();
                let wurm = driver.target_named("Craw Wurm").unwrap();
                driver.choose_targets(&[wurm]).unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
                break;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(offered, ["Craw Wurm"], "only the creature the Wall blocked");
    r5522_run(&mut driver, &mut r5522_first);
    let wurm = driver.id_of("Craw Wurm").unwrap();
    let glyphs = driver
        .state()
        .object(wurm)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("glyph".to_owned()));
    assert_eq!(glyphs, 4, "its power");
}
