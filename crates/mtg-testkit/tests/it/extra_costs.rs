//! Goal: the optional costs beyond kicker's mana and the keyword costs with parts other than mana play whole: buyback returns the spell to its owner's hand only as it resolves (not when it is countered or fizzles), entwine chooses every mode and asks none, "as an additional cost, you may" and a kicker with a sacrifice are offered only when the parts can be paid, ask their picks and pay them before the spell is cast, flashback pays life and evoke exiles a card from the hand, what a kicked permanent paid is read by its trigger after a state-based action took the permanent away, every game replays to the same state, and the answers of a player who cannot pay are not offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, pay, resolve};
use crate::common::{names, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, CostsChoice, ModeChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(zone)
        .iter()
        .find(|id| common_name(driver, **id) == name)
        .unwrap_or_else(|| panic!("no {name} in {zone:?}"))
}

fn common_name(driver: &Driver, id: ObjectId) -> String {
    crate::common::name_of(driver.state(), id)
}

fn kick(driver: &mut Driver, costs: &[usize]) -> Result<Vec<Event>, ScriptError> {
    driver.expect_decision(0, "choose-costs")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs.to_vec())))
}

/// The cards the pending question offers for a cost, by name.
fn offered_picks(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected, not {}", driver.decision().unwrap().kind.name());
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| common_name(driver, *id))
        .collect();
    from.sort();
    from
}

fn pick(driver: &mut Driver, zone: Zone, name: &str) -> Result<Vec<Event>, ScriptError> {
    let id = id_in(driver, zone, name);
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        // A trigger with an optional target asks for it as it goes on the stack: none is chosen.
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            driver.choose_targets(&[])?;
        } else if matches!(driver.decision()?.kind, DecisionKind::OrderTriggers(_)) {
            driver.order_triggers(&[0, 1])?;
        } else if driver.state().stack().is_empty() {
            return Ok(());
        } else {
            driver.pass()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn zone_has(driver: &Driver, zone: Zone, name: &str) -> bool {
    names(driver.state(), zone).iter().any(|found| found == name)
}

/// How the opponent answers the spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answered {
    Nothing,
    Counters,
    KillsTheTarget,
}

/// A Bear, two Islands for Counterspell and two Swamps for Doom Blade.
fn opposition() -> Vec<mtg_engine::PresetPermanent> {
    let mut permanents = vec![ready(free_creature("Bear", 2, 2))];
    permanents.extend(lands(2, island));
    permanents.extend(lands(2, swamp));
    permanents
}

fn capsize_table(islands: usize) -> Scenario {
    Scenario::new(2)
        .hand_named(0, ["Capsize"])
        .battlefield(0, lands(islands, island))
        .hand_named(1, ["Counterspell", "Doom Blade"])
        .battlefield(1, opposition())
}

#[rstest]
#[case::resolved_and_bought_back(6, Some(true), Answered::Nothing, true)]
#[case::resolved_without_the_buyback(6, Some(false), Answered::Nothing, false)]
#[case::no_mana_for_the_buyback(5, None, Answered::Nothing, false)]
#[case::countered_with_the_buyback_paid(6, Some(true), Answered::Counters, false)]
#[case::fizzled_with_the_buyback_paid(6, Some(true), Answered::KillsTheTarget, false)]
fn buyback_returns_the_spell_to_the_hand_only_when_it_resolves(
    #[case] islands: usize,
    #[case] bought_back: Option<bool>,
    #[case] answer: Answered,
    #[case] in_hand: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(capsize_table(islands))?;
    driver.begin_cast("Capsize")?;
    match bought_back {
        Some(paid) => {
            let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
                panic!("the buyback is asked first");
            };
            assert_eq!(options.optional[0].label, "Buyback {3}");
            kick(&mut driver, if paid { &[0] } else { &[] })?;
        }
        None => driver.expect_decision(0, "choose-targets")?,
    }
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    assert_eq!(
        driver.pay_options()?.cost.mana_value(),
        if bought_back == Some(true) { 6 } else { 3 }
    );
    pay(&mut driver)?;
    match answer {
        Answered::Nothing => {}
        Answered::Counters => {
            driver.pass()?;
            let spell = driver.state().stack()[0];
            let target = Target::Object(driver.state().object(spell).unwrap().reference());
            driver.cast_at("Counterspell", &[target])?;
        }
        Answered::KillsTheTarget => {
            driver.pass()?;
            let bear = driver.target_named("Bear")?;
            driver.cast_at("Doom Blade", &[bear])?;
        }
    }
    settle(&mut driver)?;
    assert_eq!(zone_has(&driver, Zone::Hand(pid(0)), "Capsize"), in_hand);
    assert_eq!(
        zone_has(&driver, Zone::Graveyard(pid(0)), "Capsize"),
        !in_hand
    );
    // The spell does not go back to the hand and the graveyard both, and nothing is left on the stack.
    assert_eq!(driver.state().stack(), []);
    // A Capsize that resolved bounced the Bear; one that was answered left it as it was or dead.
    let resolved = answer == Answered::Nothing;
    assert_eq!(zone_has(&driver, Zone::Hand(pid(1)), "Bear"), resolved);
    replays(&driver, capsize_table(islands));
    Ok(())
}

