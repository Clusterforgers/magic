//! Goal: the cards that alone held back two precon cards each in the third round play as printed at tables of two, three and four seats: the life an opponent's creature costs its controller (Blood Seeker, Suture Priest), the free cast from the hand (Baral's and Rishkar's Expertise), a creature that deals damage equal to its power to another (Super Combo), another creature that returns at the end step (Rienne), a creature dealt damage by this creature that dies (Predator Ooze, Vampiric Dragon), shroud after an attach (Silver Shroud Costume) and a card that returns itself from the graveyard (Silversmote Ghoul).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{on_battlefield, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::{Color, ColorSet};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{
    burn, creature, forest, free_creature, gain_life, island, mountain, plains, swamp,
};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// How many cards called `name` are in `zone`.
fn in_zone(driver: &Driver, zone: Zone, name: &str) -> usize {
    crate::common::names(driver.state(), zone)
        .iter()
        .filter(|found| found.as_str() == name)
        .count()
}

/// The `+1/+1` counters on the named permanent.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = on_battlefield(driver.state(), name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

/// Passes priority until the stack is empty, answering a yes-or-no question with `yes`, a choice of cards with the first
/// card (`pick` names the card instead when it can) and the targets of a trigger with the first option.
fn settle(driver: &mut Driver, yes: bool, pick: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let wanted = options.from.iter().copied().find(|id| {
                    pick.is_none_or(|name| crate::common::name_of(driver.state(), *id) == name)
                });
                driver.answer(Answer::ChooseCards(CardsChoice(
                    wanted.into_iter().collect(),
                )))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn the_controller_of_a_creature_that_enters_loses_life_when_the_watcher_chooses(
    #[values("Blood Seeker", "Suture Priest")] watcher: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] entrant: usize,
    #[values(true, false)] yes: bool,
) -> Result<(), ScriptError> {
    let entrant = 1 + (entrant - 1) % (seats - 1);
    let mut scenario = Scenario::new(seats)
        .at(1, entrant, Step::Main1)
        .hand(entrant, [free_creature("Newcomer", 1, 1)])
        .battlefield(0, [ready(card(watcher).unwrap())]);
    for seat in (1..seats).filter(|seat| *seat != entrant) {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Newcomer")?;
    // The first question is the one for the watcher's trigger, never for the player who cast the creature.
    for _ in 0..6 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            assert_eq!(driver.decision()?.player, pid(0));
            break;
        }
        driver.answer_passively()?;
    }
    settle(&mut driver, yes, None)?;
    for seat in 0..seats {
        let expected = if seat == entrant && yes { 19 } else { 20 };
        assert_eq!(life(&driver, seat), expected, "seat {seat}");
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_of_the_watcher_costs_nobody_life(
    #[values("Blood Seeker", "Suture Priest")] watcher: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [free_creature("Newcomer", 1, 1)])
        .battlefield(0, [ready(card(watcher).unwrap())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Newcomer")?;
    settle(&mut driver, true, None)?;
    // Only Suture Priest cares: its other ability gains its controller 1 life.
    for seat in 0..seats {
        let expected = if seat == 0 && watcher == "Suture Priest" {
            21
        } else {
            20
        };
        assert_eq!(life(&driver, seat), expected, "seat {seat}");
    }
    Ok(())
}

/// A spell of the given cost that is a creature for the table.
fn spell_costing(name: &str, generic: u32) -> Arc<CardDefinition> {
    creature(name, generic, &[], 1, 1)
}

#[rstest]
fn the_expertise_spells_cast_a_cheap_spell_from_the_hand_for_free(
    #[values("Baral's Expertise", "Rishkar's Expertise")] expertise: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values("Cheap", "Edge", "None")] chosen: &str,
) -> Result<(), ScriptError> {
    let baral = expertise.starts_with("Baral");
    let (lands, limit): (Vec<_>, u32) = if baral {
        ((0..5).map(|_| ready(island())).collect(), 4)
    } else {
        ((0..6).map(|_| ready(forest())).collect(), 5)
    };
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [
                card(expertise).unwrap(),
                spell_costing("Cheap", 1),
                spell_costing("Edge", limit),
                spell_costing("Pricey", limit + 1),
                island(),
            ],
        )
        .battlefield(0, lands);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(expertise)?;
    // Up to three targets: none are chosen.
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.choose_targets(&[])?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        if let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() {
            // Only a nonland card of the limit or less is offered: not the land, not the spell that costs one more.
            let mut offered: Vec<String> = options
                .from
                .iter()
                .map(|id| crate::common::name_of(driver.state(), *id))
                .collect();
            offered.sort();
            assert_eq!(offered, ["Cheap", "Edge"], "{expertise}");
            assert_eq!(driver.decision()?.player, pid(0));
            let picked: Vec<_> = options
                .from
                .iter()
                .copied()
                .filter(|id| crate::common::name_of(driver.state(), *id) == chosen)
                .collect();
            driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
        } else if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            // The player is asked whether to cast the card they chose.
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    // The spell was cast for free: every land was spent on the Expertise and the creature is on the battlefield all the same.
    let entered = |name: &str| in_zone(&driver, Zone::Battlefield, name);
    assert_eq!(entered("Cheap"), usize::from(chosen == "Cheap"));
    assert_eq!(entered("Edge"), usize::from(chosen == "Edge"));
    assert_eq!(entered("Pricey"), 0);
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), "Pricey"), 1);
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), "Island"), 1);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn super_combo_has_one_creature_deal_damage_equal_to_its_power_to_another(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3, 5)] power: i32,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Super Combo").unwrap()])
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                ready(free_creature("Mine", power, 4)),
            ],
        );
    scenario = scenario.battlefield(foe, [ready(free_creature("Theirs", 2, 3))]);
    for seat in 1..foe {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let targets = [driver.target_named("Mine")?, driver.target_named("Theirs")?];
    driver.begin_cast("Super Combo")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, false, None)?;
    // The creature of the first target deals the damage and takes none back (this is no fight).
    assert_eq!(
        in_zone(&driver, Zone::Battlefield, "Theirs"),
        usize::from(power < 3)
    );
    let damage = |name: &str| {
        let id = on_battlefield(driver.state(), name).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .damage()
    };
    assert_eq!(damage("Mine"), 0);
    if power < 3 {
        assert_eq!(damage("Theirs"), u32::try_from(power).unwrap());
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_that_left_before_super_combo_resolved_deals_no_damage(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Super Combo").unwrap()])
        .hand(1, [burn("Bolt", 4)])
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                ready(free_creature("Mine", 5, 4)),
            ],
        )
        .battlefield(1, [ready(mountain()), ready(free_creature("Theirs", 2, 3))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let targets = [driver.target_named("Mine")?, driver.target_named("Theirs")?];
    driver.begin_cast("Super Combo")?;
    driver.choose_targets(&targets[..1])?;
    driver.choose_targets(&targets[1..])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    // In response, the opponent burns the creature that was to deal the damage.
    crate::common::removal::pass_to(&mut driver, 1)?;
    driver.cast_at("Bolt", &[targets[0]])?;
    settle(&mut driver, false, None)?;
    assert_eq!(in_zone(&driver, Zone::Battlefield, "Mine"), 0);
    let theirs = on_battlefield(driver.state(), "Theirs").unwrap();
    assert_eq!(
        driver
            .state()
            .object(theirs)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        0
    );
    assert_replays(&driver);
    Ok(())
}

/// Plays on, declaring no attack, until the end step begins.
fn to_the_end_step(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().turn().step() == Step::End {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the end step".into()))
}

/// A multicolored creature (red and green) and a mono-colored one.
fn gold(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_color_indicator(ColorSet::EMPTY.with(Color::Red).with(Color::Green)),
    )
}

#[rstest]
fn another_multicolored_creature_that_dies_returns_to_its_owners_hand_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values("Gold", "Mono", "Rienne, Angel of Rebirth")] victim: &str,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 7)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(card("Rienne, Angel of Rebirth").unwrap()),
                ready(gold("Gold")),
                ready(free_creature("Mono", 2, 2)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(victim)?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver, false, None)?;
    assert_eq!(in_zone(&driver, Zone::Battlefield, victim), 0);
    assert_eq!(in_zone(&driver, Zone::Graveyard(pid(0)), victim), 1);
    // It waits in the graveyard until the beginning of the next end step.
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), victim), 0);
    to_the_end_step(&mut driver)?;
    settle(&mut driver, false, None)?;
    let returned = usize::from(victim == "Gold");
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), victim), returned);
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0)), victim),
        1 - returned
    );
    assert_replays(&driver);
    Ok(())
}

