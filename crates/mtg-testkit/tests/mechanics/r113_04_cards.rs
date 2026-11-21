//! Goal: the r113-04 pieces play at two to four seats: dice rolled as a permanent enters are stored on the permanent it
//! becomes and set its base power and toughness (Vedalken Squirrel-Whacker), a later six-sided roll may exchange a
//! result with one of them, declining keeps both; an Aura that enchants an instant card in a graveyard copies it when its
//! controller casts a sorcery, casts the copy for free and moves to another instant card (Spellweaver Volute); revealed
//! cards that share a mana value with another each make a Treasure; only the enchanted creature's controller activates
//! Merseine; The Pandorica holds a permanent phased out while tapped and phases it in when it untaps or leaves; Bell
//! Borca's power is the greatest mana value it noted this turn; and each game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::choose::{ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, creature, forest, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError};
use mtg_testkit::{attached_to, ready, tapped};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, pid};

/// A spell of the type for nothing whose only step is the JSON effect.
fn spell(name: &str, kind: CardType, effect: &str) -> Arc<CardDefinition> {
    let rules: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":[{effect}]}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// Passes priority until the stack is empty, answering an exchange of a roll with `pick` of its labels, yes to every
/// question of seat 0, the payment automatically and the rest passively; returns the exchanges' labels.
fn r113_settle(
    driver: &mut Driver,
    pick: &dyn Fn(&[String]) -> usize,
) -> Result<Vec<Vec<String>>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..300 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(asked);
        };
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(options) if options.text.starts_with("You may exchange") => {
                let labels: Vec<String> = options
                    .options
                    .iter()
                    .map(|option| match option {
                        ChoiceOption::Label(label) => label.clone(),
                        other => panic!("{other:?}"),
                    })
                    .collect();
                let at = pick(&labels);
                asked.push(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::YesNo(_) if decision.player == pid(0) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The results rolled since `from`.
fn r113_rolls(driver: &Driver, from: usize) -> Vec<u32> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect()
}

fn r113_id(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|object| {
            object.card().name == name && object.kind() == mtg_engine::object::ObjectKind::Card
        })
        .map(mtg_engine::object::GameObject::id)
        .unwrap_or_else(|| panic!("{name} is in the game"))
}

fn r113_stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = mtg_engine::characteristics::characteristics(driver.state(), r113_id(driver, name))
        .unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

const WHACKER: &str = "Vedalken Squirrel-Whacker";

#[rstest]
#[case(WHACKER)]
#[case("Spellweaver Volute")]
#[case("Uvilda, Dean of Perfection // Nassari, Dean of Expression")]
#[case("Vault 21: House Gambit")]
#[case("Cosima, God of the Voyage // The Omenkeel")]
#[case("Merseine")]
#[case("Blade of the Swarm")]
#[case("The Pandorica")]
#[case("Bell Borca, Spectral Sergeant")]
#[case("Volo, Itinerant Scholar")]
#[case("Casey & Raph, Hotheads")]
fn r113_the_engine_runs_every_card_of_the_pack(#[case] name: &str) {
    let record = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        record.support,
        mtg_cards::support::Support::Verified,
        "{name}: {record:?}"
    );
}

fn r113_whacker_table(seats: usize, seed: u64) -> Scenario {
    Scenario::new(seats)
        .seed(seed)
        .battlefield(0, lands(4, island))
        .hand(
            0,
            vec![
                card(WHACKER).unwrap(),
                spell(
                    "Roll",
                    CardType::Instant,
                    r#"{"RollDice":{"sides":6,"results":[]}}"#,
                ),
                spell(
                    "Roll Eight",
                    CardType::Instant,
                    r#"{"RollDice":{"sides":8,"results":[]}}"#,
                ),
            ],
        )
}

