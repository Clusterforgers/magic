//! Goal: the engine pieces of round 5 builder g play at 2 to 4 seats: `{Q}` untaps a tapped source only, "sacrifice any number" may take some or none, a discard the hand counts is paid at resolution, a storage land adds one mana for each counter removed, and mana abilities with a drawback or a life change run in one payment without clashing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaType;
use mtg_engine::Answer;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use common::{names, pid};

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn artifact(name: &str, abilities: &[&str]) -> Arc<CardDefinition> {
    abilities
        .iter()
        .fold(
            CardDefinition::new(name).with_type(CardType::Artifact),
            |card, json| card.with_rules(rules(json)),
        )
        .into()
}

fn land(name: &str, abilities: &[&str]) -> Arc<CardDefinition> {
    abilities
        .iter()
        .fold(
            CardDefinition::new(name).with_type(CardType::Land),
            |card, json| card.with_rules(rules(json)),
        )
        .into()
}

fn cards(prefix: &str, count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|at| free_creature(&format!("{prefix} {at}"), 1, 1))
        .collect()
}

/// Passes the decisions the stack asks until it is empty, answering a yes or no with `yes` and a pick of cards with `pick`.
fn settle(
    driver: &mut Driver,
    yes: bool,
    pick: &dyn Fn(&Driver, &[ObjectId]) -> Vec<ObjectId>,
) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked = pick(driver, &options.from);
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
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

const UNTAP_GAIN: &str =
    r#"{"Activated":{"cost":["Untap"],"effects":[{"GainLife":{"amount":1}}]}}"#;

#[rstest]
fn an_untap_cost_is_offered_for_a_tapped_source_only_and_untaps_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] starts_tapped: bool,
) -> Result<(), ScriptError> {
    let gadget = artifact("Gadget", &[UNTAP_GAIN]);
    let preset = if starts_tapped {
        tapped(gadget)
    } else {
        ready(gadget)
    };
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, [preset]))?;
    let offered = !driver.offered_abilities("Gadget")?.is_empty();
    assert_eq!(offered, starts_tapped, "{{Q}} needs a tapped source");
    if !offered {
        assert!(driver.begin_activation("Gadget").is_err());
        return Ok(());
    }
    driver.begin_activation("Gadget")?;
    assert!(!is_tapped(&driver, "Gadget"), "the cost untaps the source");
    settle(&mut driver, true, &|_, _| Vec::new())?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    assert!(
        driver.offered_abilities("Gadget")?.is_empty(),
        "once untapped, it is not offered again"
    );
    Ok(())
}

#[rstest]
fn the_untapped_source_is_not_one_of_the_permanents_its_cost_untaps(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] another: bool,
) -> Result<(), ScriptError> {
    let burr = artifact(
        "Burr",
        &[
            r#"{"Activated":{"cost":["Untap",{"UntapPermanents":{"filter":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ],
    );
    let mut field = vec![tapped(burr)];
    if another {
        field.push(tapped(artifact("Cog", &[])));
    }
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, field))?;
    assert_eq!(!driver.offered_abilities("Burr")?.is_empty(), another);
    if another {
        driver.begin_activation("Burr")?;
        settle(&mut driver, true, &|_, from| from.to_vec())?;
        assert!(!is_tapped(&driver, "Burr") && !is_tapped(&driver, "Cog"));
        assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    }
    Ok(())
}

#[rstest]
fn a_summoning_sick_creature_cannot_pay_an_untap_cost(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let creature: Arc<CardDefinition> = CardDefinition::new("Pixie")
        .with_type(CardType::Creature)
        .with_rules(rules(UNTAP_GAIN))
        .into();
    let preset = PresetPermanent {
        tapped: true,
        summoning_sick: true,
        ..PresetPermanent::new(creature)
    };
    let driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, [preset]))?;
    assert_eq!(driver.offered_abilities("Pixie")?, Vec::<usize>::new());
    Ok(())
}

#[rstest]
fn any_number_of_creatures_may_be_sacrificed_even_none(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 3)] taken: usize,
) -> Result<(), ScriptError> {
    let altar = artifact(
        "Altar",
        &[
            r#"{"Activated":{"cost":["Tap"],"effects":[{"SacrificeUpTo":{"filter":[{"Type":"Creature"}],"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"gone"}},{"GainLife":{"amount":{"Bound":"gone~moved"}}}]}}"#,
        ],
    );
    let mut field: Vec<PresetPermanent> = vec![ready(altar)];
    field.extend(cards("Bear", 3).into_iter().map(ready));
    let scenario = Scenario::new(seats).battlefield(0, field).battlefield(
        1,
        cards("Foe", 1).into_iter().map(ready).collect::<Vec<_>>(),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Altar")?;
    settle(&mut driver, true, &|_, from| {
        from.iter().take(taken).copied().collect()
    })?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.len(), taken, "{graveyard:?}");
    assert!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .any(|n| n == "Foe 0"),
        "only your creatures"
    );
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        20 + i32::try_from(taken).unwrap()
    );
    Ok(())
}