/// The first ping of the Dragon at a victim, then the victim burned by the opponent: the creature the Dragon hurt dies.
fn ping_then_burn(
    seats: usize,
    ping: bool,
    next_turn: bool,
    dragon: &str,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card(dragon).unwrap()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(mountain()), ready(free_creature("Victim", 1, 3))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if ping {
        driver.begin_activation(dragon)?;
        let victim = driver.target_named("Victim")?;
        driver.choose_targets(&[victim])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        settle(&mut driver, false, None)?;
    }
    if next_turn {
        driver.idle_until(2, Step::Main1)?;
    } else {
        crate::common::removal::pass_to(&mut driver, 1)?;
    }
    let victim = driver.target_named("Victim")?;
    driver.cast_at("Bolt", &[victim])?;
    settle(&mut driver, false, None)?;
    Ok(driver)
}

#[rstest]
fn a_creature_the_dragon_hurt_this_turn_adds_a_counter_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] ping: bool,
) -> Result<(), ScriptError> {
    let driver = ping_then_burn(seats, ping, false, "Vampiric Dragon")?;
    assert_eq!(in_zone(&driver, Zone::Battlefield, "Victim"), 0);
    assert_eq!(counters(&driver, "Vampiric Dragon"), u32::from(ping));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn damage_of_an_earlier_turn_does_not_count(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = ping_then_burn(seats, true, true, "Vampiric Dragon")?;
    assert_eq!(in_zone(&driver, Zone::Battlefield, "Victim"), 0);
    assert_eq!(counters(&driver, "Vampiric Dragon"), 0);
    Ok(())
}

#[rstest]
fn predator_ooze_grows_when_it_attacks_and_when_a_creature_it_hurt_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] toughness: i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Predator Ooze").unwrap())])
        .battlefield(1, [ready(free_creature("Blocker", 1, toughness))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Predator Ooze", 1)])?;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[("Blocker", "Predator Ooze")])?;
            }
            _ if driver.state().turn().step() == Step::Main2 => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    // One counter for the attack; a second only when the 1/1 Ooze killed the blocker.
    let dies = toughness == 1;
    assert_eq!(
        in_zone(&driver, Zone::Battlefield, "Blocker"),
        usize::from(!dies)
    );
    assert_eq!(counters(&driver, "Predator Ooze"), 1 + u32::from(dies));
    assert_replays(&driver);
    Ok(())
}

