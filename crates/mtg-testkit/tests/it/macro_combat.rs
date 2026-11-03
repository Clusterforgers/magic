//! Goal: the combat and trigger keywords of the macro table play whole at tables of two, three and four seats as their rules say: flanking shrinks each blocker without it, soulshift returns a Spirit of its limit, bloodthirst and unleash decide how a creature enters, training counts only attackers of greater power, ingest exiles from the library of the player it hurts, and firebending's mana lasts through combat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::sync::Arc;

use crate::common::pid;
use crate::macro_keywords::{fight_on, life, macro_keyword, play, size, table};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, keyworded, keyworded_creature, plains};
use mtg_testkit::{Driver, ScriptError, with_counters};
use rstest::rstest;

/// The +1/+1 counters on the named permanent.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

/// The names of the cards in a zone, sorted.
fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut found: Vec<String> = driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    found.sort();
    found
}

#[rstest]
fn flanking_shrinks_each_blocker_without_flanking(
    #[values(2, 3, 4)] seats: usize,
    #[values((1, 0), (2, 0), (1, 1), (0, 1))] blockers: (usize, usize),
) -> Result<(), ScriptError> {
    let (plain, flanking) = blockers;
    let knight = keyworded("Knight", 1, 6, &[macro_keyword("flanking", 0)]);
    let mut walls = Vec::new();
    let mut blocks = Vec::new();
    for at in 0..plain {
        walls.push(free_creature(&format!("Plain {at}"), 0, 5));
        blocks.push((format!("Plain {at}"), "Knight"));
    }
    for at in 0..flanking {
        walls.push(keyworded(
            &format!("Flanker {at}"),
            0,
            5,
            &[macro_keyword("flanking", 0)],
        ));
        blocks.push((format!("Flanker {at}"), "Knight"));
    }
    let mut driver = Driver::from_scenario(table(seats, vec![knight], &walls))?;
    let chosen: Vec<(&str, &str)> = blocks.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    play(
        &mut driver,
        &["Knight"],
        &chosen,
        &|_, _| None,
        Step::EndCombat,
    )?;
    for at in 0..plain {
        assert_eq!(size(&driver, &format!("Plain {at}")), (-1, 4));
    }
    for at in 0..flanking {
        assert_eq!(size(&driver, &format!("Flanker {at}")), (0, 5));
    }
    Ok(())
}

#[rstest]
fn flanking_does_nothing_for_an_unblocked_creature_or_one_without_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] has_flanking: bool,
    #[values(true, false)] blocked: bool,
) -> Result<(), ScriptError> {
    if has_flanking && blocked {
        return Ok(());
    }
    let keywords = if has_flanking {
        vec![macro_keyword("flanking", 0)]
    } else {
        Vec::new()
    };
    let attacker = keyworded("Attacker", 1, 6, &keywords);
    let mut driver =
        Driver::from_scenario(table(seats, vec![attacker], &[free_creature("Wall", 0, 5)]))?;
    let blocks: &[(&str, &str)] = if blocked {
        &[("Wall", "Attacker")]
    } else {
        &[]
    };
    play(
        &mut driver,
        &["Attacker"],
        blocks,
        &|_, _| None,
        Step::EndCombat,
    )?;
    assert_eq!(size(&driver, "Wall"), (0, 5));
    Ok(())
}

/// A Spirit card of the mana value, or another creature type.
fn card_of(name: &str, kind: &str, value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(kind)
            .with_mana_cost(cost(value, &[]))
            .with_power_toughness(1, 1),
    )
}

/// What a soulshift trigger offered, and the answers the test gives it.
struct Shift<'a> {
    pick: &'a str,
    says_yes: bool,
    offered: RefCell<Option<Vec<String>>>,
}

impl Shift<'_> {
    fn answer(&self, driver: &Driver, decision: &Decision) -> Option<Answer> {
        if decision.player != pid(0) {
            return None;
        }
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let label = |target: &Target| match target {
                    Target::Object(object) => driver
                        .state()
                        .object(object.id)
                        .unwrap()
                        .card()
                        .name
                        .clone(),
                    Target::Player(_) => "player".to_owned(),
                };
                let mut offered: Vec<String> = options.legal.iter().map(label).collect();
                offered.sort();
                *self.offered.borrow_mut() = Some(offered);
                let chosen = options.legal.iter().find(|t| label(t) == self.pick)?;
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![*chosen])))
            }
            DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(self.says_yes))),
            _ => None,
        }
    }
}

