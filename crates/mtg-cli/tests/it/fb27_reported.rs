//! Goal: the cards players reported as reading wrong play as their Oracle text and rulings say, at two to four seats: a source of any kind triggers Ghyrson Starn, each creature and each planeswalker of each opponent is hit once by End the Festivities and Goblin Chainwhirler, empower Jace makes one Jace token that keeps its counters, Toxin, Homing and Obelisk triggers and grants, a bonus counted as it resolves, X spells, draws and life of Peer into the Abyss, Dark Confidant, The One Ring, Memory Deluge, Torment of Hailfire and Crackle with Power, and Nazgûl whole.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::builtin;
use mtg_cards::card::Card;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics;
use mtg_engine::choose::{NumberChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_import::scryfall::cards;
use mtg_testkit::cards::{burn, creature, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::{self, printed_card};
use mtg_testkit::loyalty_cards::pyre_walker;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::drafts::dir;

/// The definition the engine runs for the draft `slug` once it is read and approved.
pub fn draft(slug: &str) -> Arc<CardDefinition> {
    let text = fs::read_to_string(dir().join(format!("{slug}.json"))).unwrap();
    let card: Card = serde_json::from_str(&text).unwrap();
    let record = fs::read_to_string(dir().join("fixtures").join(format!("{slug}.json"))).unwrap();
    let record = cards(format!("[{record}]").as_bytes())
        .next()
        .unwrap()
        .unwrap();
    let printed = printed_card(&record);
    let caps = mtg_engine::caps();
    let mut defined =
        mtg_cli::drafts::read_definitions(builtin::library().unwrap(), &caps, &[(&printed, &card)]);
    Arc::new(defined.remove(0).unwrap())
}

/// Answers passively until the stack is empty and seat 0 holds priority again.
pub fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && decision.player == PlayerId::from_index(0)
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    panic!("the stack did not settle in 200 answers");
}

/// The damage marked on the permanent `name`.
pub fn damage_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, mtg_engine::object::PermanentState::damage)
}

/// The counters of `kind` on the permanent `name`, or `None` once it left the battlefield.
pub fn counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> Option<u32> {
    let id = driver.id_of(name).ok()?;
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(kind))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

#[rstest]
#[case::exactly_one_from_a_spell(1, 17)]
#[case::two_from_a_spell(2, 18)]
fn ghyrson_starn_answers_one_damage_from_a_spell_you_control(
    #[values(2, 4)] seats: usize,
    #[case] amount: i32,
    #[case] life_after: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [burn("Ping", amount)])
            .battlefield(
                0,
                [ready(draft("ghyrson_starn_kelermorph")), ready(mountain())],
            ),
    )?;
    let target = driver.target_player(1);
    driver.cast_at("Ping", &[target])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), life_after);
    Ok(())
}

#[rstest]
fn end_the_festivities_hits_each_opponent_and_each_creature_and_planeswalker_they_control(
    #[values(2, 3, 4)] seats: usize,
    #[values("end_the_festivities")] slug: &str,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [draft(slug)])
        .battlefield(0, [ready(mountain()), ready(free_creature("Mine", 2, 2))]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Theirs {seat}"), 2, 2)),
                ready(pyre_walker().unwrap()),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("End the Festivities")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20);
    assert_eq!(damage_on(&driver, "Mine"), 0);
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 19);
        assert_eq!(damage_on(&driver, &format!("Theirs {seat}")), 1);
    }
    let walkers: Vec<u32> = driver
        .state()
        .objects()
        .filter(|object| object.card().name == "Pyre Walker")
        .filter_map(|object| {
            object
                .permanent()
                .map(|p| p.counters_of(&CounterKind::Loyalty))
        })
        .collect();
    assert_eq!(walkers, vec![2; seats - 1]);
    Ok(())
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(seat)))
        .len()
}

/// A library of `count` vanilla creatures, each of mana value `value`.
fn library(count: usize, value: u32) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|n| creature(&format!("Card {n}"), value, &[], 1, 1))
        .collect()
}

#[rstest]
fn goblin_chainwhirler_hits_each_opponent_and_each_creature_and_planeswalker_they_control(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, [draft("goblin_chainwhirler")])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(free_creature("Mine", 1, 1)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Theirs {seat}"), 2, 2)),
                ready(pyre_walker().unwrap()),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Goblin Chainwhirler")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20);
    assert_eq!(damage_on(&driver, "Mine"), 0);
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 19);
        assert_eq!(damage_on(&driver, &format!("Theirs {seat}")), 1);
    }
    Ok(())
}

