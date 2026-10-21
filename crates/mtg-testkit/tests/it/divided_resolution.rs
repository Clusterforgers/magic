//! Goal: a divided spell or ability (and the corpus cards Forked Bolt and Armament Corps) gives each target exactly the share chosen as it was announced (damage and counters), a target that became illegal loses its share while the others keep theirs, a spell whose targets all became illegal fizzles, an amount divided evenly is rounded down among the targets still legal, one target takes the whole amount, the log of each replays to the live state, and whole bot games with divided spells deal what the divisions say under every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{DivideChoice, ModeChoice, Target};
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::PayChoice;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::{Answer, Event, replay};
use mtg_testkit::announce_cards::{
    even_bolt, split_bolt, split_charm, split_growth, split_hawk, split_pinger,
};
use mtg_testkit::cards::{burn, forest, free_creature, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready, split_fuzz_setup,
};
use rstest::rstest;

use crate::common::seeds::for_each_seed;

/// Seat 0 holds `card` with two Mountains and two Forests and controls Pal and Pal Two (1/1 each); seat 1 controls Bear
/// (2/2) and Ogre (4/4) and holds Zap, which deals 3 damage for `{R}`, with a Mountain to cast it.
fn scenario(card: Arc<CardDefinition>) -> Scenario {
    answered_by(card, vec![burn("Zap", 3)], vec![ready(mountain())])
}

/// The same board, with the opponent holding `hand` and the lands to cast it.
fn answered_by(
    card: Arc<CardDefinition>,
    hand: Vec<Arc<CardDefinition>>,
    mut lands: Vec<mtg_engine::PresetPermanent>,
) -> Scenario {
    lands.extend([
        ready(free_creature("Bear", 2, 2)),
        ready(free_creature("Ogre", 4, 4)),
    ]);
    Scenario::new(2)
        .hand(0, [card])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(forest()),
                ready(forest()),
                ready(free_creature("Pal", 1, 1)),
                ready(free_creature("Pal Two", 1, 1)),
            ],
        )
        .hand(1, hand)
        .battlefield(1, lands)
}

/// The target that goes by this name: `foe` and `me` are the seats, anything else a permanent.
fn target(driver: &Driver, name: &str) -> Target {
    match name {
        "me" => driver.target_player(0),
        "foe" => driver.target_player(1),
        permanent => driver.target_named(permanent).unwrap(),
    }
}

fn targets(driver: &Driver, names: &[&str]) -> Vec<Target> {
    names.iter().map(|name| target(driver, name)).collect()
}

/// Casts the spell at the chosen targets, dividing as `shares` say (a lone target is not asked), and pays.
fn cast_divided(
    driver: &mut Driver,
    spell: &str,
    names: &[&str],
    shares: &[u32],
) -> Result<ObjectId, ScriptError> {
    driver.begin_cast(spell)?;
    let chosen = targets(driver, names);
    driver.choose_targets(&chosen)?;
    if !shares.is_empty() {
        let division = chosen.iter().copied().zip(shares.iter().copied()).collect();
        driver.answer(Answer::DivideAmount(DivideChoice::Division(division)))?;
    }
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(cast_object(&events))
}

fn cast_object(events: &[Event]) -> ObjectId {
    events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { object, .. } => Some(*object),
            _ => None,
        })
        .expect("a spell was cast")
}

/// Passes until nothing is on the stack and returns what was logged meanwhile.
fn settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let before = driver.engine().events().len();
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(driver.engine().events()[before..].to_vec());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The opponent answers the spell on the stack by casting a Zap at each of the creatures.
fn zap_in_response(driver: &mut Driver, victims: &[&str]) -> Result<(), ScriptError> {
    driver.pass()?;
    for victim in victims {
        let victim = driver.target_named(victim)?;
        driver.cast_at("Zap", &[victim])?;
    }
    Ok(())
}

/// What the object dealt, by who took it: `foe` and `me` for the seats, the card name for a permanent.
fn dealt_by(driver: &Driver, events: &[Event], source: ObjectId) -> BTreeMap<String, u32> {
    let mut dealt = BTreeMap::new();
    for event in events {
        if let Event::DamageDealt {
            source: from,
            target,
            amount,
            ..
        } = event
            && *from == source
        {
            let name = match target {
                DamageTarget::Player(player) if *player == pid(0) => "me".to_owned(),
                DamageTarget::Player(_) => "foe".to_owned(),
                DamageTarget::Object(id) => name_of(driver.state(), *id),
            };
            *dealt.entry(name).or_insert(0) += amount;
        }
    }
    dealt
}

