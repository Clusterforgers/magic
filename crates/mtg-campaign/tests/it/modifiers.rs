//! Goal: every encounter modifier of the pool loads from the content and does what its rule says at a table of two
//! heroes and the boss, its emblem on the boss's seat; the pool holds the 23 modifiers, each with a card; and the draw
//! never pairs a modifier with a boss its `not_with` names.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_campaign::content;
use mtg_campaign::content::CardKind;
use mtg_campaign::modifiers;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::keywords::has_keyword;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest, mountain, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use proptest::prelude::*;
use rstest::rstest;

/// The boss's seat; seats 0 and 1 are the heroes.
const BOSS: usize = 2;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

/// The table of two heroes and the boss, the modifier's emblem on the boss's seat.
fn table(id: &str) -> Scenario {
    let content = content().unwrap();
    let card = content.cards.get(id).unwrap();
    assert_eq!(card.kind, CardKind::Encounter);
    Scenario::new(3).emblems(BOSS, [card.definition()])
}

/// A card with one ability written in the card DSL.
fn spell(name: &str, ty: CardType, colors: &[Color], ability: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(ability).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(ty)
            .with_mana_cost(cost(0, colors))
            .with_rules(ability),
    )
}

fn named(driver: &Driver, seat: usize, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = state.object(*id).unwrap();
            object.controller() == pid(seat) && object.card().name == name
        })
        .collect()
}

fn resolve(driver: &mut Driver, events: &mut Vec<Event>) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            // Keep the targets of a copy.
            events.extend(driver.answer(Answer::YesNo(YesNoChoice(false)))?);
        } else {
            events.extend(driver.pass()?);
        }
    }
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn flips(events: &[Event]) -> Vec<bool> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect()
}

/// A hero bolts `target` (a seat, or a creature by name) with a Mountain; the events of the cast and resolution.
fn bolt(
    scenario: Scenario,
    target: Result<usize, &str>,
) -> Result<(Driver, Vec<Event>), ScriptError> {
    let mut driver = Driver::from_scenario(
        scenario
            .battlefield(0, [ready(mountain())])
            .hand(0, [card("Lightning Bolt").unwrap()]),
    )?;
    let target = match target {
        Ok(seat) => driver.target_player(seat),
        Err(name) => driver.target_named(name)?,
    };
    let mut events = driver.cast_at("Lightning Bolt", &[target])?;
    resolve(&mut driver, &mut events)?;
    Ok((driver, events))
}

fn mirror_hall(table: Scenario) -> Result<(), ScriptError> {
    let bears = (0..6).map(|_| ready(creature("Bear", 2, &[Color::Green], 2, 2)));
    let mut driver = Driver::from_scenario(table.battlefield(0, bears))?;
    driver.pass()?;
    assert_eq!(named(&driver, 0, "Bear").len(), 5);
    Ok(())
}

fn living_stone(table: Scenario) -> Result<(), ScriptError> {
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let mut driver = Driver::from_scenario(table.battlefield(0, [ready(relic)]))?;
    driver.tap_for_mana("Relic")?;
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 1);
    Ok(())
}

fn waking_earth(table: Scenario) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table.battlefield(1, [ready(forest())]))?;
    let land = named(&driver, 1, "Forest")[0];
    let chars = characteristics(driver.state(), land).unwrap();
    assert!(chars.is_creature() && chars.power == Some(1) && chars.toughness == Some(1));
    Ok(())
}

fn bloodlust(table: Scenario) -> Result<(), ScriptError> {
    let bear = ready(creature("Bear", 2, &[Color::Green], 2, 2));
    let driver = Driver::from_scenario(table.battlefield(BOSS, [bear]))?;
    let id = named(&driver, BOSS, "Bear")[0];
    assert!(has_keyword(driver.state(), id, &Keyword::DoubleStrike));
    Ok(())
}

fn twin_flames(table: Scenario) -> Result<(), ScriptError> {
    let before = life(&Driver::from_scenario(table.clone())?, BOSS);
    let (driver, _) = bolt(table, Ok(BOSS))?;
    assert_eq!(before - life(&driver, BOSS), 6);
    Ok(())
}

fn overflowing_mana(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table.battlefield(0, [ready(forest())]))?;
    driver.tap_for_mana("Forest")?;
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 2);
    Ok(())
}

fn deep_pockets(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table
            .at(2, 1, Step::Upkeep)
            .library(1, (0..5).map(|_| forest())),
    )?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(1))).len(), 2);
    Ok(())
}

fn quickening(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table
            .at(2, 0, Step::Upkeep)
            .battlefield(0, [ready(forest())])
            .hand(0, [creature("Bear", 0, &[Color::Green], 2, 2)]),
    )?;
    let mut events = driver.cast("Bear")?;
    resolve(&mut driver, &mut events)?;
    assert_eq!(named(&driver, 0, "Bear").len(), 1);
    Ok(())
}