/// A Vampire creature with a size.
fn vampire(name: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Vampire")
            .with_power_toughness(power, toughness),
    )
}

#[rstest]
fn the_vampire_that_deals_combat_damage_to_a_player_gets_the_counter(
    #[values("Rakish Heir", "Stensia Masquerade")] watcher: &str,
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] blocked: bool,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let table = vec![
        ready(vampire("Fang", 3, 3)),
        ready(free_creature("Bruiser", 2, 2)),
        ready(card(watcher).unwrap()),
    ];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::DeclareAttackers)
        .battlefield(0, table)
        .battlefield(foe, [ready(free_creature("Wall", 0, 9))]);
    for seat in 1..foe {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let mut attackers = vec![("Fang", foe), ("Bruiser", foe)];
    if watcher == "Rakish Heir" {
        attackers.push(("Rakish Heir", foe));
    }
    driver.attack(&attackers)?;
    for _ in 0..80 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                let blocks: &[(&str, &str)] = if blocked { &[("Wall", "Fang")] } else { &[] };
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    // Only a Vampire that got through to the player earns the counter, and the counter is its own.
    assert_eq!(counters(&driver, "Fang"), u32::from(!blocked));
    assert_eq!(counters(&driver, "Bruiser"), 0);
    if watcher == "Rakish Heir" {
        assert_eq!(counters(&driver, "Rakish Heir"), 1);
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn silver_shroud_costume_attaches_and_gives_shroud_for_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Silver Shroud Costume").unwrap()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(plains()),
                ready(free_creature("Wearer", 2, 2)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Silver Shroud Costume")?;
    for _ in 0..6 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    let wearer = driver.target_named("Wearer")?;
    driver.choose_targets(&[wearer])?;
    settle(&mut driver, false, None)?;
    let costume = driver.id_of("Silver Shroud Costume")?;
    let wearer = driver.id_of("Wearer")?;
    assert_eq!(driver.state().host_of(costume), Some(wearer));
    assert!(mtg_engine::keywords::has_keyword(
        driver.state(),
        wearer,
        &Keyword::Shroud
    ));
    // The shroud ends with the turn.
    driver.idle_until(2, Step::Upkeep)?;
    let wearer = driver.id_of("Wearer")?;
    assert!(!mtg_engine::keywords::has_keyword(
        driver.state(),
        wearer,
        &Keyword::Shroud
    ));
    assert_replays(&driver);
    Ok(())
}

/// A creature card of this creature type.
fn kin(name: &str, kind: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(kind)
            .with_power_toughness(1, 1),
    )
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

#[rstest]
fn a_land_asks_whether_to_reveal_a_card_and_enters_untapped_only_when_it_does(
    #[values(("Flamekin Village", "Elemental"), ("Gilt-Leaf Palace", "Elf"))] land: (&str, &str),
    #[values(2, 3, 4)] seats: usize,
    // How many cards of the kind the hand holds, and what the player answers.
    #[values((0, false), (1, true), (1, false), (2, true), (2, false))] hand: (usize, bool),
) -> Result<(), ScriptError> {
    let (land, kind) = land;
    let (cards, reveals) = hand;
    let mut held = vec![card(land).unwrap(), kin("Bystander", "Human")];
    held.extend((0..cards).map(|n| kin(&format!("Kin {n}"), kind)));
    let scenario = || Scenario::new(seats).hand(0, held.clone());
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    driver.play_land(land)?;
    // The question is asked all the same when no card can be shown, with No the only answer: the pace tells nothing.
    let decision = driver.decision()?.clone();
    assert_eq!(decision.player, pid(0));
    let DecisionKind::YesNo(question) = &decision.kind else {
        panic!("a question was expected, got {:?}", decision.kind);
    };
    assert_eq!(question.only_no, cards == 0);
    assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), land), 1);
    if question.only_no {
        assert!(driver.answer(Answer::YesNo(YesNoChoice(true))).is_err());
    }
    driver.answer(Answer::YesNo(YesNoChoice(reveals)))?;
    let mut shown = None;
    if reveals && cards > 1 {
        // Only the cards of the kind are offered, never the Human.
        let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
            panic!("a choice of cards was expected");
        };
        let mut offered: Vec<String> = options
            .from
            .iter()
            .map(|id| crate::common::name_of(driver.state(), *id))
            .collect();
        offered.sort();
        assert_eq!(offered, ["Kin 0", "Kin 1"]);
        let last = *options.from.last().unwrap();
        shown = Some(last);
        driver.answer(Answer::ChooseCards(CardsChoice(vec![last])))?;
    }
    assert_eq!(in_zone(&driver, Zone::Battlefield, land), 1);
    assert_eq!(is_tapped(&driver, land), !reveals);
    let revealed: Vec<_> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::Revealed(revealed) => Some(revealed.clone()),
            _ => None,
        })
        .collect();
    if reveals {
        assert_eq!(revealed.len(), 1);
        assert_eq!(revealed[0].player, pid(0));
        assert_eq!(revealed[0].cards.len(), 1);
        if let Some(shown) = shown {
            assert_eq!(revealed[0].cards, [shown]);
        }
        // The card revealed stays in the hand.
        assert_eq!(in_zone(&driver, Zone::Hand(pid(0)), "Bystander"), 1);
        assert_eq!(
            in_zone(&driver, Zone::Hand(pid(0)), "Kin 0")
                + in_zone(&driver, Zone::Hand(pid(0)), "Kin 1"),
            cards
        );
    } else {
        assert!(revealed.is_empty());
    }
    assert_eq!(
        replay(&setup, driver.engine().events())
            .unwrap()
            .state_hash(),
        driver.state().state_hash()
    );
    Ok(())
}

/// An instant for `{G}` that gains `amount` life.
fn brew(amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Brew")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[Color::Green]))
            .with_rules(mtg_cards::ability::Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(amount)],
            }),
    )
}

#[rstest]
fn the_ghoul_returns_tapped_at_the_end_step_after_three_life_gained(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3, 5)] gained: i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [brew(gained)])
        .graveyard(0, [card("Silversmote Ghoul").unwrap()])
        .battlefield(0, [ready(forest())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Brew")?;
    settle(&mut driver, false, None)?;
    driver.pass_until(Step::End)?;
    settle(&mut driver, false, None)?;
    let back = gained >= 3;
    assert_eq!(
        in_zone(&driver, Zone::Battlefield, "Silversmote Ghoul"),
        usize::from(back)
    );
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0)), "Silversmote Ghoul"),
        usize::from(!back)
    );
    if back {
        let ghoul = on_battlefield(driver.state(), "Silversmote Ghoul").unwrap();
        assert!(
            driver
                .state()
                .object(ghoul)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        );
    }
    assert_replays(&driver);
    Ok(())
}