#[rstest]
#[case::one_copy(1, 5)]
#[case::two_copies_share_one_token(2, 10)]
fn way_of_the_mentor_empowers_one_jace_token(
    #[values(2, 4)] seats: usize,
    #[case] copies: usize,
    #[case] loyalty: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, (0..copies).map(|_| draft("way_of_the_mentor")))
            .battlefield(0, (0..3 * copies).map(|_| ready(plains()))),
    )?;
    for _ in 0..copies {
        driver.cast("Way of the Mentor")?;
        settle(&mut driver)?;
    }
    let jaces: Vec<u32> = driver
        .state()
        .objects()
        .filter(|object| object.card().name == "Jace")
        .filter_map(|object| {
            object
                .permanent()
                .map(|p| p.counters_of(&CounterKind::Loyalty))
        })
        .collect();
    assert_eq!(jaces, vec![loyalty]);
    Ok(())
}

#[rstest]
#[case::even(10, 5)]
#[case::odd_rounds_up(9, 5)]
fn peer_into_the_abyss_draws_half_the_library_and_takes_half_the_life_rounded_up(
    #[values(2, 4)] seats: usize,
    #[case] cards: usize,
    #[case] drawn: usize,
    #[values(20, 7)] start: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [draft("peer_into_the_abyss")])
            .battlefield(0, (0..7).map(|_| ready(swamp())))
            .library(1, library(cards, 1))
            .life(1, start),
    )?;
    let before = hand_size(&driver, 1);
    let target = driver.target_player(1);
    driver.cast_at("Peer into the Abyss", &[target])?;
    settle(&mut driver)?;
    assert_eq!(hand_size(&driver, 1), before + drawn);
    assert_eq!(life(&driver, 1), start - (start + 1) / 2);
    Ok(())
}

#[rstest]
#[case::zero(0)]
#[case::three(3)]
fn dark_confidant_puts_the_top_card_into_hand_and_takes_its_mana_value(
    #[values(2, 4)] seats: usize,
    #[case] value: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(3, seats - 1, Step::End)
            .battlefield(0, [ready(draft("dark_confidant"))])
            .library(0, library(3, value)),
    )?;
    let before = hand_size(&driver, 0);
    driver.pass_until(Step::Upkeep)?;
    settle(&mut driver)?;
    assert_eq!(driver.state().turn().step(), Step::Upkeep);
    assert_eq!(hand_size(&driver, 0), before + 1);
    assert_eq!(life(&driver, 0), 20 - i32::try_from(value).unwrap());
    Ok(())
}

#[rstest]
#[case::first(0, 1)]
#[case::third(2, 3)]
fn the_one_ring_draws_one_card_for_each_burden_counter_after_adding_one(
    #[values(2, 4)] seats: usize,
    #[case] burdens: u32,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let burden = CounterKind::Other("burden".into());
    let ring = with_counters(ready(draft("the_one_ring")), burden.clone(), burdens);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ring])
            .library(0, library(10, 1)),
    )?;
    let before = hand_size(&driver, 0);
    driver.begin_activation("The One Ring")?;
    settle(&mut driver)?;
    assert_eq!(hand_size(&driver, 0), before + drawn);
    assert_eq!(
        counters_on(&driver, "The One Ring", &burden),
        Some(burdens + 1)
    );
    Ok(())
}

#[rstest]
#[case::blocked_by_a_tough_wall(true)]
#[case::unblocked(false)]
fn toxin_sliver_destroys_a_creature_it_deals_combat_damage_to(
    #[values(2, 4)] seats: usize,
    #[case] blocked: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(corpus::card("Toxin Sliver").unwrap())])
            .battlefield(1, [ready(free_creature("Wall", 0, 7))]),
    )?;
    driver.attack(&[("Toxin Sliver", 1)])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::DeclareBlockers(_))
            && decision.player == PlayerId::from_index(1)
        {
            if blocked {
                driver.block(&[("Wall", "Toxin Sliver")])?;
            } else {
                driver.answer(Answer::DeclareBlockers(Vec::new()))?;
            }
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    driver.idle_until(2, Step::EndCombat)?;
    let wall_alive = driver.id_of("Wall").ok().and_then(|id| {
        driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
    });
    assert_eq!(wall_alive.is_none(), blocked);
    assert_eq!(life(&driver, 1), if blocked { 20 } else { 17 });
    Ok(())
}