#[test]
fn a_spell_bought_back_can_be_cast_again_and_asks_for_the_buyback_again() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(capsize_table(12))?;
    for _ in 0..2 {
        driver.begin_cast("Capsize")?;
        kick(&mut driver, &[0])?;
        let land = id_in(&driver, Zone::Battlefield, "Island");
        let target = Target::Object(driver.state().object(land).unwrap().reference());
        driver.choose_targets(&[target])?;
        pay(&mut driver)?;
        resolve(&mut driver)?;
        assert!(zone_has(&driver, Zone::Hand(pid(0)), "Capsize"));
    }
    Ok(())
}

fn grip_table(islands: usize) -> Scenario {
    Scenario::new(2)
        .hand_named(0, ["Dream's Grip"])
        .battlefield(0, lands(islands, island))
        .battlefield(
            1,
            [
                ready(free_creature("Ready Bear", 2, 2)),
                tapped(free_creature("Tired Bear", 2, 2)),
            ],
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
#[case::entwined(2, Some(true), true, true)]
#[case::not_entwined_asks_the_mode(2, Some(false), false, false)]
#[case::no_mana_for_the_entwine(1, None, false, false)]
fn entwine_chooses_every_mode_and_asks_none(
    #[case] islands: usize,
    #[case] entwine: Option<bool>,
    #[case] both: bool,
    #[case] untapped_the_tired_bear: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(grip_table(islands))?;
    driver.begin_cast("Dream's Grip")?;
    if let Some(paid) = entwine {
        let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
            panic!("the entwine comes before the modes");
        };
        assert_eq!(options.optional[0].label, "Entwine {1}");
        kick(&mut driver, if paid { &[0] } else { &[] })?;
    }
    if both {
        // Every mode is chosen: a target for each, in the order they print, and no mode is asked.
        let ready = driver.target_named("Ready Bear")?;
        driver.choose_targets(&[ready])?;
        let tired = driver.target_named("Tired Bear")?;
        driver.choose_targets(&[tired])?;
        assert_eq!(driver.pay_options()?.cost.mana_value(), 2);
    } else {
        driver.expect_decision(0, "choose-mode")?;
        driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))?;
        let ready = driver.target_named("Ready Bear")?;
        driver.choose_targets(&[ready])?;
        assert_eq!(driver.pay_options()?.cost.mana_value(), 1);
    }
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert!(is_tapped(&driver, "Ready Bear"));
    assert_eq!(!is_tapped(&driver, "Tired Bear"), untapped_the_tired_bear);
    replays(&driver, grip_table(islands));
    Ok(())
}

fn surge_table(artifacts: usize, creatures: usize) -> Scenario {
    let mut battlefield: Vec<_> = (0..artifacts)
        .map(|_| ready(mtg_testkit::corpus::card("Bonesplitter").unwrap()))
        .collect();
    battlefield.extend(lands(2, mountain));
    battlefield.extend((0..creatures).map(|_| ready(free_creature("Pet", 1, 1))));
    Scenario::new(2)
        .hand_named(0, ["Voltage Surge"])
        .battlefield(0, battlefield)
        .battlefield(1, [ready(free_creature("Giant", 4, 4))])
}