#[rstest]
fn r113_the_whacker_enters_with_its_rolls_and_exchanges_a_later_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 7)] seed: u64,
    #[values(0, 1, 2)] pick: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r113_whacker_table(seats, seed))?;
    let from = driver.engine().events().len();
    driver.cast(WHACKER)?;
    let asked = r113_settle(&mut driver, &|_| 0)?;
    assert!(asked.is_empty(), "its own entering roll is not exchanged");
    let [power, toughness] = r113_rolls(&driver, from)[..] else {
        panic!("two dice rolled as it enters");
    };
    let (power, toughness) = (
        i32::try_from(power).unwrap(),
        i32::try_from(toughness).unwrap(),
    );
    assert_eq!(r113_stats(&driver, WHACKER), (power, toughness));
    // An eight-sided roll is not exchanged.
    driver.cast("Roll Eight")?;
    assert!(r113_settle(&mut driver, &|_| 1)?.is_empty());
    let from = driver.engine().events().len();
    driver.cast("Roll")?;
    let asked = r113_settle(&mut driver, &|_| pick)?;
    let [labels] = &asked[..] else {
        panic!("one exchange offered");
    };
    assert_eq!(labels.len(), 3, "nothing, power or toughness");
    assert_eq!(labels[0], "Exchange nothing");
    let [rolled] = r113_rolls(&driver, from)[..] else {
        panic!("one die");
    };
    let rolled = i32::try_from(rolled).unwrap();
    let stats = r113_stats(&driver, WHACKER);
    match pick {
        0 => assert_eq!(stats, (power, toughness)),
        1 => {
            assert_eq!(stats, (driver_natural(&driver, from), toughness));
            assert_eq!(rolled, power, "the die takes the base power");
        }
        _ => {
            assert_eq!(stats, (power, driver_natural(&driver, from)));
            assert_eq!(rolled, toughness, "the die takes the base toughness");
        }
    }
    let setup = r113_whacker_table(seats, seed).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// The number the last die since `from` showed before an exchange.
fn driver_natural(driver: &Driver, from: usize) -> i32 {
    driver.engine().events()[from..]
        .iter()
        .find_map(|event| match event {
            Event::R6402DieChanged { from, .. } => Some(i32::try_from(*from).unwrap()),
            _ => None,
        })
        .expect("an exchange")
}

const VOLUTE: &str = "Spellweaver Volute";