/// Seat 0 attacks seat 1 with `attacker` and seat 1 blocks with `blockers`; answers until combat ends.
fn fight(
    driver: &mut Driver,
    attacker: &str,
    blockers: &[(&str, &str)],
) -> Result<(), ScriptError> {
    driver.attack(&[(attacker, 1)])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::DeclareBlockers(_))
            && decision.player == PlayerId::from_index(1)
        {
            driver.block(blockers)?;
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    driver.idle_until(2, Step::EndCombat)
}

#[rstest]
#[case::blocked(true, 21, 19)]
#[case::unblocked(false, 20, 19)]
fn obelisk_spider_drains_when_you_put_minus_counters_on_a_creature(
    #[values(2, 4)] seats: usize,
    #[case] blocked: bool,
    #[case] mine: i32,
    #[case] theirs: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(draft("obelisk_spider"))])
            .battlefield(1, [ready(free_creature("Wall", 0, 7))]),
    )?;
    let blocks: &[(&str, &str)] = if blocked {
        &[("Wall", "Obelisk Spider")]
    } else {
        &[]
    };
    fight(&mut driver, "Obelisk Spider", blocks)?;
    assert_eq!(life(&driver, 0), mine);
    // Unblocked, the spider hits seat 1 for 1 and puts no counter.
    assert_eq!(life(&driver, 1), theirs);
    for seat in 2..seats {
        assert_eq!(life(&driver, seat), if blocked { 19 } else { 20 });
    }
    Ok(())
}

/// The attack bonus of Butch `DeLoria` counts the Rogues and Snakes as it resolves (CR 611.2c): a creature that becomes a Rogue
/// later in the turn adds nothing.
#[rstest]
#[case::rogue_before_the_attack(true, 3)]
#[case::rogue_after_the_bonus(false, 2)]
fn butch_deloria_counts_rogues_as_the_bonus_resolves(
    #[case] before: bool,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    const BUTCH: &str = "Butch DeLoria, Tunnel Snake";
    let mut driver = Driver::from_scenario(Scenario::new(2).at(2, 0, Step::Main1).battlefield(
        0,
        [
            ready(draft("butch_deloria_tunnel_snake")),
            ready(free_creature("Mate", 1, 1)),
            ready(swamp()),
            ready(swamp()),
        ],
    ))?;
    let rogue = |driver: &mut Driver| -> Result<(), ScriptError> {
        driver.begin_activation(BUTCH)?;
        let mate = driver.target_named("Mate")?;
        driver.choose_targets(&[mate])?;
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
        settle(driver)
    };
    if before {
        rogue(&mut driver)?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(BUTCH, 1)])?;
    settle(&mut driver)?;
    if !before {
        rogue(&mut driver)?;
    }
    let id = driver.id_of(BUTCH)?;
    let derived = characteristics(driver.state(), id).unwrap();
    assert_eq!(derived.power, Some(power));
    Ok(())
}

#[rstest]
#[case::with_homing_sliver(true)]
#[case::without(false)]
fn homing_sliver_gives_each_sliver_card_in_hand_slivercycling(
    #[values(2, 4)] seats: usize,
    #[case] homing: bool,
) -> Result<(), ScriptError> {
    let toxin = || corpus::card("Toxin Sliver").unwrap();
    let mut permanents = vec![ready(mountain()), ready(mountain()), ready(mountain())];
    if homing {
        permanents.push(ready(draft("homing_sliver")));
    }
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [toxin()])
            .library(0, [toxin(), free_creature("Filler", 1, 1)])
            .battlefield(0, permanents),
    )?;
    let offered = driver.offered_abilities("Toxin Sliver")?;
    assert_eq!(!offered.is_empty(), homing, "{offered:?}");
    Ok(())
}

