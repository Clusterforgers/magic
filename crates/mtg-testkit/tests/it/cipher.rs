//! Goal: cipher plays whole at tables of two, three and four seats: the spell is exiled encoded on a creature its controller picks (or goes to the graveyard when there is none or they decline), whenever that creature deals combat damage to a player its controller may cast a copy of the card for nothing, the copy is never encoded and ceases to exist, the link ends when the card leaves exile or the creature leaves the battlefield, several encoded cards and several carriers trigger separately, a new controller of the creature casts the copy, and every game replays to the same state; bot games with the cipher cards finish and really cast copies.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::seeds::fuzz_games;
use crate::common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, cost, free_creature, gain_life, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, cipher_fuzz_setup, play_game, ready};
use rstest::rstest;

/// A sorcery for nothing that gains its controller `amount` life, with cipher.
fn salve(name: &str, amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(amount)],
            })
            .with_rules(Ability::Keyword(Keyword::Cipher)),
    )
}

/// What the players answer to the questions of a cipher spell and of the copies it casts.
#[derive(Clone)]
struct Policy {
    /// The creatures the caster encodes on, by name; none declines.
    carriers: Vec<&'static str>,
    /// The controller of the trigger casts the copy.
    cast: bool,
}

/// Seat 0 holds the cipher spells and controls `carriers`, ready to attack; no other seat has a creature.
fn table(seats: usize, spells: &[&str], carriers: &[&str]) -> Result<Driver, ScriptError> {
    table_with_a_foe(seats, spells, carriers, Vec::new())
}

/// [`table`], and seat 1 holds these cards and a Mountain.
fn table_with_a_foe(
    seats: usize,
    spells: &[&str],
    carriers: &[&str],
    foe_hand: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut mine = lands(2, island);
    mine.extend(carriers.iter().map(|name| ready(free_creature(name, 2, 2))));
    let hand: Vec<_> = spells.iter().map(|name| salve(name, 2)).collect();
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, hand)
            .hand(1, foe_hand)
            .battlefield(0, mine)
            .battlefield(1, lands(1, mountain)),
        |scenario, seat| scenario.library(seat, (0..12).map(|_| island())),
    );
    Driver::from_scenario(scenario)
}

/// Answers the questions of the stack until it is empty and its owner has priority again.
fn settle(driver: &mut Driver, policy: &Policy) -> Result<(), ScriptError> {
    answer_until(driver, policy, |driver| driver.state().stack().is_empty())
}

/// Answers the questions by `policy` until `done` holds at a decision of priority; a pass moves the game on before.
fn answer_until(
    driver: &mut Driver,
    policy: &Policy,
    done: impl Fn(&Driver) -> bool,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if done(driver) => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
                continue;
            }
            DecisionKind::ChooseCards(options) => {
                let state = driver.state();
                let wanted: Vec<ObjectId> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| policy.carriers.contains(&name_of(state, *id).as_str()))
                    .collect();
                Answer::ChooseCards(CardsChoice(wanted))
            }
            DecisionKind::YesNo(options) => {
                assert!(options.text.starts_with("Cast"), "{}", options.text);
                Answer::YesNo(YesNoChoice(policy.cast))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![options.legal[0]]))
            }
            // Blocks, the order of triggers and the like are answered the passive way.
            _ => {
                driver.answer_passively()?;
                continue;
            }
        };
        driver.answer(answer)?;
    }
    panic!("the game did not reach the end of the questions");
}

/// Seat 0 attacks seat 1 with these creatures in turn 1, and the game goes on through the combat damage and every
/// trigger it causes, answered by `policy`.
fn attack_and_settle(
    driver: &mut Driver,
    attackers: &[&str],
    policy: &Policy,
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    answer_until(driver, policy, |driver| {
        driver.state().turn().step() == Step::EndCombat && driver.state().stack().is_empty()
    })
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The cards of this name in a zone.
fn cards_in(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == name)
        .collect()
}

/// Casts the spell and answers by `policy`; the card is where the rules put it.
fn cast_and_settle(driver: &mut Driver, name: &str, policy: &Policy) -> Result<(), ScriptError> {
    driver.cast(name)?;
    settle(driver, policy)
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn the_spell_is_exiled_encoded_on_the_creature_its_controller_picks(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["Veil"], &["Carrier A", "Carrier B"])?;
    let policy = Policy {
        carriers: vec!["Carrier B"],
        cast: true,
    };
    cast_and_settle(&mut driver, "Veil", &policy)?;
    let card = cards_in(&driver, Zone::Exile, "Veil")[0];
    let carrier = cards_in(&driver, Zone::Battlefield, "Carrier B")[0];
    assert_eq!(driver.state().encoded_creature(card), Some(carrier));
    assert!(cards_in(&driver, Zone::Graveyard(pid(0)), "Veil").is_empty());
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
    let encoded = driver.engine().events().iter().any(|event| {
        matches!(
            event,
            Event::ZoneChanged { to: Zone::Exile, cause: ZoneCause::Encoded { by }, .. }
                if by.id == carrier
        )
    });
    assert!(encoded);
    Ok(())
}