fn expected(list: &[(&str, u32)]) -> BTreeMap<String, u32> {
    list.iter()
        .map(|(name, amount)| ((*name).to_owned(), *amount))
        .collect()
}

/// How the resolution of the object ended.
fn ended(events: &[Event], object: ObjectId) -> Vec<ResolutionOutcome> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ResolutionEnded {
                object: ended,
                outcome,
            } if *ended == object => Some(*outcome),
            _ => None,
        })
        .collect()
}

/// The counters put on each permanent, by the card name.
fn counters_put(driver: &Driver, events: &[Event]) -> BTreeMap<String, i32> {
    let mut put = BTreeMap::new();
    for event in events {
        if let Event::CountersChanged(changed) = event
            && let Target::Object(object) = changed.on
        {
            *put.entry(name_of(driver.state(), object.id)).or_insert(0) += changed.delta;
        }
    }
    put
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::two_and_one(3, &["foe", "Bear"], &[2, 1])]
#[case::one_and_two(3, &["foe", "Bear"], &[1, 2])]
#[case::one_each(2, &["foe", "Bear"], &[1, 1])]
#[case::three_targets(5, &["foe", "Bear", "Ogre"], &[3, 1, 1])]
#[case::the_most_to_one_of_four(6, &["foe", "me", "Bear", "Ogre"], &[1, 1, 1, 3])]
#[case::one_target_takes_the_whole_amount(4, &["Ogre"], &[])]
fn each_target_gets_exactly_its_share(
    #[case] total: i32,
    #[case] names: &[&str],
    #[case] shares: &[u32],
) -> Result<(), ScriptError> {
    let card = || split_bolt("Split Bolt", total);
    let mut driver = Driver::from_scenario(scenario(card()))?;
    let spell = cast_divided(&mut driver, "Split Bolt", names, shares)?;
    let events = settle(&mut driver)?;
    // A lone target is not asked: it takes the whole amount.
    let whole = [u32::try_from(total).unwrap()];
    let given = if shares.is_empty() {
        &whole[..]
    } else {
        shares
    };
    let wanted: Vec<(&str, u32)> = names.iter().copied().zip(given.iter().copied()).collect();
    assert_eq!(dealt_by(&driver, &events, spell), expected(&wanted));
    assert_eq!(ended(&events, spell), [ResolutionOutcome::Resolved]);
    replays(&driver, scenario(card()));
    Ok(())
}

#[rstest]
#[case::the_other_target_keeps_its_share(3, &["foe", "Bear"], &[2, 1], &[("foe", 2)])]
#[case::the_share_of_the_lost_target_is_not_given_away(3, &["foe", "Bear"], &[1, 2], &[("foe", 1)])]
#[case::each_of_the_others_keeps_its_own(5, &["foe", "Bear", "Ogre"], &[1, 2, 2], &[("foe", 1), ("Ogre", 2)])]
fn a_target_that_became_illegal_loses_its_share_and_the_others_keep_theirs(
    #[case] total: i32,
    #[case] names: &[&str],
    #[case] shares: &[u32],
    #[case] dealt: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let card = || split_bolt("Split Bolt", total);
    let mut driver = Driver::from_scenario(scenario(card()))?;
    let spell = cast_divided(&mut driver, "Split Bolt", names, shares)?;
    zap_in_response(&mut driver, &["Bear"])?;
    let events = settle(&mut driver)?;
    assert_eq!(dealt_by(&driver, &events, spell), expected(dealt));
    assert_eq!(
        ended(&events, spell),
        [ResolutionOutcome::Resolved],
        "one target is left, so the spell does something"
    );
    replays(&driver, scenario(card()));
    Ok(())
}