#[rstest]
#[case::limit_one(1, &["Spirit One"], true)]
#[case::limit_two(2, &["Spirit One", "Spirit Two"], true)]
#[case::limit_four(4, &["Spirit Four", "Spirit One", "Spirit Two"], true)]
#[case::declined(2, &["Spirit One", "Spirit Two"], false)]
fn soulshift_returns_a_spirit_of_its_limit(
    #[values(2, 3, 4)] seats: usize,
    #[case] limit: u32,
    #[case] legal: &[&str],
    #[case] says_yes: bool,
) -> Result<(), ScriptError> {
    let shifter = keyworded("Shifter", 1, 1, &[macro_keyword("soulshift", limit)]);
    let scenario = table(seats, vec![shifter], &[free_creature("Wall", 3, 4)])
        .graveyard(
            0,
            [
                card_of("Spirit One", "Spirit", 1),
                card_of("Spirit Two", "Spirit", 2),
                card_of("Spirit Four", "Spirit", 4),
                card_of("Goblin One", "Goblin", 1),
            ],
        )
        .graveyard(1, [card_of("Their Spirit", "Spirit", 1)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let shift = Shift {
        pick: legal[0],
        says_yes,
        offered: RefCell::new(None),
    };
    play(
        &mut driver,
        &["Shifter"],
        &[("Wall", "Shifter")],
        &|d, decision| shift.answer(d, decision),
        Step::EndCombat,
    )?;
    let mut expected: Vec<String> = legal.iter().map(|name| (*name).to_owned()).collect();
    expected.sort();
    assert_eq!(shift.offered.take(), Some(expected));
    let hand = names(&driver, Zone::Hand(pid(0)));
    let graveyard = names(&driver, Zone::Graveyard(pid(0)));
    assert_eq!(hand.contains(&legal[0].to_owned()), says_yes);
    assert_eq!(graveyard.contains(&legal[0].to_owned()), !says_yes);
    assert!(graveyard.contains(&"Goblin One".to_owned()));
    assert_eq!(names(&driver, Zone::Graveyard(pid(1))).len(), 1);
    Ok(())
}

#[rstest]
fn soulshift_without_a_spirit_of_its_limit_does_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let shifter = keyworded("Shifter", 1, 1, &[macro_keyword("soulshift", 1)]);
    let scenario = table(seats, vec![shifter], &[free_creature("Wall", 3, 4)])
        .graveyard(0, [card_of("Spirit Four", "Spirit", 4)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let shift = Shift {
        pick: "Spirit Four",
        says_yes: true,
        offered: RefCell::new(None),
    };
    play(
        &mut driver,
        &["Shifter"],
        &[("Wall", "Shifter")],
        &|d, decision| shift.answer(d, decision),
        Step::EndCombat,
    )?;
    assert_eq!(shift.offered.take(), None);
    assert_eq!(names(&driver, Zone::Hand(pid(0))), Vec::<String>::new());
    Ok(())
}

/// Passes until the stack is empty, answering a yes-or-no question with `says_yes` and everything else passively.
fn settle(driver: &mut Driver, says_yes: bool) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(says_yes)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not settle");
}

#[rstest]
fn bloodthirst_counters_only_after_an_opponent_was_dealt_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
    #[values(true, false)] damaged: bool,
) -> Result<(), ScriptError> {
    let thirsty = keyworded_creature(
        "Thirsty",
        1,
        &[],
        1,
        1,
        &[macro_keyword("bloodthirst", amount)],
    );
    let scenario = table(
        seats,
        vec![free_creature("Raider", 1, 1), plains()],
        &[free_creature("Wall", 0, 4)],
    )
    .hand(0, [thirsty]);
    let mut driver = Driver::from_scenario(scenario)?;
    if damaged {
        play(&mut driver, &["Raider"], &[], &|_, _| None, Step::Main2)?;
        assert_eq!(life(&driver, 1), 19);
    } else {
        fight_on(&mut driver, Step::Main2)?;
    }
    driver.cast("Thirsty")?;
    settle(&mut driver, true)?;
    let bonus = if damaged {
        i32::try_from(amount).unwrap()
    } else {
        0
    };
    assert_eq!(size(&driver, "Thirsty"), (1 + bonus, 1 + bonus));
    Ok(())
}

#[rstest]
#[case::lesser_mate(2, Some(1), 0)]
#[case::equal_mate(2, Some(2), 0)]
#[case::greater_mate(2, Some(3), 1)]
#[case::far_greater_mate(1, Some(9), 1)]
#[case::alone(2, None, 0)]
fn training_counts_only_another_attacker_of_greater_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] power: i32,
    #[case] mate: Option<i32>,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let trainee = keyworded("Trainee", power, 5, &[macro_keyword("training", 0)]);
    let mut attackers = vec![trainee];
    if let Some(mate) = mate {
        attackers.push(free_creature("Mate", mate, 5));
    }
    let mut driver =
        Driver::from_scenario(table(seats, attackers, &[free_creature("Wall", 0, 9)]))?;
    let attacking: Vec<&str> = if mate.is_some() {
        vec!["Trainee", "Mate"]
    } else {
        vec!["Trainee"]
    };
    play(&mut driver, &attacking, &[], &|_, _| None, Step::EndCombat)?;
    assert_eq!(size(&driver, "Trainee"), (power + gained, 5 + gained));
    Ok(())
}

#[rstest]
fn training_ignores_a_stronger_creature_that_does_not_attack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let attackers = vec![
        keyworded("Trainee", 1, 5, &[macro_keyword("training", 0)]),
        free_creature("Bystander", 7, 7),
    ];
    let mut driver =
        Driver::from_scenario(table(seats, attackers, &[free_creature("Wall", 0, 9)]))?;
    play(
        &mut driver,
        &["Trainee"],
        &[],
        &|_, _| None,
        Step::EndCombat,
    )?;
    assert_eq!(counters(&driver, "Trainee"), 0);
    Ok(())
}