fn r113_volute_table(seats: usize) -> Scenario {
    let gain = |name: &str, life: u32| {
        spell(
            name,
            CardType::Instant,
            &format!(r#"{{"GainLife":{{"amount":{life}}}}}"#),
        )
    };
    Scenario::new(seats)
        .battlefield(0, lands(5, island))
        .hand(
            0,
            vec![
                card(VOLUTE).unwrap(),
                spell("Ponder", CardType::Sorcery, r#"{"Draw":{}}"#),
            ],
        )
        .graveyard(0, vec![gain("Salve Three", 3)])
        .graveyard(1, vec![gain("Salve Two", 2)])
        .library(0, (0..4).map(|_| island()).collect::<Vec<_>>())
        .library(1, (0..4).map(|_| island()).collect::<Vec<_>>())
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

#[rstest]
fn r113_the_volute_casts_a_copy_and_moves_on(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r113_volute_table(seats))?;
    let three = mtg_engine::choose::Target::Object(
        driver
            .state()
            .object(r113_id(&driver, "Salve Three"))
            .unwrap()
            .reference(),
    );
    driver.cast_at(VOLUTE, &[three])?;
    r113_settle(&mut driver, &|_| 0)?;
    let volute = r113_id(&driver, VOLUTE);
    let salve = r113_id(&driver, "Salve Three");
    let host = |driver: &Driver| {
        driver
            .state()
            .object(volute)
            .unwrap()
            .permanent()
            .unwrap()
            .attached_to()
            .map(|on| on.id)
    };
    assert_eq!(
        driver.state().object(volute).unwrap().zone(),
        Zone::Battlefield
    );
    assert_eq!(
        host(&driver),
        Some(salve),
        "it enchants the card in the graveyard"
    );
    driver.cast("Ponder")?;
    r113_settle(&mut driver, &|_| 0)?;
    assert_eq!(life(&driver), 23, "the copy was cast and resolved");
    assert_eq!(driver.state().object(salve).unwrap().zone(), Zone::Exile);
    assert_eq!(
        host(&driver),
        Some(r113_id(&driver, "Salve Two")),
        "it moved to the other instant card"
    );
    let setup = r113_volute_table(seats).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// Passes priority until the stack is empty, answering a choice of cards of seat 0 with the cards named `pick`,
/// payments automatically and the rest passively.
fn r113_settle_picking(driver: &mut Driver, pick: &[&str]) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(options) if decision.player == pid(0) => {
                let chosen = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| pick.contains(&name_of(driver.state(), *id).as_str()))
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn r113_count(driver: &Driver, zone: Zone, name: &str) -> usize {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == name)
        .count()
}

const GAMBIT: &str = r#"{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Not":{"Type":"Land"}}]},"count":{"min":0,"max":5},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}},{"CreateToken":{"token":{"Named":"treasure"},"count":{"R113SharesWithAnother":{"of":{"Bound":"revealed"},"stat":"ManaValue"}}}}"#;

#[rstest]
#[case::two_pairs(&["Two A", "Two B", "Three A", "Three B", "Five"], 4)]
#[case::all_apart(&["Two A", "Three A", "Five"], 0)]
#[case::one_pair(&["Two A", "Two B", "Five"], 2)]
#[case::three_alike(&["Three A", "Three B", "Three C"], 3)]
#[case::none(&[], 0)]
fn r113_each_revealed_card_sharing_a_mana_value_makes_a_treasure(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: &[&str],
    #[case] treasures: usize,
) -> Result<(), ScriptError> {
    let named = |name: &str, value: u32| creature(name, value, &[], 1, 1);
    let table = || {
        Scenario::new(seats).hand(
            0,
            vec![
                spell("Gambit", CardType::Sorcery, GAMBIT),
                named("Two A", 2),
                named("Two B", 2),
                named("Three A", 3),
                named("Three B", 3),
                named("Three C", 3),
                named("Five", 5),
            ],
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Gambit")?;
    r113_settle_picking(&mut driver, pick)?;
    assert_eq!(
        r113_count(&driver, Zone::Battlefield, "Treasure"),
        treasures
    );
    let replayed = replay(&table().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r113_only_the_enchanted_creatures_controller_activates_the_aura(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let net = CounterKind::Other("net".to_owned());
    let aura = with_counters(
        attached_to(ready(card("Merseine").unwrap()), 1, 0),
        net.clone(),
        3,
    );
    let mut permanents = vec![ready(creature("Bear", 1, &[Color::Green], 2, 2))];
    permanents.extend(lands(2, forest));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [aura])
        .battlefield(1, permanents);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.decision()?.player, pid(0));
    assert!(
        driver.offered_abilities("Merseine")?.is_empty(),
        "the Aura's controller may not"
    );
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert!(!driver.offered_abilities("Merseine")?.is_empty());
    driver.begin_activation("Merseine")?;
    r113_settle(&mut driver, &|_| 0)?;
    let counters = driver
        .state()
        .object(r113_id(&driver, "Merseine"))
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&net);
    assert_eq!(
        counters, 2,
        "a net counter removed for the Bear's mana cost"
    );
    Ok(())
}

#[rstest]
#[case::untapped(r#"{"Untap":{"what":{"Each":[{"Named":"The Pandorica"}]}}}"#, true)]
#[case::destroyed(r#"{"Destroy":{"what":{"Each":[{"Named":"The Pandorica"}]}}}"#, true)]
#[case::still_tapped(r#"{"Draw":{}}"#, false)]
fn r113_the_pandorica_holds_a_permanent_out_while_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] then: &str,
    #[case] back: bool,
) -> Result<(), ScriptError> {
    let bear = creature("Bear", 1, &[Color::Green], 2, 2);
    let mut mine = vec![ready(card("The Pandorica").unwrap())];
    mine.extend(lands(2, plains));
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine.clone())
            .battlefield(1, [tapped(bear.clone())])
            .hand(0, vec![spell("Then", CardType::Instant, then)])
            .library(0, (0..4).map(|_| island()).collect::<Vec<_>>())
            .library(1, (0..4).map(|_| island()).collect::<Vec<_>>())
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_activation("The Pandorica")?;
    let bear_at = driver.target_named("Bear")?;
    driver.choose_targets(&[bear_at])?;
    r113_settle(&mut driver, &|_| 0)?;
    let bear = r113_id(&driver, "Bear");
    assert_eq!(driver.state().object(bear).unwrap().zone(), Zone::PhasedOut);
    driver.cast("Then")?;
    r113_settle(&mut driver, &|_| 0)?;
    let zone = driver.state().object(bear).unwrap().zone();
    assert_eq!(zone == Zone::Battlefield, back, "{zone:?}");
    if !back {
        // The Bear's controller's untap step does not phase it in while The Pandorica stays tapped.
        driver.idle_until(2, Step::Main1)?;
        assert_eq!(driver.state().object(bear).unwrap().zone(), Zone::PhasedOut);
    }
    let replayed = replay(&table().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::two_new_types(&[("Elf Warrior", &["Elf", "Warrior"] as &[&str]), ("Elf Druid", &["Elf", "Druid"])], 2)]
#[case::one_type_twice(&[("Elf One", &["Elf"] as &[&str]), ("Elf Two", &["Elf"])], 1)]
#[case::no_type(&[("Plain One", &[] as &[&str])], 0)]
fn r113_volos_journal_notes_one_new_creature_type_of_each_creature_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] spells: &[(&str, &[&str])],
    #[case] noted: usize,
) -> Result<(), ScriptError> {
    let creature_of = |name: &str, types: &[&str]| {
        let mut card = CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(1, 1);
        for subtype in types {
            card = card.with_subtype(*subtype);
        }
        Arc::new(card)
    };
    let count =
        r#"{"Draw":{"count":{"R113NotedWords":{"on":{"Each":[{"Named":"Volo's Journal"}]}}}}}"#;
    let mut hand = vec![
        card("Volo, Itinerant Scholar").unwrap(),
        spell("Count", CardType::Instant, count),
    ];
    hand.extend(spells.iter().map(|(name, types)| creature_of(name, types)));
    let table = || {
        Scenario::new(seats)
            .battlefield(0, lands(3, island))
            .hand(0, hand.clone())
            .library(0, (0..8).map(|_| island()).collect::<Vec<_>>())
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Volo, Itinerant Scholar")?;
    r113_settle(&mut driver, &|_| 0)?;
    assert_eq!(r113_count(&driver, Zone::Battlefield, "Volo's Journal"), 1);
    for (name, _) in spells {
        driver.cast(name)?;
        r113_settle(&mut driver, &|_| 0)?;
    }
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.cast("Count")?;
    r113_settle(&mut driver, &|_| 0)?;
    // The Count spell left the hand and its draws came in.
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len() + 1,
        before + noted
    );
    let replayed = replay(&table().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r113_casey_and_raph_let_the_target_play_its_card_until_its_next_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wild = Arc::new(CardDefinition::new("Wild Land").with_type(CardType::Land));
    let deck: Vec<_> = (0..5).map(|_| Arc::clone(&wild)).collect();
    let table = || {
        let mut scenario = Scenario::new(seats)
            .battlefield(0, lands(5, mtg_testkit::cards::mountain))
            .hand(0, vec![card("Casey & Raph, Hotheads").unwrap()])
            .library(1, deck.clone());
        for seat in [0].into_iter().chain(2..seats) {
            scenario = scenario.library(seat, (0..4).map(|_| island()).collect::<Vec<_>>());
        }
        scenario
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Casey & Raph, Hotheads")?;
    let mut refused_same = false;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseMode(_) => {
                driver.answer(Answer::ChooseMode(mtg_engine::choose::ModeChoice::Modes(
                    vec![0, 1],
                )))?;
            }
            DecisionKind::ChooseTargets(options) if options.spec == 0 => {
                driver.answer(Answer::ChooseTargets(
                    mtg_engine::choose::TargetChoice::Targets(vec![driver.target_player(1)]),
                ))?;
            }
            DecisionKind::ChooseTargets(options) => {
                // Each mode targets a different player: the player of the first mode is not offered again.
                refused_same = !options.legal.contains(&driver.target_player(1));
                driver.answer(Answer::ChooseTargets(
                    mtg_engine::choose::TargetChoice::Targets(vec![driver.target_player(0)]),
                ))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                break;
            }
            _ => {
                driver.answer(r113_answer(&decision))?;
            }
        }
    }
    assert!(
        refused_same,
        "the second mode is not offered the first mode's player"
    );
    assert_eq!(r113_count(&driver, Zone::Battlefield, "Treasure"), 2);
    assert_eq!(r113_count(&driver, Zone::Exile, "Wild Land"), 1);
    // In its own turn the target plays the card for free.
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.decision()?.player, pid(1));
    let exiled = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Wild Land")
        .unwrap();
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority");
    };
    assert!(
        options.playable_lands.contains(&exiled),
        "the exiled card is playable"
    );
    Ok(())
}

