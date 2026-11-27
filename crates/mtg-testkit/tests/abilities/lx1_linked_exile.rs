//! Goal: the enter exiles the compiler links to "return the exiled card" (a land you choose, a permanent an opponent chooses, a creature exiled so the card is kept, a target held while any player pays the upkeep) play from Oracle text: the right player picks, the picked card is exiled, it returns when the card leaves, a card that is not exiled is not returned, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid, spell};

const RETURN: &str = "When this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control.";

/// A permanent for `{0}` with the abilities the compiler reads from this text; the card must read whole.
fn compiled(name: &str, card_type: CardType, type_line: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "4216f9ce-bdd9-4213-8c3f-e2ef4cd848f9".to_owned(),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let compiled = compile(&printed);
    assert!(
        compiled.clauses.iter().all(|clause| clause.read),
        "{compiled:?}"
    );
    let card = compiled.card.expect("the card compiles");
    let mut definition = CardDefinition::new(name)
        .with_type(card_type)
        .with_mana_cost(cost(0, &[]));
    if card_type == CardType::Creature {
        definition = definition.with_power_toughness(1, 1);
    }
    for ability in &card.faces[0].abilities {
        definition = definition.with_rules(ability.clone());
    }
    Arc::new(definition)
}

fn sorted(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut found = names(driver.state(), zone);
    found.sort();
    found
}

/// Answers until the stack is empty: a card choice picks `pick` and records who chose, a yes-no is `yes`, the rest
/// passively.
fn play_out(
    driver: &mut Driver,
    pick: &str,
    yes: bool,
    choosers: &mut Vec<usize>,
) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseCards(options) => {
                choosers.push(decision.player.index());
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| name_of(driver.state(), *id) == pick)
                    .take(1)
                    .collect();
                assert_eq!(chosen.len(), 1, "{pick} is offered");
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn smite() -> Arc<CardDefinition> {
    spell(
        "Smite",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    )
}

#[rstest]
#[case::newt_exiles_a_land_you_choose(
    "When this creature enters, exile a land you control.",
    "Island",
    true,
    0
)]
#[case::crab_exiles_what_the_opponent_chooses(
    "When this creature enters, an opponent chooses a permanent you control other than this creature and exiles it.",
    "Alpha",
    true,
    1
)]
#[case::drake_exiles_a_creature_to_stay(
    "When this creature enters, sacrifice it unless you exile a creature you control other than this creature.",
    "Alpha",
    true,
    0
)]
fn the_card_exiled_on_entering_returns_when_the_card_leaves(
    #[case] enters: &str,
    #[case] pick: &str,
    #[case] yes: bool,
    #[case] chooser: usize,
) -> Result<(), ScriptError> {
    let text = format!("{enters}\n{RETURN}");
    let wormfang = compiled("Wormfang", CardType::Creature, "Creature — Beast", &text);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [wormfang, smite()])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(forest()),
                    ready(free_creature("Alpha", 2, 2)),
                    ready(free_creature("Gamma", 1, 1)),
                ],
            )
            .battlefield(1, [ready(free_creature("Beta", 3, 3))]),
    )?;
    driver.cast("Wormfang")?;
    let mut choosers = Vec::new();
    play_out(&mut driver, pick, yes, &mut choosers)?;
    assert_eq!(choosers, [chooser], "one player picks the card");
    assert_eq!(sorted(&driver, Zone::Exile), [pick]);
    assert!(sorted(&driver, Zone::Battlefield).contains(&"Wormfang".to_owned()));

    let target = driver.target_named("Wormfang")?;
    driver.cast_at("Smite", &[target])?;
    play_out(&mut driver, pick, yes, &mut Vec::new())?;
    assert!(sorted(&driver, Zone::Exile).is_empty());
    assert_eq!(
        sorted(&driver, Zone::Battlefield),
        ["Alpha", "Beta", "Forest", "Gamma", "Island"]
    );
    let back = driver.id_of(pick)?;
    assert_eq!(driver.state().object(back).map(|o| o.owner()), Some(pid(0)));
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[test]
fn a_drake_kept_by_no_exile_is_sacrificed_and_returns_nothing() -> Result<(), ScriptError> {
    let text = format!(
        "When this creature enters, sacrifice it unless you exile a creature you control other than this creature.\n{RETURN}"
    );
    let drake = compiled("Wormfang", CardType::Creature, "Creature — Drake", &text);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [drake])
            .battlefield(0, [ready(free_creature("Alpha", 2, 2))]),
    )?;
    driver.cast("Wormfang")?;
    play_out(&mut driver, "Alpha", false, &mut Vec::new())?;
    assert_eq!(sorted(&driver, Zone::Battlefield), ["Alpha"]);
    assert!(sorted(&driver, Zone::Exile).is_empty());
    assert_eq!(sorted(&driver, Zone::Graveyard(pid(0))), ["Wormfang"]);
    Ok(())
}

#[test]
fn icy_prison_unpaid_at_upkeep_is_sacrificed_and_returns_the_creature() -> Result<(), ScriptError> {
    let text = "When this enchantment enters, exile target creature.\nAt the beginning of your upkeep, sacrifice this enchantment unless any player pays {3}.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control.";
    let prison = compiled("Icy Prison", CardType::Enchantment, "Enchantment", text);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [prison])
            .library(0, [island(), island(), island()])
            .library(1, [island(), island(), island()])
            .battlefield(1, [ready(free_creature("Beta", 3, 3))]),
    )?;
    driver.cast("Icy Prison")?;
    let mut asked = Vec::new();
    for _ in 0..30 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let beta = driver.target_named("Beta")?;
                driver.choose_targets(&[beta])?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(sorted(&driver, Zone::Exile), ["Beta"]);
    driver.idle_until(3, Step::Upkeep)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::YesNo(_) = decision.kind {
            asked.push(decision.player.index());
        }
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().turn().step() != Step::Upkeep
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(asked, [0, 1], "each player in turn order may pay");
    assert_eq!(sorted(&driver, Zone::Battlefield), ["Beta"]);
    assert_eq!(sorted(&driver, Zone::Graveyard(pid(0))), ["Icy Prison"]);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
