//! Goal: preventions that name their damage and a token replacement for one kind of token play whole at tables of two, three and four seats with the burn or the cards on the next seat or the last one: Panther Habit and the shield of Gatta and Luzzu turn the damage into that many +1/+1 counters, and with a shield that prevents the next 2 damage the controller of the creature picks the order (CR 616.1), the used-up shield ends and the other takes the rest; damage that can't be prevented passes them; Temple Altisaur leaves 1 of the damage to another Dinosaur and none of its own; Divine Visitation turns only the creature tokens of a spell into as many Angels, once each, in either order with Anointed Procession; Kalitas sacrifices another Vampire or Zombie and its Zombie becomes an Angel; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{ReplacementChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The seat that holds the other permanents: the next one or the last one.
const fn holder(seats: usize, next_seat: bool) -> usize {
    if next_seat { 1 } else { seats - 1 }
}

fn spell(name: &str, kind: CardType, color: Color, json: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[color]))
            .with_rules(rules),
    )
}

/// An instant for `{W}`: "Prevent the next 2 damage that would be dealt to target creature this turn."
fn ward() -> Arc<CardDefinition> {
    spell(
        "Ward",
        CardType::Instant,
        Color::White,
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Shield":{"when":{"Damage":{"to":{"Target":0}}},"by":{"PreventUpTo":2},"duration":"UntilEndOfTurn"}}]}}"#,
    )
}

/// A sorcery for `{W}`: two 1/1 white Soldier creature tokens and a Treasure-like artifact token.
fn muster() -> Arc<CardDefinition> {
    spell(
        "Muster",
        CardType::Sorcery,
        Color::White,
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":2}},{"CreateToken":{"token":{"Custom":{"name":"Gold","types":["Artifact"],"subtypes":["Gold"]}}}}]}}"#,
    )
}

/// An enchantment: "Damage can't be prevented."
fn unpreventable() -> Arc<CardDefinition> {
    spell(
        "Fury",
        CardType::Enchantment,
        Color::Red,
        r#"{"Rule":{"rule":{"DamageCantBePrevented":{}}}}"#,
    )
}

fn raptor() -> PresetPermanent {
    ready(Arc::new(
        CardDefinition::new("Raptor")
            .with_type(CardType::Creature)
            .with_subtype("Dinosaur")
            .with_power_toughness(3, 3),
    ))
}