#[rstest]
#[case::one_target(&["Bear"], &[])]
#[case::two_targets(&["Bear", "Pal"], &[1, 2])]
fn a_spell_whose_targets_all_became_illegal_deals_nothing_and_fizzles(
    #[case] names: &[&str],
    #[case] shares: &[u32],
) -> Result<(), ScriptError> {
    let scenario = || {
        answered_by(
            split_bolt("Split Bolt", 3),
            vec![burn("Zap", 3), burn("Zap", 3)],
            vec![ready(mountain()), ready(mountain())],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let spell = cast_divided(&mut driver, "Split Bolt", names, shares)?;
    zap_in_response(&mut driver, names)?;
    let events = settle(&mut driver)?;
    assert!(dealt_by(&driver, &events, spell).is_empty());
    assert_eq!(ended(&events, spell), [ResolutionOutcome::Fizzled]);
    let life = |seat| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!((life(0), life(1)), (20, 20));
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::one_target_takes_the_whole_amount(5, &["foe"], None, &[("foe", 5)])]
#[case::two_targets(5, &["foe", "Bear"], None, &[("foe", 2), ("Bear", 2)])]
#[case::rounded_down(5, &["foe", "Bear", "Ogre"], None, &[("foe", 1), ("Bear", 1), ("Ogre", 1)])]
#[case::exact(6, &["foe", "Bear", "Ogre"], None, &[("foe", 2), ("Bear", 2), ("Ogre", 2)])]
#[case::four_targets(6, &["foe", "me", "Bear", "Ogre"], None, &[("foe", 1), ("me", 1), ("Bear", 1), ("Ogre", 1)])]
#[case::rounded_down_to_nothing(3, &["foe", "me", "Bear", "Ogre"], None, &[])]
#[case::among_those_still_legal(6, &["foe", "Bear", "Ogre"], Some("Bear"), &[("foe", 3), ("Ogre", 3)])]
#[case::the_last_legal_target_takes_the_whole_amount(6, &["foe", "Bear"], Some("Bear"), &[("foe", 6)])]
#[case::one_fewer_changes_the_rounding(5, &["foe", "Bear", "Ogre"], Some("Bear"), &[("foe", 2), ("Ogre", 2)])]
fn an_even_division_is_rounded_down_among_the_targets_still_legal(
    #[case] total: i32,
    #[case] names: &[&str],
    #[case] lost: Option<&str>,
    #[case] dealt: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let card = || even_bolt("Even Bolt", total);
    let mut driver = Driver::from_scenario(scenario(card()))?;
    let spell = cast_divided(&mut driver, "Even Bolt", names, &[])?;
    if let Some(victim) = lost {
        zap_in_response(&mut driver, &[victim])?;
    }
    let events = settle(&mut driver)?;
    assert_eq!(dealt_by(&driver, &events, spell), expected(dealt));
    replays(&driver, scenario(card()));
    Ok(())
}

#[rstest]
#[case::two_and_one(&["Pal", "Pal Two"], &[2, 1], &[("Pal", 2), ("Pal Two", 1)])]
#[case::one_and_two(&["Pal", "Pal Two"], &[1, 2], &[("Pal", 1), ("Pal Two", 2)])]
#[case::one_target_takes_all(&["Pal Two"], &[], &[("Pal Two", 3)])]
#[case::a_creature_of_the_opponent(&["Pal", "Ogre"], &[2, 1], &[("Pal", 2), ("Ogre", 1)])]
fn counters_divided_as_chosen_go_to_each_creature_in_its_share(
    #[case] names: &[&str],
    #[case] shares: &[u32],
    #[case] put: &[(&str, i32)],
) -> Result<(), ScriptError> {
    let card = || split_growth("Split Growth", 3);
    let mut driver = Driver::from_scenario(scenario(card()))?;
    let spell = cast_divided(&mut driver, "Split Growth", names, shares)?;
    let events = settle(&mut driver)?;
    let put: BTreeMap<String, i32> = put
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count))
        .collect();
    assert_eq!(counters_put(&driver, &events), put);
    assert_eq!(ended(&events, spell), [ResolutionOutcome::Resolved]);
    replays(&driver, scenario(card()));
    Ok(())
}

#[test]
fn a_creature_that_died_before_the_counters_loses_its_share_and_the_other_keeps_its_own()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_growth("Split Growth", 3)))?;
    let spell = cast_divided(&mut driver, "Split Growth", &["Pal", "Pal Two"], &[2, 1])?;
    zap_in_response(&mut driver, &["Pal Two"])?;
    let events = settle(&mut driver)?;
    let put = BTreeMap::from([("Pal".to_owned(), 2)]);
    assert_eq!(counters_put(&driver, &events), put);
    assert_eq!(ended(&events, spell), [ResolutionOutcome::Resolved]);
    Ok(())
}