#[rstest]
#[case::no_artifact_to_sacrifice(0, false, false)]
#[case::declined(2, true, false)]
#[case::paid_with_a_pick(2, true, true)]
#[case::the_only_artifact_needs_no_pick(1, true, true)]
fn an_optional_additional_cost_is_offered_only_when_its_part_can_be_paid(
    #[case] artifacts: usize,
    #[case] offered: bool,
    #[case] paid: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(surge_table(artifacts, 0))?;
    driver.begin_cast("Voltage Surge")?;
    if offered {
        let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
            panic!("the additional cost is asked");
        };
        assert_eq!(options.optional[0].label, "Sacrifice an artifact");
        kick(&mut driver, if paid { &[0] } else { &[] })?;
        if paid && artifacts > 1 {
            assert_eq!(offered_picks(&driver), ["Bonesplitter", "Bonesplitter"]);
            let id = driver.state().zone(Zone::Battlefield)[0];
            driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
        }
    } else {
        driver.expect_decision(0, "choose-targets")?;
    }
    let giant = driver.target_named("Giant")?;
    driver.choose_targets(&[giant])?;
    let events = pay(&mut driver)?;
    let sacrificed = events.iter().position(|event| {
        matches!(
            event,
            Event::ZoneChanged {
                cause: mtg_engine::events::ZoneCause::Sacrificed,
                ..
            }
        )
    });
    let cast = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert_eq!(sacrificed.is_some(), paid);
    if let Some(sacrificed) = sacrificed {
        assert!(sacrificed < cast, "the artifact is paid before the spell is cast");
    }
    resolve(&mut driver)?;
    // Four damage kill the Giant, two do not.
    assert_eq!(zone_has(&driver, Zone::Battlefield, "Giant"), !paid);
    let left = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Bonesplitter")
        .count();
    assert_eq!(left, artifacts - usize::from(paid));
    replays(&driver, surge_table(artifacts, 0));
    Ok(())
}

fn swamps_and_two_creatures() -> Vec<mtg_engine::PresetPermanent> {
    let mut permanents = lands(2, swamp);
    permanents.extend([
        ready(free_creature("Pet", 1, 1)),
        ready(free_creature("Cub", 1, 1)),
    ]);
    permanents
}

#[rstest]
#[case::unkicked_shrinks_a_little(false, true)]
#[case::kicked_kills(true, false)]
fn a_kicker_that_sacrifices_a_creature_asks_which_and_pays_it_before_the_cast(
    #[case] kicked: bool,
    #[case] survives: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand_named(0, ["Vicious Offering"])
            .battlefield(0, swamps_and_two_creatures())
            .battlefield(1, [ready(free_creature("Ogre", 5, 5))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Vicious Offering")?;
    let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
        panic!("the kicker is asked");
    };
    assert_eq!(options.optional[0].label, "Kicker Sacrifice a creature");
    kick(&mut driver, if kicked { &[0] } else { &[] })?;
    if kicked {
        // Only the creatures of the player are offered, in the order the cost asks for them.
        assert_eq!(offered_picks(&driver), ["Cub", "Pet"]);
        pick(&mut driver, Zone::Battlefield, "Cub")?;
    }
    let ogre = driver.target_named("Ogre")?;
    driver.choose_targets(&[ogre])?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(zone_has(&driver, Zone::Battlefield, "Ogre"), survives);
    assert_eq!(zone_has(&driver, Zone::Battlefield, "Cub"), !kicked);
    assert!(zone_has(&driver, Zone::Battlefield, "Pet"));
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn the_kicker_that_sacrifices_is_not_offered_to_a_player_with_no_creature_to_give() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Vicious Offering"])
            .battlefield(0, lands(2, swamp))
            .battlefield(1, [ready(free_creature("Ogre", 5, 5))]),
    )?;
    driver.begin_cast("Vicious Offering")?;
    driver.expect_decision(0, "choose-targets")?;
    Ok(())
}

fn analysis_table(life: i32, islands: usize) -> Scenario {
    Scenario::new(2)
        .graveyard_named(0, ["Deep Analysis"])
        .battlefield(0, lands(islands, island))
        .library(0, (0..6).map(|_| forest()))
        .life(0, life)
}

#[rstest]
#[case::paid(20, 2, true)]
#[case::one_life_to_spare(4, 2, true)]
#[case::not_enough_life(2, 2, false)]
#[case::not_enough_mana(20, 1, false)]
fn flashback_with_life_is_offered_only_when_the_life_can_be_paid(
    #[case] life: i32,
    #[case] islands: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(analysis_table(life, islands))?;
    assert_eq!(driver.begin_cast("Deep Analysis").is_ok(), offered);
    if !offered {
        return Ok(());
    }
    let me = driver.target_player(0);
    driver.choose_targets(&[me])?;
    assert_eq!(driver.pay_options()?.cost.mana_value(), 2);
    let events = pay(&mut driver)?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::LifePaid { amount: 3, .. }
    )));
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life - 3);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 2);
    assert_eq!(names(driver.state(), Zone::Exile), ["Deep Analysis"]);
    replays(&driver, analysis_table(life, islands));
    Ok(())
}