#[rstest]
#[case::no_creature(&[], &[])]
#[case::declined(&["Carrier A"], &[])]
#[case::declined_with_two(&["Carrier A", "Carrier B"], &[])]
fn a_spell_nobody_encodes_goes_to_the_graveyard(
    #[case] on_table: &[&str],
    #[case] picked: &[&'static str],
) -> Result<(), ScriptError> {
    for seats in 2..=4 {
        let mut driver = table(seats, &["Veil"], on_table)?;
        let policy = Policy {
            carriers: picked.to_vec(),
            cast: true,
        };
        cast_and_settle(&mut driver, "Veil", &policy)?;
        assert_eq!(cards_in(&driver, Zone::Graveyard(pid(0)), "Veil").len(), 1);
        assert!(cards_in(&driver, Zone::Exile, "Veil").is_empty());
    }
    Ok(())
}

/// How many events of a kind the log holds.
fn count(driver: &Driver, kind: fn(&Event) -> bool) -> usize {
    driver.engine().events().iter().filter(|e| kind(e)).count()
}

fn replays_to_the_same_state(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn the_creature_dealing_combat_damage_casts_a_copy_of_the_card_for_nothing(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["Veil"], &["Carrier A"])?;
    let policy = Policy {
        carriers: vec!["Carrier A"],
        cast: true,
    };
    cast_and_settle(&mut driver, "Veil", &policy)?;
    attack_and_settle(&mut driver, &["Carrier A"], &policy)?;
    let tail: Vec<String> = driver
        .engine()
        .events()
        .iter()
        .rev()
        .take(45)
        .map(|e| format!("{e:?}").chars().take(110).collect())
        .collect();
    assert_eq!(
        life(&driver, 0),
        24,
        "the spell and its copy gain 2 each: {tail:#?}"
    );
    assert_eq!(life(&driver, 1), 18, "the damage of the creature");
    let carrier = cards_in(&driver, Zone::Battlefield, "Carrier A")[0];
    let card = cards_in(&driver, Zone::Exile, "Veil");
    assert_eq!(card.len(), 1, "the copy is gone and the card stays");
    assert_eq!(driver.state().encoded_creature(card[0]), Some(carrier));
    assert!(cards_in(&driver, Zone::Graveyard(pid(0)), "Veil").is_empty());
    assert_eq!(count(&driver, |e| matches!(e, Event::CardCopied(_))), 1);
    assert_eq!(count(&driver, |e| matches!(e, Event::SpellCast { .. })), 2);
    assert_eq!(count(&driver, |e| matches!(e, Event::CopyCeased { .. })), 0);
    let encodings = count(&driver, |e| {
        matches!(
            e,
            Event::ZoneChanged {
                cause: ZoneCause::Encoded { .. },
                ..
            }
        )
    });
    assert_eq!(encodings, 1, "the copy is not encoded");
    replays_to_the_same_state(&driver);
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn a_card_whose_creature_died_is_exiled_for_good_and_a_new_attacker_casts_nothing(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let bolt = burn("Bolt", 3);
    let mut driver = table_with_a_foe(seats, &["Veil"], &["Carrier A", "Carrier B"], vec![bolt])?;
    let policy = Policy {
        carriers: vec!["Carrier A"],
        cast: true,
    };
    cast_and_settle(&mut driver, "Veil", &policy)?;
    // The foe kills the creature with the card on it before combat.
    driver.pass()?;
    driver.begin_cast("Bolt")?;
    let target = driver.target_named("Carrier A")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &policy)?;
    assert!(cards_in(&driver, Zone::Battlefield, "Carrier A").is_empty());
    let card = cards_in(&driver, Zone::Exile, "Veil");
    assert_eq!(card.len(), 1, "the card stays in exile");
    assert_eq!(driver.state().encoded_creature(card[0]), None);
    attack_and_settle(&mut driver, &["Carrier B"], &policy)?;
    assert_eq!(life(&driver, 0), 22, "no copy was cast");
    assert_eq!(count(&driver, |e| matches!(e, Event::CardCopied(_))), 0);
    replays_to_the_same_state(&driver);
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn the_new_controller_of_the_creature_casts_the_copy(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["Veil"], &["Carrier A"])?;
    let policy = Policy {
        carriers: vec!["Carrier A"],
        cast: true,
    };
    cast_and_settle(&mut driver, "Veil", &policy)?;
    let carrier = cards_in(&driver, Zone::Battlefield, "Carrier A")[0];
    driver.add_effect(
        driver.on(carrier),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    // Seat 1 attacks seat 0 with it on their own turn and is the one who casts the copy.
    driver.idle_until(2, Step::Main1)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Carrier A", 0)])?;
    answer_until(&mut driver, &policy, |driver| {
        driver.state().turn().step() == Step::EndCombat && driver.state().stack().is_empty()
    })?;
    assert_eq!(life(&driver, 1), 22, "the copy gained life for its caster");
    assert_eq!(
        life(&driver, 0),
        20,
        "their own spell, and the damage of the creature"
    );
    let casters: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { player, .. } => Some(*player),
            _ => None,
        })
        .collect();
    assert_eq!(casters, [pid(0), pid(1)]);
    replays_to_the_same_state(&driver);
    Ok(())
}