/// Seat 0's payment automatically, every other question passively.
fn r113_answer(decision: &mtg_engine::decision::Decision) -> Answer {
    match decision.kind {
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        _ => Driver::passive_answer(decision),
    }
}

#[rstest]
fn r113_bell_borca_is_as_strong_as_the_greatest_value_noted_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let named = |name: &str, value: u32| creature(name, value, &[], 1, 1);
    let bell = "Bell Borca, Spectral Sergeant";
    let purge = r#"{"Exile":{"what":{"Each":[{"In":"Graveyard"}]}}}"#;
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card(bell).unwrap())])
        .graveyard(0, vec![named("Three", 3), named("Five", 5)])
        .graveyard(1, vec![named("Two", 2)])
        .hand(0, vec![spell("Purge", CardType::Instant, purge)])
        .library(0, (0..4).map(|_| island()).collect::<Vec<_>>())
        .library(1, (0..4).map(|_| island()).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(r113_stats(&driver, bell).0, 0, "nothing noted yet");
    driver.cast("Purge")?;
    r113_settle(&mut driver, &|_| 0)?;
    assert_eq!(r113_stats(&driver, bell).0, 5);
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(
        r113_stats(&driver, bell).0,
        0,
        "the notes of an earlier turn do not count"
    );
    Ok(())
}