#[rstest]
fn two_trainees_each_look_for_a_stronger_attacker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let attackers = vec![
        keyworded("Small", 1, 5, &[macro_keyword("training", 0)]),
        keyworded("Large", 3, 5, &[macro_keyword("training", 0)]),
    ];
    let mut driver =
        Driver::from_scenario(table(seats, attackers, &[free_creature("Wall", 0, 9)]))?;
    play(
        &mut driver,
        &["Small", "Large"],
        &[],
        &|_, _| None,
        Step::EndCombat,
    )?;
    assert_eq!(
        (counters(&driver, "Small"), counters(&driver, "Large")),
        (1, 0)
    );
    Ok(())
}

#[rstest]
fn ingest_exiles_the_top_card_of_the_player_it_hurts(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] ingesters: usize,
    #[values(true, false)] blocked: bool,
) -> Result<(), ScriptError> {
    let attackers: Vec<_> = (0..ingesters)
        .map(|at| {
            keyworded(
                &format!("Ingester {at}"),
                1,
                3,
                &[macro_keyword("ingest", 0)],
            )
        })
        .collect();
    let mut scenario = table(seats, attackers, &[free_creature("Wall", 0, 9)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let names_of: Vec<String> = (0..ingesters).map(|at| format!("Ingester {at}")).collect();
    let attacking: Vec<&str> = names_of.iter().map(String::as_str).collect();
    let blocks: Vec<(&str, &str)> = if blocked {
        vec![("Wall", "Ingester 0")]
    } else {
        Vec::new()
    };
    play(
        &mut driver,
        &attacking,
        &blocks,
        &|_, _| None,
        Step::EndCombat,
    )?;
    let hurt = ingesters - usize::from(blocked);
    assert_eq!(driver.state().zone(Zone::Library(pid(1))).len(), 6 - hurt);
    assert_eq!(driver.state().zone(Zone::Exile).len(), hurt);
    for seat in [0, 2, 3].into_iter().filter(|seat| *seat < seats) {
        assert_eq!(
            driver.state().zone(Zone::Library(pid(seat))).len(),
            6,
            "seat {seat}"
        );
    }
    Ok(())
}

#[rstest]
fn firebending_mana_lasts_through_combat_and_not_past_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] amount: u32,
) -> Result<(), ScriptError> {
    let bender = keyworded("Bender", 2, 2, &[macro_keyword("firebending", amount)]);
    let mut driver =
        Driver::from_scenario(table(seats, vec![bender], &[free_creature("Wall", 0, 9)]))?;
    play(&mut driver, &["Bender"], &[], &|_, _| None, Step::EndCombat)?;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().mana().total(),
        amount
    );
    fight_on(&mut driver, Step::Main2)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 0);
    Ok(())
}

#[rstest]
fn unleash_asks_as_the_creature_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] says_yes: bool,
) -> Result<(), ScriptError> {
    let unleasher = keyworded_creature("Unleasher", 1, &[], 2, 2, &[macro_keyword("unleash", 0)]);
    let scenario =
        table(seats, vec![plains()], &[free_creature("Wall", 0, 4)]).hand(0, [unleasher]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Unleasher")?;
    let mut asked = false;
    for _ in 0..20 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                assert_eq!(driver.decision()?.player, pid(0));
                asked = true;
                driver.answer(Answer::YesNo(YesNoChoice(says_yes)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(asked);
    let bonus = i32::from(says_yes);
    assert_eq!(size(&driver, "Unleasher"), (2 + bonus, 2 + bonus));
    assert_eq!(counters(&driver, "Unleasher"), u32::from(says_yes));
    Ok(())
}

#[rstest]
fn a_creature_with_an_unleash_counter_cannot_block(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] has_counter: bool,
) -> Result<(), ScriptError> {
    let unleasher = keyworded("Unleasher", 2, 2, &[macro_keyword("unleash", 0)]);
    let mut theirs = mtg_testkit::ready(unleasher);
    if has_counter {
        theirs = with_counters(theirs, CounterKind::PlusOnePlusOne, 1);
    }
    let mut scenario = mtg_testkit::Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [mtg_testkit::ready(free_creature("Raider", 1, 1))])
        .battlefield(1, [theirs]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [mtg_testkit::ready(free_creature("Wall", 0, 4))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let offered = RefCell::new(None);
    play(
        &mut driver,
        &["Raider"],
        &[],
        &|d, decision| {
            if let DecisionKind::DeclareBlockers(options) = &decision.kind
                && decision.player == pid(1)
            {
                let id = d.id_of("Unleasher").unwrap();
                *offered.borrow_mut() = Some(options.blockers.iter().any(|b| b.object == id));
            }
            None
        },
        Step::EndCombat,
    )?;
    assert_eq!(offered.take().unwrap_or(false), !has_counter);
    Ok(())
}