fn one_voice(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table
            .battlefield(0, [ready(forest()), ready(forest())])
            .hand(
                0,
                [
                    creature("Bear", 0, &[Color::Green], 2, 2),
                    creature("Elf", 0, &[Color::Green], 1, 1),
                ],
            ),
    )?;
    let mut events = driver.cast("Bear")?;
    resolve(&mut driver, &mut events)?;
    assert!(driver.cast("Elf").is_err(), "a second spell this turn");
    Ok(())
}

fn narrow_pass(table: Scenario) -> Result<(), ScriptError> {
    let board = ["A", "B", "C"].map(|name| ready(creature(name, 1, &[Color::Green], 1, 1)));
    let mut driver =
        Driver::from_scenario(table.at(2, 0, Step::DeclareAttackers).battlefield(0, board))?;
    assert!(
        driver
            .attack(&[("A", BOSS), ("B", BOSS), ("C", BOSS)])
            .is_err(),
        "a third attacker"
    );
    Ok(())
}

fn second_spring(table: Scenario) -> Result<(), ScriptError> {
    let call = spell(
        "Call",
        CardType::Instant,
        &[Color::White],
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"power":1,"toughness":1}}}}]}}"#,
    );
    let mut driver =
        Driver::from_scenario(table.battlefield(0, [ready(plains())]).hand(0, [call]))?;
    let mut events = driver.cast("Call")?;
    resolve(&mut driver, &mut events)?;
    assert_eq!(named(&driver, 0, "Soldier").len(), 2);
    Ok(())
}

fn no_rest(table: Scenario) -> Result<(), ScriptError> {
    let (driver, events) = bolt(table.seed(3), Ok(BOSS))?;
    let flipped = flips(&events);
    let gone = driver.state().zone(Zone::Exile).len();
    assert_eq!((flipped.len(), gone), (1, usize::from(!flipped[0])));
    Ok(())
}

fn stone_sleep(table: Scenario) -> Result<(), ScriptError> {
    let giant = tapped(creature("Giant", 3, &[Color::Red], 3, 3));
    let driver =
        Driver::from_scenario(table.seed(3).at(2, 1, Step::Untap).battlefield(1, [giant]))?;
    let flipped = flips(driver.engine().events());
    let id = named(&driver, 1, "Giant")[0];
    let held = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!((flipped.len(), held), (1, !flipped[0]));
    Ok(())
}

fn prismatic_chaos(table: Scenario) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table)?;
    let drawn: Vec<bool> = (0..3)
        .map(|seat| mtg_engine::rules::cost_colors_drawn(driver.state(), pid(seat)))
        .collect();
    assert_eq!(drawn, [true, true, false]);
    Ok(())
}

fn endless_summer(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table.hand(0, [forest(), mountain()]))?;
    driver.play_land("Forest")?;
    driver.play_land("Mountain")?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 0);
    Ok(())
}

fn open_book(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table
            .battlefield(0, [ready(forest())])
            .library(0, [creature("Bear", 0, &[Color::Green], 2, 2), forest()]),
    )?;
    let mut events = driver.cast("Bear")?;
    resolve(&mut driver, &mut events)?;
    assert_eq!(named(&driver, 0, "Bear").len(), 1);
    Ok(())
}

fn thin_veil(table: Scenario) -> Result<(), ScriptError> {
    let seer = Arc::new(
        CardDefinition::new("Seer")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_power_toughness(1, 1)
            .with_rules(
                serde_json::from_str(
                    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#,
                )
                .unwrap(),
            ),
    );
    let mut driver = Driver::from_scenario(
        table
            .battlefield(0, [ready(mtg_testkit::cards::island())])
            .hand(0, [seer])
            .library(0, (0..3).map(|_| forest())),
    )?;
    let mut events = driver.cast("Seer")?;
    resolve(&mut driver, &mut events)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 0);
    Ok(())
}

fn shared_blood(table: Scenario) -> Result<(), ScriptError> {
    let bear = ready(creature("Bear", 2, &[Color::Green], 2, 2));
    let driver = Driver::from_scenario(table.battlefield(1, [bear]))?;
    let id = named(&driver, 1, "Bear")[0];
    assert!(
        characteristics(driver.state(), id)
            .unwrap()
            .all_creature_types
    );
    Ok(())
}

fn giants_table(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table
            .battlefield(0, [ready(forest())])
            .hand(0, [creature("Giant", 2, &[Color::Green], 3, 3)]),
    )?;
    let mut events = driver.cast("Giant")?;
    resolve(&mut driver, &mut events)?;
    assert_eq!(named(&driver, 0, "Giant").len(), 1);
    Ok(())
}