/// Nazgûl plays whole now that its deck rule is a rule: the Ring tempts you as it enters and each Wraith you control gets a
/// +1/+1 counter for each Nazgûl that sees the temptation, the one already out too.
#[rstest]
#[case::alone(0, vec![1])]
#[case::beside_another_both_trigger(1, vec![2, 2])]
fn nazgul_tempts_and_grows_each_wraith(
    #[values(2, 4)] seats: usize,
    #[case] out: usize,
    #[case] counters: Vec<u32>,
) -> Result<(), ScriptError> {
    let nazgul = || corpus::card("Nazgûl").unwrap();
    let mut permanents: Vec<_> = (0..3).map(|_| ready(swamp())).collect();
    permanents.extend((0..out).map(|_| ready(nazgul())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [nazgul()])
            .battlefield(0, permanents),
    )?;
    driver.cast("Nazgûl")?;
    settle(&mut driver)?;
    let grown: Vec<u32> = driver
        .state()
        .objects()
        .filter(|object| object.card().name == "Nazgûl")
        .filter_map(|object| {
            object
                .permanent()
                .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        })
        .collect();
    assert_eq!(grown, counters);
    assert_eq!(nazgul().singleton_limit(), Some(9));
    Ok(())
}

/// Seat 0 casts `name` announcing `x` and choosing `targets`, pays automatically and lets everything resolve.
fn cast_with(
    driver: &mut Driver,
    name: &str,
    x: u32,
    targets: &[Target],
) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        let answer = match decision.kind {
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(targets.to_vec()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => break,
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    settle(driver)
}

#[rstest]
#[case::once(1, 3)]
#[case::twice(2, 6)]
fn torment_of_hailfire_takes_3_life_each_time_from_an_opponent_with_nothing_to_give(
    #[values(2, 4)] seats: usize,
    #[case] x: u32,
    #[case] lost: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [draft("torment_of_hailfire")])
            .battlefield(0, (0..4).map(|_| ready(swamp()))),
    )?;
    cast_with(&mut driver, "Torment of Hailfire", x, &[])?;
    assert_eq!(life(&driver, 0), 20);
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20 - lost);
    }
    Ok(())
}

#[rstest]
fn crackle_with_power_deals_five_times_x_to_each_target(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [draft("crackle_with_power")])
            .battlefield(0, (0..5).map(|_| ready(mountain()))),
    )?;
    let target = driver.target_player(1);
    cast_with(&mut driver, "Crackle with Power", 1, &[target])?;
    assert_eq!(life(&driver, 1), 15);
    Ok(())
}

#[rstest]
fn memory_deluge_keeps_two_of_the_cards_its_mana_looks_at(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [draft("memory_deluge")])
            .battlefield(0, (0..4).map(|_| ready(island())))
            .library(0, library(10, 1)),
    )?;
    let before = hand_size(&driver, 0);
    cast_with(&mut driver, "Memory Deluge", 0, &[])?;
    // The deluge left the hand and two cards came in; the library keeps its other cards.
    assert_eq!(hand_size(&driver, 0), before + 1);
    assert_eq!(
        driver
            .state()
            .zone(Zone::Library(PlayerId::from_index(0)))
            .len(),
        8
    );
    Ok(())
}

/// The card and its printed side of the draft `slug`.
fn draft_card(slug: &str) -> (Card, mtg_cards::PrintedCard) {
    let text = fs::read_to_string(dir().join(format!("{slug}.json"))).unwrap();
    let record = fs::read_to_string(dir().join("fixtures").join(format!("{slug}.json"))).unwrap();
    let record = cards(format!("[{record}]").as_bytes())
        .next()
        .unwrap()
        .unwrap();
    (serde_json::from_str(&text).unwrap(), printed_card(&record))
}

/// The lines of the read sheet of the draft `slug`, as a player reads them beside the card.
fn sheet_lines(slug: &str) -> Vec<String> {
    let (card, printed) = draft_card(slug);
    let mut lines = Vec::new();
    for row in mtg_cards::read::sheet(&card, &printed).rows {
        lines.push(format!("[{:?}] {}", row.state, row.printed));
        lines.extend(row.wording.map(|wording| format!("  wording: {wording}")));
        lines.extend(
            row.steps
                .iter()
                .map(|step| format!("  {}{}", "  ".repeat(usize::from(step.depth)), step.text)),
        );
    }
    lines
}

#[test]
#[ignore = "prints the read sheets of the slugs in FB27_SHEETS"]
fn print_sheets() {
    for slug in std::env::var("FB27_SHEETS").unwrap().split(',') {
        println!("== {slug}\n{}", sheet_lines(slug).join("\n"));
    }
}