#[rstest]
#[case::one_each(&["foe", "Bear"], &[1, 1], &[("foe", 1), ("Bear", 1)])]
#[case::a_lone_target_takes_both(&["Bear"], &[], &[("Bear", 2)])]
fn a_mode_of_a_charm_gives_its_shares(
    #[case] names: &[&str],
    #[case] shares: &[u32],
    #[case] dealt: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_charm("Split Charm")))?;
    driver.begin_cast("Split Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))?;
    let chosen = targets(&driver, names);
    driver.choose_targets(&chosen)?;
    if !shares.is_empty() {
        let division = chosen.iter().copied().zip(shares.iter().copied()).collect();
        driver.answer(Answer::DivideAmount(DivideChoice::Division(division)))?;
    }
    let spell = cast_object(&driver.answer(Answer::PayMana(PayChoice::Auto))?);
    let events = settle(&mut driver)?;
    assert_eq!(dealt_by(&driver, &events, spell), expected(dealt));
    Ok(())
}

#[rstest]
#[case::divided(&["foe", "Bear"], &[1, 1], &[("foe", 1), ("Bear", 1)])]
#[case::a_lone_target_takes_both(&["Bear"], &[], &[("Bear", 2)])]
fn an_activated_ability_gives_its_shares(
    #[case] names: &[&str],
    #[case] shares: &[u32],
    #[case] dealt: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .battlefield(0, [ready(split_pinger("Split Pinger"))])
            .battlefield(1, [ready(free_creature("Bear", 2, 2))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let pinger = driver.id_of("Split Pinger")?;
    driver.begin_activation("Split Pinger")?;
    let chosen = targets(&driver, names);
    driver.choose_targets(&chosen)?;
    if !shares.is_empty() {
        let division = chosen.iter().copied().zip(shares.iter().copied()).collect();
        driver.answer(Answer::DivideAmount(DivideChoice::Division(division)))?;
    }
    let events = settle(&mut driver)?;
    assert_eq!(dealt_by(&driver, &events, pinger), expected(dealt));
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn a_triggered_ability_gives_the_shares_it_divided_as_it_went_on_the_stack()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(split_hawk("Split Hawk")))?;
    driver.cast("Split Hawk")?;
    driver.pass()?;
    driver.pass()?;
    let hawk = driver.id_of("Split Hawk")?;
    let chosen = targets(&driver, &["foe", "Bear"]);
    driver.choose_targets(&chosen)?;
    let division = chosen.iter().copied().zip([1, 1]).collect();
    driver.answer(Answer::DivideAmount(DivideChoice::Division(division)))?;
    let events = settle(&mut driver)?;
    assert_eq!(
        dealt_by(&driver, &events, hawk),
        expected(&[("foe", 1), ("Bear", 1)])
    );
    Ok(())
}

/// What the bot games saw.
#[derive(Default)]
struct Seen {
    shares: u64,
    even: u64,
    counters: u64,
}

#[rstest]
#[case(2)]
#[case(4)]
fn bot_games_with_divided_spells_deal_what_their_divisions_say(#[case] players: usize) {
    let seen = std::sync::Mutex::new(Seen::default());
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    for_each_seed(0..games, |seed| {
        let outcome = play_game_with(
            split_fuzz_setup(players, seed),
            seed,
            PlayOptions::default(),
        )
        .unwrap_or_else(|failure| panic!("{failure}"));
        let mut seen = seen.lock().unwrap();
        for (at, event) in outcome.events.iter().enumerate() {
            let Event::SpellCast {
                object, choices, ..
            } = event
            else {
                continue;
            };
            // The spell deals its damage as it resolves; a creature spell has no resolution of its own to end, and its
            // source deals combat damage much later.
            let resolved = outcome.events[at..]
                .iter()
                .position(|later| match later {
                    Event::ResolutionEnded { object: ended, .. }
                    | Event::SpellResolved { object: ended } => ended == object,
                    _ => false,
                })
                .map_or(outcome.events.len(), |end| at + end);
            let dealt: Vec<(DamageTarget, u32)> = outcome.events[at..resolved]
                .iter()
                .filter_map(|later| match later {
                    Event::DamageDealt {
                        source,
                        target,
                        amount,
                        ..
                    } if source == object => Some((*target, *amount)),
                    _ => None,
                })
                .collect();
            if choices.division.is_empty() {
                // Divided evenly, whatever the number of targets: every target takes the same, no more than its part.
                if let Some((_, each)) = dealt.first() {
                    assert!(dealt.iter().all(|(_, amount)| amount == each), "{dealt:?}");
                    assert!(
                        *each * u32::try_from(dealt.len()).unwrap() <= 5,
                        "{dealt:?}"
                    );
                    seen.even += 1;
                }
                continue;
            }
            for (target, amount) in dealt {
                let share = choices
                    .division
                    .iter()
                    .find(|(chosen, _)| match (chosen, target) {
                        (Target::Player(a), DamageTarget::Player(b)) => *a == b,
                        (Target::Object(a), DamageTarget::Object(b)) => a.id == b,
                        _ => false,
                    })
                    .map(|(_, share)| *share);
                assert_eq!(share, Some(amount), "seed {seed}: {choices:?}");
                seen.shares += 1;
            }
        }
        seen.counters += outcome
            .events
            .iter()
            .filter(|event| matches!(event, Event::CountersChanged(_)))
            .count() as u64;
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.shares > 0, "some divided spell dealt its shares");
    assert!(seen.even > 0, "some spell divided its damage evenly");
    assert!(seen.counters > 0, "some spell put its counters");
}

#[rstest]
#[case::one_each(&["foe", "Bear"], &[1, 1], None, &[("foe", 1), ("Bear", 1)])]
#[case::both_at_one_target(&["Bear"], &[], None, &[("Bear", 2)])]
#[case::both_at_the_player(&["foe"], &[], None, &[("foe", 2)])]
#[case::the_share_of_a_target_that_died_is_lost(&["foe", "Bear"], &[1, 1], Some("Bear"), &[("foe", 1)])]
fn forked_bolt_deals_its_two_damage_as_divided(
    #[case] names: &[&str],
    #[case] shares: &[u32],
    #[case] lost: Option<&str>,
    #[case] dealt: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let scenario = || {
        answered_by(
            card("Forked Bolt").unwrap(),
            vec![burn("Zap", 3)],
            vec![ready(mountain())],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let spell = cast_divided(&mut driver, "Forked Bolt", names, shares)?;
    if let Some(victim) = lost {
        zap_in_response(&mut driver, &[victim])?;
    }
    let events = settle(&mut driver)?;
    assert_eq!(dealt_by(&driver, &events, spell), expected(dealt));
    replays(&driver, scenario());
    Ok(())
}

/// Seat 0 casts Armament Corps with Pal (1/1) beside it; seat 1 holds Zap and a Mountain.
fn armament_scenario() -> Scenario {
    Scenario::new(2)
        .hand(0, [card("Armament Corps").unwrap()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(swamp()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(free_creature("Pal", 1, 1)),
            ],
        )
        .hand(1, [burn("Zap", 3)])
        .battlefield(1, [ready(mountain())])
}

#[rstest]
#[case::one_each(&["Armament Corps", "Pal"], &[1, 1], None, &[("Armament Corps", 1), ("Pal", 1)])]
#[case::both_on_the_other(&["Pal"], &[], None, &[("Pal", 2)])]
#[case::both_on_itself(&["Armament Corps"], &[], None, &[("Armament Corps", 2)])]
#[case::the_share_of_a_creature_that_died_is_lost(&["Armament Corps", "Pal"], &[1, 1], Some("Pal"), &[("Armament Corps", 1)])]
fn armament_corps_distributes_its_two_counters_as_divided(
    #[case] names: &[&str],
    #[case] shares: &[u32],
    #[case] lost: Option<&str>,
    #[case] put: &[(&str, i32)],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(armament_scenario())?;
    driver.cast("Armament Corps")?;
    // Both pass and the creature resolves; its trigger then asks for its targets.
    driver.pass()?;
    driver.pass()?;
    let chosen = targets(&driver, names);
    driver.choose_targets(&chosen)?;
    if !shares.is_empty() {
        let division = chosen.iter().copied().zip(shares.iter().copied()).collect();
        driver.answer(Answer::DivideAmount(DivideChoice::Division(division)))?;
    }
    if let Some(victim) = lost {
        zap_in_response(&mut driver, &[victim])?;
    }
    let events = settle(&mut driver)?;
    let put: BTreeMap<String, i32> = put
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count))
        .collect();
    assert_eq!(counters_put(&driver, &events), put);
    replays(&driver, armament_scenario());
    Ok(())
}