#[rstest]
#[case::two_carriers_one_card_each(&["Carrier A", "Carrier B"], &["Carrier A", "Carrier B"], 2)]
#[case::one_carrier_two_cards(&["Carrier A", "Carrier A"], &["Carrier A"], 2)]
#[case::one_of_two_carriers_attacks(&["Carrier A", "Carrier B"], &["Carrier B"], 1)]
fn every_encoded_card_of_an_attacker_triggers_on_its_own(
    #[case] encoded_on: &[&'static str],
    #[case] attackers: &[&str],
    #[case] copies: i32,
) -> Result<(), ScriptError> {
    for seats in 2..=4 {
        let mut driver = table(seats, &["Veil A", "Veil B"], &["Carrier A", "Carrier B"])?;
        for (name, on) in ["Veil A", "Veil B"].into_iter().zip(encoded_on) {
            let policy = Policy {
                carriers: vec![*on],
                cast: true,
            };
            cast_and_settle(&mut driver, name, &policy)?;
        }
        assert_eq!(life(&driver, 0), 24, "two spells gain 2 each");
        let policy = Policy {
            carriers: Vec::new(),
            cast: true,
        };
        attack_and_settle(&mut driver, attackers, &policy)?;
        let expected = 24 + 2 * copies;
        assert_eq!(
            life(&driver, 0),
            expected,
            "{copies} copies at {seats} seats"
        );
        assert_eq!(cards_in(&driver, Zone::Exile, "Veil A").len(), 1);
        assert_eq!(cards_in(&driver, Zone::Exile, "Veil B").len(), 1);
        replays_to_the_same_state(&driver);
    }
    Ok(())
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_cipher_finish_and_cast_copies(#[case] seats: usize) {
    let (mut encoded, mut copies, mut ceased) = (0, 0, 0);
    for seed in 0..fuzz_games(60) {
        let setup = cipher_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        for event in &outcome.events {
            match event {
                Event::ZoneChanged {
                    cause: ZoneCause::Encoded { .. },
                    ..
                } => encoded += 1,
                Event::CardCopied(_) => copies += 1,
                Event::CopyCeased { .. } => ceased += 1,
                _ => {}
            }
        }
    }
    assert!(encoded > 0, "a card was encoded");
    assert!(copies > 0, "a copy was made");
    assert!(ceased > 0, "a copy that was not cast ceased to exist");
}

#[rstest]
#[case::two(2, 41)]
#[case::four(4, 42)]
fn a_game_with_cipher_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = cipher_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = cipher_fuzz_setup(seats, seed).unwrap();
    assert_eq!(
        replay(&setup, &first.events).unwrap().state_hash(),
        first.final_hash
    );
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn a_copy_that_is_not_cast_ceases_to_exist_and_the_card_stays_encoded(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &["Veil"], &["Carrier A"])?;
    let encode = Policy {
        carriers: vec!["Carrier A"],
        cast: false,
    };
    cast_and_settle(&mut driver, "Veil", &encode)?;
    attack_and_settle(&mut driver, &["Carrier A"], &encode)?;
    assert_eq!(life(&driver, 0), 22, "no copy was cast");
    let card = cards_in(&driver, Zone::Exile, "Veil");
    assert_eq!(card.len(), 1);
    assert!(driver.state().encoded_creature(card[0]).is_some());
    assert_eq!(count(&driver, |e| matches!(e, Event::CardCopied(_))), 1);
    assert_eq!(count(&driver, |e| matches!(e, Event::CopyCeased { .. })), 1);
    assert_eq!(count(&driver, |e| matches!(e, Event::SpellCast { .. })), 1);
    assert!(driver.state().stack().is_empty());
    replays_to_the_same_state(&driver);
    Ok(())
}
