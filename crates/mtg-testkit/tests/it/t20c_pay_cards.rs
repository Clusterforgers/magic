//! Goal: a creature whose trigger makes the opponent who cast a spell pay {1} as many times as its power (the payment of {X} where X is its power) asks for that much generic mana, its controller drawing only when the payment is refused; Flare of Fortitude freezes the life total of its controller and gives their permanents hexproof and indestructible for the turn; Dawn's Truce gives hexproof, and indestructible only when the gift was promised.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::choose::{CostsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::keywords::has_keyword;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn yes_no(yes: bool) -> Answer {
    Answer::YesNo(YesNoChoice(yes))
}

/// Passes priority while something waits on the stack and the table asks only for priority.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

/// Passes priority until `seat` holds it with nothing else asked.
fn until_held_by(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(seat)
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

/// Passes priority until some player is asked a question other than priority.
fn settle_to_question(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        driver.pass()?;
    }
    Ok(())
}

fn question(driver: &Driver) -> String {
    match &driver.decision().unwrap().kind {
        DecisionKind::YesNo(options) => options.text.clone(),
        other => panic!("a yes-or-no question was expected, not {}", other.name()),
    }
}

/// A 1/1 creature with "Whenever an opponent casts a spell, draw a card unless that player pays {X}, where X is this
/// creature's power", the rules as the checker reads them back.
fn sentinel() -> Arc<CardDefinition> {
    let rules = r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"effects":[{"Pay":{"who":"Triggering","cost":"{1}","times":{"Stat":{"of":"This","stat":"Power"}},"unpaid":[{"Draw":{}}]}}]}}"#;
    Arc::new(
        CardDefinition::new("Sentinel")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

/// Seat 0 controls the Sentinel with `extra` more power; seat 1 holds a Lightning Bolt and four lands.
fn sentinel_table(extra: i32) -> Result<Driver, ScriptError> {
    let mut lands = vec![ready(mountain())];
    lands.extend(vec![ready(forest()); 3]);
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(sentinel())])
        .library_named(0, ["Opt", "Brainstorm", "Doom Blade"])
        .hand_named(1, ["Lightning Bolt"])
        .battlefield(1, lands);
    let mut driver = Driver::from_scenario(scenario)?;
    if extra > 0 {
        let creature = driver.id_of("Sentinel")?;
        driver.add_effect(
            driver.on(creature),
            Modification::ModifyPowerToughness {
                power: extra,
                toughness: 0,
            },
            Duration::Forever,
        )?;
    }
    until_held_by(&mut driver, 1)?;
    Ok(driver)
}

#[rstest]
#[case::power_one(0, "Pay {1}?", 1)]
#[case::power_two(1, "Pay {2}?", 2)]
#[case::power_three(2, "Pay {3}?", 3)]
fn the_caster_is_asked_for_mana_equal_to_the_power(
    #[case] extra: i32,
    #[case] asked: &str,
    #[case] lands: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let mut driver = sentinel_table(extra)?;
    let target = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[target])?;
    settle_to_question(&mut driver)?;
    assert_eq!(driver.decision()?.player, pid(1), "the caster is asked");
    assert_eq!(question(&driver), asked);
    driver.answer(yes_no(pays))?;
    if pays {
        driver.pay_tapping(&["Forest", "Forest", "Forest"][..lands], 0)?;
    }
    settle(&mut driver)?;
    let drawn = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(
        drawn,
        if pays {
            Vec::<String>::new()
        } else {
            vec!["Opt".to_owned()]
        }
    );
    Ok(())
}

/// Seat 0 controls a Bear and `lands` Plains and holds `card`; seat 1 holds a Lightning Bolt and a Mountain.
fn shield_table(card: &str, lands: usize) -> Result<Driver, ScriptError> {
    let mut field = vec![ready(free_creature("Bear", 2, 2))];
    field.extend((0..lands).map(|_| ready(plains())));
    let scenario = Scenario::new(2)
        .battlefield(0, field)
        .hand_named(0, [card])
        .battlefield(1, [ready(mountain())])
        .library_named(1, ["Opt", "Brainstorm"])
        .hand_named(1, ["Lightning Bolt"]);
    Driver::from_scenario(scenario)
}

#[test]
fn flare_of_fortitude_freezes_life_and_protects_the_permanents() -> Result<(), ScriptError> {
    let mut driver = shield_table("Flare of Fortitude", 4)?;
    driver.cast("Flare of Fortitude")?;
    settle(&mut driver)?;
    let bear = driver.id_of("Bear")?;
    assert!(has_keyword(driver.state(), bear, &Keyword::Hexproof));
    assert!(has_keyword(driver.state(), bear, &Keyword::Indestructible));
    let life = driver.state().player(pid(0)).unwrap().life();
    until_held_by(&mut driver, 1)?;
    let target = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[target])?;
    settle(&mut driver)?;
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life,
        "the life total cannot change"
    );
    Ok(())
}

#[rstest]
#[case::without_the_gift(false)]
#[case::with_the_gift(true)]
fn dawns_truce_gives_hexproof_and_indestructible_only_when_the_gift_was_promised(
    #[case] promised: bool,
) -> Result<(), ScriptError> {
    let mut driver = shield_table("Dawn's Truce", 2)?;
    driver.begin_cast("Dawn's Truce")?;
    let costs = match &driver.decision()?.kind {
        DecisionKind::ChooseCosts(options) if promised => vec![options.optional[0].index],
        DecisionKind::ChooseCosts(_) => Vec::new(),
        other => panic!("the gift is asked, not {}", other.name()),
    };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let bear = driver.id_of("Bear")?;
    assert!(has_keyword(driver.state(), bear, &Keyword::Hexproof));
    assert_eq!(
        has_keyword(driver.state(), bear, &Keyword::Indestructible),
        promised
    );
    Ok(())
}