fn endurance_table(hand: &[&str]) -> Scenario {
    Scenario::new(2)
        .hand_named(0, std::iter::once("Endurance").chain(hand.iter().copied()))
        .battlefield(0, lands(3, forest))
        .graveyard_named(1, ["Brainstorm"])
}

#[rstest]
#[case::two_green_cards_ask_which(&["Giant Growth", "Rampant Growth"], true)]
#[case::one_green_card_needs_no_pick(&["Giant Growth"], false)]
fn evoke_exiles_a_card_from_the_hand_and_the_permanent_is_sacrificed(
    #[case] hand: &[&str],
    #[case] asks: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(endurance_table(hand))?;
    driver.begin_cast("Endurance")?;
    choose_way(&mut driver, "Evoke")?;
    if asks {
        assert_eq!(offered_picks(&driver), ["Giant Growth", "Rampant Growth"]);
        pick(&mut driver, Zone::Hand(pid(0)), "Rampant Growth")?;
    }
    assert_eq!(driver.pay_options()?.cost.mana_value(), 0);
    let events = pay(&mut driver)?;
    let exiled = events.iter().position(|event| {
        matches!(event, Event::ZoneChanged { to: Zone::Exile, .. })
    });
    let cast = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert!(exiled.unwrap() < cast, "the card is exiled before the spell is cast");
    // The three Forests are untouched: the evoke cost has no mana.
    assert_eq!(
        driver.state().zone(Zone::Battlefield).iter().filter(|id| {
            driver.state().object(**id).unwrap().permanent().unwrap().tapped()
        }).count(),
        0
    );
    settle(&mut driver)?;
    assert!(
        zone_has(&driver, Zone::Graveyard(pid(0)), "Endurance"),
        "{:?} {:?} {:?}",
        names(driver.state(), Zone::Battlefield),
        names(driver.state(), Zone::Exile),
        driver.decision().map(|d| d.kind.name())
    );
    assert_eq!(
        zone_has(&driver, Zone::Exile, "Rampant Growth"),
        asks
    );
    assert_eq!(zone_has(&driver, Zone::Exile, "Giant Growth"), !asks);
    replays(&driver, endurance_table(hand));
    Ok(())
}

#[test]
fn evoke_with_a_card_to_exile_is_not_offered_when_the_hand_has_no_other_green_card() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(endurance_table(&["Lightning Bolt"]))?;
    driver.begin_cast("Endurance")?;
    // Only the mana cost: no question about the way.
    assert_eq!(driver.pay_options()?.cost.mana_value(), 3);
    Ok(())
}

#[rstest]
fn the_kicks_of_a_permanent_that_a_state_based_action_removed_are_read_by_its_trigger(
    #[values(true, false)] kicked: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand_named(0, ["Gatekeeper of Malakir"])
            .battlefield(0, lands(4, swamp))
            .battlefield(1, [ready(free_creature("Wall", 0, 20))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    // Every creature gets -9/-9 for the turn: the Gatekeeper dies as it enters, before its trigger goes on the stack,
    // and the Wall does not.
    driver.add_effect(
        Selector::AllCreatures,
        Modification::ModifyPowerToughness {
            power: -9,
            toughness: -9,
        },
        Duration::UntilEndOfTurn,
    )?;
    driver.begin_cast("Gatekeeper of Malakir")?;
    kick(&mut driver, if kicked { &[0] } else { &[] })?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    if kicked {
        // The trigger asks for its target as it goes on the stack, after the permanent is gone.
        assert!(!zone_has(&driver, Zone::Battlefield, "Gatekeeper of Malakir"));
        let foe = driver.target_player(1);
        driver.choose_targets(&[foe])?;
    }
    settle(&mut driver)?;
    assert!(zone_has(&driver, Zone::Graveyard(pid(0)), "Gatekeeper of Malakir"));
    assert_eq!(
        zone_has(&driver, Zone::Graveyard(pid(1)), "Wall"),
        kicked,
        "the trigger reads the kick of the permanent that was"
    );
    replays(&driver, scenario());
    Ok(())
}

#[test]
fn bot_games_with_the_cards_that_pay_parts_keep_every_invariant_and_use_them() {
    use crate::common::seeds::for_each_seed;
    use mtg_testkit::{PlayOptions, extra_cost_fuzz_setup, play_game_with};
    let seen = std::sync::Mutex::new((0u64, 0u64, 0u64, 0u64));
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                extra_cost_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    // Spells paid with an optional cost, picks included.
                    Event::SpellCast { choices, .. } if !choices.optional_costs.is_empty() => {
                        seen.0 += 1;
                        if choices.cost_picks().iter().any(|picked| !picked.is_empty()) {
                            seen.1 += 1;
                        }
                    }
                    Event::LifePaid { amount: 3, .. } => seen.2 += 1,
                    Event::ZoneChanged {
                        from: Zone::Stack,
                        to: Zone::Hand(_),
                        ..
                    } => seen.3 += 1,
                    _ => {}
                }
            }
        }
    });
    let (paid, picked, life, bought_back) = seen.into_inner().unwrap();
    assert!(paid > 0, "some spells paid an optional cost");
    assert!(picked > 0, "some optional cost took objects the player picked");
    assert!(life > 0, "some cost paid 3 life");
    assert!(bought_back > 0, "some spell returned to a hand as it resolved");
}