/// Passes until a player has priority over an empty stack: `target` answers each choice of targets, `first` names the
/// source whose replacement goes first; returns how many times the order was asked.
fn settle(driver: &mut Driver, target: Option<&Target>, first: &str) -> Result<usize, ScriptError> {
    let mut ordered = 0;
    for _ in 0..300 {
        let decision = driver.engine().decision().map(|d| d.kind.clone());
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        match decision {
            Some(DecisionKind::Priority(_)) if idle => return Ok(ordered),
            Some(DecisionKind::Priority(_)) => {
                driver.pass()?;
            }
            Some(DecisionKind::ChooseTargets(_)) if target.is_some() => {
                driver
                    .choose_targets(target.into_iter().copied().collect::<Vec<_>>().as_slice())?;
            }
            Some(DecisionKind::YesNo(_)) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            Some(DecisionKind::ChooseReplacement(options)) => {
                ordered += 1;
                let pick = options
                    .candidates
                    .iter()
                    .position(|c| {
                        driver
                            .state()
                            .object(c.source)
                            .map(|o| o.card().name.clone())
                            == Some(first.to_owned())
                    })
                    .unwrap_or(0);
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(pick)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn object(driver: &Driver, name: &str) -> Target {
    let id = driver.id_of(name).unwrap();
    Target::Object(driver.state().object(id).unwrap().reference())
}

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

fn damage(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

fn replays(setup: &Scenario, driver: &Driver) {
    let setup = setup.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 bolts the creature `name` with its Mountain.
fn bolt(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let target = object(driver, name);
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Mountain"], 0)?;
    Ok(())
}

#[rstest]
fn panther_habit_turns_the_damage_into_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
    #[values(1, 3)] amount: i32,
    #[values(false, true)] fury: bool,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let mut mine = vec![ready(mountain())];
    if fury {
        mine.push(ready(unpreventable()));
    }
    let game = Scenario::new(seats)
        .hand(0, [burn("Bolt", amount)])
        .battlefield(0, mine)
        .battlefield(
            seat,
            [
                ready(card("Grizzly Bears").unwrap()),
                attached_to(ready(card("Panther Habit").unwrap()), seat, 0),
            ],
        );
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    bolt(&mut driver, "Grizzly Bears")?;
    settle(&mut driver, None, "")?;
    let dead = names(driver.state(), Zone::Graveyard(pid(seat))).contains(&"Grizzly Bears".into());
    if fury {
        assert_eq!(dead, amount >= 2, "damage that can't be prevented is dealt");
    } else {
        assert!(!dead);
        assert_eq!(counters(&driver, "Grizzly Bears"), amount.unsigned_abs());
        assert_eq!(damage(&driver, "Grizzly Bears"), 0);
    }
    replays(&game, &driver);
    Ok(())
}

/// Seat 0 casts the named spell, at the Bears when it targets, pays automatically and lets the stack resolve.
fn cast_on(
    driver: &mut Driver,
    name: &str,
    bears: &Target,
    targets: bool,
    first: &str,
) -> Result<usize, ScriptError> {
    driver.begin_cast(name)?;
    if targets {
        driver.choose_targets(std::slice::from_ref(bears))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver, Some(bears), first)
}

#[rstest]
fn the_shield_of_gatta_and_luzzu_and_a_used_up_shield_go_in_the_order_picked(
    #[values(2, 3, 4)] seats: usize,
    #[values("Gatta and Luzzu", "Ward")] first: &str,
) -> Result<(), ScriptError> {
    let mut lands = vec![ready(mountain())];
    lands.extend((0..4).map(|_| ready(plains())));
    lands.push(ready(card("Grizzly Bears").unwrap()));
    let game = Scenario::new(seats)
        .hand(
            0,
            [burn("Bolt", 3), card("Gatta and Luzzu").unwrap(), ward()],
        )
        .battlefield(0, lands);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    let bears = object(&driver, "Grizzly Bears");
    cast_on(&mut driver, "Gatta and Luzzu", &bears, false, first)?;
    cast_on(&mut driver, "Ward", &bears, true, first)?;
    let ordered = cast_on(&mut driver, "Bolt", &bears, true, first)?;
    assert_eq!(
        ordered, 1,
        "the controller of the Bears picks the order once"
    );
    let (counters_now, ward_left) = if first == "Ward" {
        (1, false)
    } else {
        (3, true)
    };
    assert_eq!(counters(&driver, "Grizzly Bears"), counters_now);
    assert_eq!(damage(&driver, "Grizzly Bears"), 0);
    let expired = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::RuleExpired { .. }));
    assert_eq!(expired, !ward_left, "the Ward used up ends");
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn temple_altisaur_leaves_one_damage_to_another_dinosaur(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
    #[values("Raptor", "Temple Altisaur")] hit: &str,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let game = Scenario::new(seats)
        .hand(0, [burn("Bolt", 2)])
        .battlefield(0, [ready(mountain())])
        .battlefield(seat, [ready(card("Temple Altisaur").unwrap()), raptor()]);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    bolt(&mut driver, hit)?;
    settle(&mut driver, None, "")?;
    let expected = if hit == "Raptor" { 1 } else { 2 };
    assert_eq!(damage(&driver, hit), expected);
    replays(&game, &driver);
    Ok(())
}

fn tokens(driver: &Driver, seat: usize) -> Vec<String> {
    let mut found: Vec<String> = names(driver.state(), Zone::Battlefield)
        .into_iter()
        .zip(driver.state().zone(Zone::Battlefield))
        .filter(|(_, id)| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.controller() == pid(seat) && o.kind() == ObjectKind::Token)
        })
        .map(|(name, _)| name)
        .collect();
    found.sort();
    found
}

#[rstest]
fn divine_visitation_turns_only_creature_tokens_into_angels(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some("Divine Visitation"), Some("Anointed Procession"))] first: Option<&str>,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(plains()), ready(card("Divine Visitation").unwrap())];
    if first.is_some() {
        mine.push(ready(card("Anointed Procession").unwrap()));
    }
    let game = Scenario::new(seats)
        .hand(0, [muster()])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    driver.cast("Muster")?;
    let ordered = settle(&mut driver, None, first.unwrap_or(""))?;
    let times = if first.is_some() { 2 } else { 1 };
    let mut expected = vec!["Angel".to_owned(); 2 * times];
    expected.extend(vec!["Gold".to_owned(); times]);
    expected.sort();
    assert_eq!(tokens(&driver, 0), expected);
    assert!(ordered <= 2, "each replacement applies once to each event");
    let angel = driver.id_of("Angel").unwrap();
    let angel = driver.state().object(angel).unwrap();
    assert_eq!(
        (angel.card().power, angel.card().toughness),
        (Some(4), Some(4)),
        "a 4/4 Angel"
    );
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn kalitas_sacrifices_another_vampire_or_zombie(
    #[values(2, 3, 4)] seats: usize,
    #[values("Zombie", "Vampire", "Elf")] kind: &str,
) -> Result<(), ScriptError> {
    let fodder = ready(Arc::new(
        CardDefinition::new("Fodder")
            .with_type(CardType::Creature)
            .with_subtype(kind)
            .with_power_toughness(1, 1),
    ));
    let swamps = (0..3).map(|_| ready(mtg_testkit::cards::swamp()));
    let mut mine: Vec<PresetPermanent> = swamps.collect();
    mine.push(ready(card("Kalitas, Traitor of Ghet").unwrap()));
    mine.push(fodder);
    let game = Scenario::new(seats).battlefield(0, mine);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    let offered = !driver
        .offered_abilities("Kalitas, Traitor of Ghet")?
        .is_empty();
    assert_eq!(
        offered,
        kind != "Elf",
        "only another Vampire or Zombie pays"
    );
    if offered {
        driver.begin_activation("Kalitas, Traitor of Ghet")?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        settle(&mut driver, None, "")?;
        assert_eq!(counters(&driver, "Kalitas, Traitor of Ghet"), 2);
        assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Fodder".into()));
    }
    replays(&game, &driver);
    Ok(())
}

#[rstest]
fn the_zombie_of_kalitas_becomes_an_angel(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] next_seat: bool,
) -> Result<(), ScriptError> {
    let seat = holder(seats, next_seat);
    let game = Scenario::new(seats)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(card("Kalitas, Traitor of Ghet").unwrap()),
                ready(card("Divine Visitation").unwrap()),
            ],
        )
        .battlefield(seat, [ready(card("Grizzly Bears").unwrap())]);
    let mut driver = Driver::from_scenario(game.clone())?;
    driver.pass_until(Step::Main1)?;
    bolt(&mut driver, "Grizzly Bears")?;
    settle(&mut driver, None, "")?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Grizzly Bears"]);
    assert_eq!(tokens(&driver, 0), ["Angel"]);
    replays(&game, &driver);
    Ok(())
}