fn free_command(table: Scenario) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table
            .format(Arc::new(Commander))
            .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
            .battlefield(0, [ready(mountain()), ready(mountain())]),
    )?;
    let mut events = driver.cast("Cmd")?;
    resolve(&mut driver, &mut events)?;
    let object = driver.state().commanders().next().unwrap();
    driver.manual(ManualAction::Move {
        object,
        to: ManualZone::Graveyard,
    })?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert_eq!(driver.state().commander_tax(object), 0);
    Ok(())
}

fn echoing_spells(table: Scenario) -> Result<(), ScriptError> {
    let before = life(&Driver::from_scenario(table.clone())?, BOSS);
    let (driver, _) = bolt(table, Ok(BOSS))?;
    assert_eq!(before - life(&driver, BOSS), 6);
    Ok(())
}

fn no_mercy(table: Scenario) -> Result<(), ScriptError> {
    let salve = spell(
        "Salve",
        CardType::Instant,
        &[Color::White],
        r#"{"Spell":{"effects":[{"GainLife":{"amount":3}}]}}"#,
    );
    let mut driver = Driver::from_scenario(
        table
            .seed(3)
            .battlefield(0, [ready(plains())])
            .hand(0, [salve]),
    )?;
    let before = life(&driver, 0);
    let mut events = driver.cast("Salve")?;
    resolve(&mut driver, &mut events)?;
    let flipped = flips(&events);
    assert_eq!(
        (flipped.len(), life(&driver, 0) - before),
        (1, if flipped[0] { 3 } else { 0 })
    );
    Ok(())
}

fn haunted_ground(table: Scenario) -> Result<(), ScriptError> {
    let bear = ready(creature("Bear", 2, &[Color::Green], 2, 2));
    let (driver, _) = bolt(table.battlefield(1, [bear]), Err("Bear"))?;
    assert_eq!(named(&driver, 1, "Spirit").len(), 1);
    Ok(())
}

type Check = fn(Scenario) -> Result<(), ScriptError>;

#[rstest]
#[case::mirror_hall("mirror_hall", mirror_hall)]
#[case::living_stone("living_stone", living_stone)]
#[case::waking_earth("waking_earth", waking_earth)]
#[case::bloodlust("bloodlust", bloodlust)]
#[case::twin_flames("twin_flames", twin_flames)]
#[case::overflowing_mana("overflowing_mana", overflowing_mana)]
#[case::deep_pockets("deep_pockets", deep_pockets)]
#[case::quickening("quickening", quickening)]
#[case::one_voice("one_voice", one_voice)]
#[case::narrow_pass("narrow_pass", narrow_pass)]
#[case::second_spring("second_spring", second_spring)]
#[case::no_rest("no_rest", no_rest)]
#[case::stone_sleep("stone_sleep", stone_sleep)]
#[case::prismatic_chaos("prismatic_chaos", prismatic_chaos)]
#[case::endless_summer("endless_summer", endless_summer)]
#[case::open_book("open_book", open_book)]
#[case::thin_veil("thin_veil", thin_veil)]
#[case::shared_blood("shared_blood", shared_blood)]
#[case::giants_table("giants_table", giants_table)]
#[case::free_command("free_command", free_command)]
#[case::echoing_spells("echoing_spells", echoing_spells)]
#[case::no_mercy("no_mercy", no_mercy)]
#[case::haunted_ground("haunted_ground", haunted_ground)]
fn each_modifier_does_what_it_says(#[case] id: &str, #[case] check: Check) {
    check(table(id)).unwrap();
}

#[test]
fn the_pool_holds_the_23_modifiers_each_with_its_card() {
    let content = content().unwrap();
    let ids: BTreeSet<&str> = content.modifiers.iter().map(|m| m.id.as_str()).collect();
    assert_eq!((ids.len(), content.modifiers.len()), (23, 23));
    for id in ids {
        let card = content.cards.get(id).unwrap();
        assert_eq!(card.kind, CardKind::Encounter, "{id}");
        assert!(!card.abilities.is_empty() && !card.text.is_empty(), "{id}");
    }
}

/// Every boss a `not_with` names, and a boss no modifier excludes.
fn bosses() -> Vec<String> {
    let content = content().unwrap();
    let mut all: BTreeSet<String> = content
        .modifiers
        .iter()
        .flat_map(|m| m.not_with.iter().cloned())
        .collect();
    all.insert("kings_forge".into());
    all.into_iter().collect()
}

proptest! {
    #[test]
    fn the_draw_never_pairs_a_modifier_with_a_boss_it_excludes(
        seed in any::<u64>(),
        floor in 1u8..5,
        room in 0u8..12,
    ) {
        let pool = &content().unwrap().modifiers;
        for boss in bosses() {
            let drawn = modifiers::draw(pool, &boss, seed, floor, room).unwrap();
            prop_assert!(!drawn.not_with.contains(&boss), "{} with {}", drawn.id, boss);
        }
    }
}