#[rstest]
fn discarding_the_hand_is_counted_as_the_payment_resolves(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] hand: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let barterer = artifact(
        "Barterer",
        &[
            r#"{"Activated":{"cost":["Tap"],"effects":[{"Pay":{"cost":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}],"paid":[{"Draw":{"count":2}}]}}]}}"#,
        ],
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(barterer)])
        .hand(0, cards("Held", hand))
        .hand(1, cards("Theirs", 2))
        .library(0, cards("Deep", 5));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Barterer")?;
    settle(&mut driver, pays, &|_, from| from.to_vec())?;
    let held = names(driver.state(), Zone::Hand(pid(0)));
    let discarded = names(driver.state(), Zone::Graveyard(pid(0)));
    if pays {
        assert_eq!((held.len(), discarded.len()), (2, hand), "{held:?}");
    } else {
        assert_eq!((held.len(), discarded.len()), (hand, 0));
    }
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(1))).len(),
        2,
        "only the payer's hand counts"
    );
    Ok(())
}

#[rstest]
fn a_storage_land_adds_one_mana_for_each_counter_it_removes(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] removed: u32,
) -> Result<(), ScriptError> {
    let vault = land(
        "Vault",
        &[
            r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"count":"X","counter":"charge"}}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":"X"}}]}}"#,
        ],
    );
    let preset = mtg_testkit::with_counters(ready(vault), CounterKind::Charge, 3);
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, [preset]))?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority first");
    };
    let mut counts: Vec<u32> = options
        .mana_abilities
        .iter()
        .filter(|option| option.produces == ManaType::Colorless)
        .map(|option| option.count)
        .collect();
    counts.sort_unstable();
    assert_eq!(counts, vec![1, 2, 3], "one way for each number of counters");
    let source = options
        .mana_abilities
        .iter()
        .find(|option| option.count == removed)
        .unwrap()
        .source;
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(source)))?;
    let vault = driver.id_of("Vault")?;
    let left = driver
        .state()
        .object(vault)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Charge);
    assert_eq!(left, 3 - removed);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().mana().total(),
        removed
    );
    assert!(is_tapped(&driver, "Vault"));
    Ok(())
}

const MARSH: [&str; 2] = [
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}},{"Continuous":{"affected":"This","duration":"NextUntapStep","mods":["DoesntUntap"]}}]}}"#,
];

#[rstest]
fn two_lands_that_skip_their_next_untap_pay_one_spell_together(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let spell: Arc<CardDefinition> = CardDefinition::new("Dark Draw")
        .with_type(CardType::Sorcery)
        .with_mana_cost(mtg_core::mana::ManaCost::parse("{B}{B}").unwrap())
        .with_rules(rules(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#))
        .into();
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(land("Marsh A", &MARSH)),
                ready(land("Marsh B", &MARSH)),
            ],
        )
        .hand(0, [spell])
        .library(0, cards("Deep", 5));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Dark Draw")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, true, &|_, _| Vec::new())?;
    assert_eq!(
        driver.state().effects().len(),
        2,
        "one effect for each land"
    );
    // Each land stays tapped through its controller's next untap step.
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    assert!(is_tapped(&driver, "Marsh A") && is_tapped(&driver, "Marsh B"));
    driver.idle_until(u32::try_from(2 * seats).unwrap() + 1, Step::Main1)?;
    assert!(!is_tapped(&driver, "Marsh A") && !is_tapped(&driver, "Marsh B"));
    Ok(())
}

#[rstest]
fn a_land_that_feeds_the_opponents_gives_each_of_them_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let grove = land(
        "Grove",
        &[
            r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}},{"GainLife":{"amount":1,"who":"Opponents"}}]}}"#,
        ],
    );
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, [ready(grove)]))?;
    driver.tap_for_mana_of("Grove", ManaType::Green)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    for seat in 1..seats {
        assert_eq!(driver.state().player(pid(seat)).unwrap().life(), 21);
    }
    Ok(())
}