#[test]
fn the_pick_from_the_hand_is_asked_of_its_player_alone_refuses_a_card_not_offered_and_logs_nothing_until_paid(
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(endurance_table(&["Giant Growth", "Rampant Growth"]))?;
    driver.begin_cast("Endurance")?;
    choose_way(&mut driver, "Evoke")?;
    // The question is for the seat that casts the spell, and nothing of the hand is in the log yet.
    driver.expect_decision(0, "choose-cards")?;
    let logged = driver.engine().events().len();
    let not_offered = id_in(&driver, Zone::Battlefield, "Forest");
    let refused = driver.answer(Answer::ChooseCards(CardsChoice(vec![not_offered])));
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(driver.engine().events().len(), logged);
    driver.expect_decision(0, "choose-cards")?;
    // The card is exiled in the open once the spell is paid for, as every exile is.
    pick(&mut driver, Zone::Hand(pid(0)), "Giant Growth")?;
    assert_eq!(driver.engine().events().len(), logged);
    let events = pay(&mut driver)?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ZoneChanged { to: Zone::Exile, cause: mtg_engine::events::ZoneCause::Exiled, .. }
    )));
    Ok(())
}

/// An instant that deals 3 damage with madness {R} and 2 life.
fn painful_temper() -> std::sync::Arc<mtg_cards::CardDefinition> {
    use mtg_cards::ability::Ability;
    use mtg_cards::cost::{Cost, CostItem};
    use mtg_cards::keyword::Keyword;
    use mtg_cards::value::Amount;
    use mtg_core::card::CardType;
    let spell = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#;
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Painful Temper")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_core::mana::ManaCost::parse("{1}{R}").unwrap())
            .with_rules(serde_json::from_str(spell).unwrap())
            .with_rules(Ability::Keyword(Keyword::Madness(Cost(vec![
                CostItem::Mana("{R}".parse().unwrap()),
                CostItem::PayLife(Amount::Fixed(2)),
            ])))),
    )
}

#[rstest]
#[case::paid(20, true)]
#[case::not_enough_life_goes_to_the_graveyard(1, false)]
fn a_madness_cost_with_life_is_paid_inside_the_resolution_and_asked_only_when_it_can_be(
    #[case] life: i32,
    #[case] cast: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(
                0,
                [
                    mtg_testkit::corpus::card("Faithless Looting").unwrap(),
                    painful_temper(),
                    forest(),
                ],
            )
            .battlefield(0, lands(3, mountain))
            .library(0, (0..6).map(|_| forest()))
            .life(0, life)
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Faithless Looting")?;
    resolve(&mut driver)?;
    let temper = id_in(&driver, Zone::Hand(pid(0)), "Painful Temper");
    let other = id_in(&driver, Zone::Hand(pid(0)), "Forest");
    driver.answer(Answer::ChooseCards(CardsChoice(vec![temper, other])))?;
    settle_triggers(&mut driver)?;
    if !cast {
        assert!(zone_has(&driver, Zone::Graveyard(pid(0)), "Painful Temper"));
        return Ok(());
    }
    driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(true)))?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    let events = pay(&mut driver)?;
    let life_paid = events
        .iter()
        .position(|event| matches!(event, Event::LifePaid { amount: 2, .. }))
        .expect("the life of the madness cost is paid");
    let spell = events
        .iter()
        .position(|event| matches!(event, Event::SpellCast { .. }))
        .unwrap();
    assert!(life_paid < spell);
    resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 18);
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    replays(&driver, scenario());
    Ok(())
}

/// Lets the madness trigger of a discard resolve until the player is asked about the cast.
fn settle_triggers(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..6 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_))
            || driver.state().stack().is_empty()
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}
