//! Goal: the cleave cards of the rmt0 pack play at two to four seats as their two texts say: Dread Fugue reveals the
//! target player's hand and its caster picks the nonland card that player discards (mana value 2 or less unless it was
//! cleaved), Wash Away counters only a spell not cast from a hand unless it was cleaved, and Lantern Flare's X is the
//! number of its caster's creatures, or the X announced for its cleave cost; every game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;

use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The game replays from its setup and events to the same state.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::for_its_mana_cost("Cast", &["Cheap"])]
#[case::cleaved("Cleave", &["Cheap", "Pricey"])]
fn dread_fugue_lets_its_caster_pick_the_discarded_card(
    #[case] way: &str,
    #[case] offered: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Dread Fugue").unwrap()])
        .battlefield(0, lands(3, swamp))
        .hand(
            victim,
            [
                creature("Cheap", 1, &[Color::Black], 2, 2),
                creature("Pricey", 3, &[Color::Black], 4, 4),
                swamp(),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Dread Fugue")?;
    choose_way(&mut driver, way)?;
    driver.choose_targets(&[driver.target_player(victim)])?;
    pay(&mut driver)?;
    for _ in 0..seats {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?.clone();
    // One card of the kind leaves nothing to choose.
    let picked = if let DecisionKind::ChooseCards(options) = &decision.kind {
        assert_eq!(decision.player, pid(0), "the caster chooses");
        let mut seen: Vec<String> = options
            .from
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect();
        seen.sort();
        assert_eq!(seen, offered);
        // The costliest card offered: "Pricey" when it may be taken.
        let last = *options
            .from
            .iter()
            .max_by_key(|id| name_of(driver.state(), **id).len())
            .unwrap();
        let picked = name_of(driver.state(), last);
        driver.answer(Answer::ChooseCards(CardsChoice(vec![last])))?;
        picked
    } else {
        assert_eq!(offered.len(), 1, "{:?}", decision.kind.name());
        offered[0].to_owned()
    };
    assert!(names(driver.state(), Zone::Graveyard(pid(victim))).contains(&picked));
    assert_eq!(names(driver.state(), Zone::Hand(pid(victim))).len(), 2);
    replays(&driver);
    Ok(())
}

/// An instant for {U} that does nothing, with flashback {U}.
fn ripple() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ripple")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(serde_json::from_str(r#"{"Spell":{"effects":[]}}"#).unwrap())
            .with_rules(serde_json::from_str(r#"{"Keyword":{"Flashback":"{U}"}}"#).unwrap()),
    )
}

#[rstest]
#[case::a_hand_spell_for_its_mana_cost("Cast", false, false)]
#[case::a_flashback_spell_for_its_mana_cost("Cast", true, true)]
#[case::a_hand_spell_cleaved("Cleave", false, true)]
#[case::a_flashback_spell_cleaved("Cleave", true, true)]
fn wash_away_counters_a_spell_from_a_hand_only_cleaved(
    #[case] way: &str,
    #[case] flashback: bool,
    #[case] legal: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Wash Away").unwrap()])
        .battlefield(0, lands(5, island));
    scenario = if flashback {
        scenario.graveyard(0, [ripple()])
    } else {
        scenario.hand(0, [card("Wash Away").unwrap(), ripple()])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Ripple")?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    driver.begin_cast("Wash Away")?;
    // A way whose text has no legal target is not offered (CR 601.2c).
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        let offered = ways(&driver)?;
        assert_eq!(
            offered.iter().any(|found| found.starts_with(way)),
            legal,
            "{offered:?}"
        );
        if !legal {
            return Ok(());
        }
        choose_way(&mut driver, way)?;
    } else if way == "Cast" {
        assert!(!legal, "only the cleave cost is offered");
        return Ok(());
    }
    let ripple = driver.state().stack()[0];
    let target = Target::Object(driver.state().object(ripple).unwrap().reference());
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    for _ in 0..seats * 2 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let gone = if flashback {
        Zone::Exile
    } else {
        Zone::Graveyard(pid(0))
    };
    assert!(names(driver.state(), gone).contains(&"Ripple".to_owned()));
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::for_its_mana_cost_x_is_its_creatures("Cast", None, 2)]
#[case::cleaved_x_is_announced("Cleave", Some(3), 3)]
fn lantern_flare_deals_and_gains_x(
    #[case] way: &str,
    #[case] announced: Option<u32>,
    #[case] amount: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Lantern Flare").unwrap()])
        .battlefield(0, {
            let mut permanents = lands(3, mountain);
            permanents.extend(lands(3, plains));
            permanents.push(ready(creature("First", 0, &[Color::White], 1, 1)));
            permanents.push(ready(creature("Second", 0, &[Color::White], 1, 1)));
            permanents
        })
        .battlefield(
            victim,
            [ready(creature("Victim", 0, &[Color::Green], 9, 9))],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Lantern Flare")?;
    choose_way(&mut driver, way)?;
    if let Some(x) = announced {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    driver.choose_targets(&[driver.target_named("Victim")?])?;
    pay(&mut driver)?;
    for _ in 0..seats * 2 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let victim = driver.state().object(driver.id_of("Victim")?).unwrap();
    assert_eq!(victim.permanent().unwrap().damage(), amount);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life + i32::try_from(amount).unwrap()
    );
    replays(&driver);
    Ok(())
}
