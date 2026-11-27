//! Goal: the linked exiles the compiler reads from their own words (a card you pick from a revealed hand, cards from a single graveyard, a target and its namesakes, creatures from the battlefield and graveyards together, and the cast prohibition on the exiled card's name) play from Oracle text: the picked cards are exiled, the namesakes go too, the prohibition holds only while the card is exiled, everything returns where the card says when it leaves, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::compile::compile;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid, spell};

/// A permanent for `{0}` with the abilities the compiler reads from this text; the card must read whole.
fn compiled(name: &str, card_type: CardType, type_line: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "2677c0d9-1251-478e-b157-072859c593a9".to_owned(),
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

/// The objects in `zone` with one of these names, each as a target.
fn targets_in(driver: &Driver, zone: Zone, wanted: &[&str]) -> Vec<Target> {
    let mut found = Vec::new();
    let mut left: Vec<&str> = wanted.to_vec();
    for id in driver.state().zone(zone).iter().copied() {
        let name = name_of(driver.state(), id);
        if let Some(at) = left.iter().position(|want| *want == name) {
            left.remove(at);
            let object = driver.state().object(id).unwrap();
            found.push(Target::Object(object.reference()));
        }
    }
    found
}

/// Answers until the stack is empty: targets are `targets`, a card choice picks `pick`, a yes-no is yes, the rest
/// passively.
fn play_out(driver: &mut Driver, targets: &[Target], pick: &str) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::ChooseCards(options) => {
                assert_eq!(decision.player.index(), 0, "the controller picks");
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
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// Destroys target permanent.
fn shatter() -> Arc<CardDefinition> {
    spell(
        "Shatter",
        CardType::Instant,
        r#"[{"what":{"Object":[]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    )
}

/// Casts Shatter at the permanent named `name` and plays it out.
fn shatter_it(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let target = driver.target_named(name)?;
    driver.cast_at("Shatter", &[target])?;
    play_out(driver, &[], "")
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[test]
fn the_card_picked_from_a_revealed_hand_returns_to_that_hand() -> Result<(), ScriptError> {
    let text = "When this creature enters, target opponent reveals their hand and you choose a nonland card from it. Exile that card.\nWhen this creature leaves the battlefield, return the exiled card to its owner's hand.";
    let sculler = compiled(
        "Sculler",
        CardType::Creature,
        "Artifact Creature — Zombie",
        text,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [sculler, shatter()])
            .hand(1, [island(), free_creature("Beta", 2, 2)]),
    )?;
    driver.cast("Sculler")?;
    let opponent = driver.target_player(1);
    play_out(&mut driver, &[opponent], "Beta")?;
    assert_eq!(sorted(&driver, Zone::Exile), ["Beta"]);
    assert_eq!(sorted(&driver, Zone::Hand(pid(1))), ["Island"]);
    shatter_it(&mut driver, "Sculler")?;
    assert_eq!(sorted(&driver, Zone::Exile), Vec::<String>::new());
    assert_eq!(sorted(&driver, Zone::Hand(pid(1))), ["Beta", "Island"]);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::both_from_the_opponent(&["Beta", "Gamma"], &[], true)]
#[case::one_from_each_graveyard(&["Beta"], &["Alpha"], false)]
fn the_cards_of_a_single_graveyard_return_to_it(
    #[case] theirs: &[&str],
    #[case] mine: &[&str],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let text = "When this creature enters, exile up to two target cards from a single graveyard.\nWhen this creature leaves the battlefield, return the exiled cards to their owner's graveyard.";
    let gouger = compiled(
        "Gouger",
        CardType::Creature,
        "Creature — Nightmare Horror",
        text,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [gouger, shatter()])
            .graveyard(0, [free_creature("Alpha", 1, 1)])
            .graveyard(
                1,
                [free_creature("Beta", 2, 2), free_creature("Gamma", 1, 1)],
            ),
    )?;
    driver.cast("Gouger")?;
    let mut targets = targets_in(&driver, Zone::Graveyard(pid(1)), theirs);
    targets.extend(targets_in(&driver, Zone::Graveyard(pid(0)), mine));
    if !legal {
        // Cards of two graveyards are not one choice (CR 115.3).
        for _ in 0..10 {
            if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
                assert!(driver.choose_targets(&targets).is_err());
                return Ok(());
            }
            driver.answer_passively()?;
        }
        panic!("the trigger asks for targets");
    }
    play_out(&mut driver, &targets, "")?;
    assert_eq!(sorted(&driver, Zone::Exile), ["Beta", "Gamma"]);
    shatter_it(&mut driver, "Gouger")?;
    assert_eq!(sorted(&driver, Zone::Exile), Vec::<String>::new());
    assert_eq!(sorted(&driver, Zone::Graveyard(pid(1))), ["Beta", "Gamma"]);
    replays(&driver);
    Ok(())
}

#[test]
fn a_target_and_its_namesakes_are_exiled_and_return_together() -> Result<(), ScriptError> {
    let text = "When this enchantment enters, you may exile target nonland permanent not named Sphere and all other permanents with the same name as that permanent.\nWhen this enchantment leaves the battlefield, return the exiled cards to the battlefield under their owner's control.";
    let sphere = compiled("Sphere", CardType::Enchantment, "Enchantment", text);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [sphere, shatter()])
            .battlefield(0, [ready(free_creature("Beta", 1, 1))])
            .battlefield(
                1,
                [
                    ready(free_creature("Beta", 2, 2)),
                    ready(free_creature("Gamma", 3, 3)),
                ],
            ),
    )?;
    driver.cast("Sphere")?;
    let beta = targets_in(&driver, Zone::Battlefield, &["Beta"]);
    play_out(&mut driver, &beta, "")?;
    assert_eq!(sorted(&driver, Zone::Exile), ["Beta", "Beta"]);
    assert_eq!(sorted(&driver, Zone::Battlefield), ["Gamma", "Sphere"]);
    shatter_it(&mut driver, "Sphere")?;
    assert_eq!(
        sorted(&driver, Zone::Battlefield),
        ["Beta", "Beta", "Gamma"]
    );
    assert_eq!(driver.state().zone(Zone::Battlefield).len(), 3);
    replays(&driver);
    Ok(())
}

#[test]
fn creatures_from_the_battlefield_and_graveyards_return_to_their_owners_hands()
-> Result<(), ScriptError> {
    let text = "Flying\nWhen this creature enters, you may exile up to three other target creatures from the battlefield and/or creature cards from graveyards.\nWhen this creature leaves the battlefield, return the exiled cards to their owners' hands.";
    let angel = compiled("Angel", CardType::Creature, "Creature — Angel", text);
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [angel, shatter()])
            .graveyard(0, [free_creature("Alpha", 1, 1)])
            .battlefield(1, [ready(free_creature("Beta", 2, 2))]),
    )?;
    driver.cast("Angel")?;
    let mut targets = targets_in(&driver, Zone::Battlefield, &["Beta"]);
    targets.extend(targets_in(&driver, Zone::Graveyard(pid(0)), &["Alpha"]));
    play_out(&mut driver, &targets, "")?;
    assert_eq!(sorted(&driver, Zone::Exile), ["Alpha", "Beta"]);
    shatter_it(&mut driver, "Angel")?;
    assert_eq!(sorted(&driver, Zone::Exile), Vec::<String>::new());
    assert_eq!(sorted(&driver, Zone::Hand(pid(0))), ["Alpha"]);
    assert_eq!(sorted(&driver, Zone::Hand(pid(1))), ["Beta"]);
    replays(&driver);
    Ok(())
}

/// Whether the player with priority now may cast the card named `name` from their hand.
fn may_cast(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name))
}

#[test]
fn the_opponents_cannot_cast_the_exiled_cards_name_while_it_is_exiled() -> Result<(), ScriptError> {
    let text = "When this enchantment enters, exile target creature.\nYour opponents can't cast spells with the same name as the exiled card.\nWhen this enchantment leaves the battlefield, return the exiled card to its owner's hand.";
    let erasure = compiled("Erasure", CardType::Enchantment, "Enchantment", text);
    let bolt = |name: &str| {
        spell(
            name,
            CardType::Instant,
            "[]",
            r#"[{"GainLife":{"who":"You","amount":1}}]"#,
        )
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [erasure, shatter()])
            .hand(1, [bolt("Beta"), bolt("Gamma")])
            .battlefield(1, [ready(free_creature("Beta", 2, 2))]),
    )?;
    driver.cast("Erasure")?;
    let beta = targets_in(&driver, Zone::Battlefield, &["Beta"]);
    play_out(&mut driver, &beta, "")?;
    assert_eq!(sorted(&driver, Zone::Exile), ["Beta"]);
    driver.pass()?;
    assert_eq!(driver.decision()?.player.index(), 1);
    assert!(
        !may_cast(&driver, "Beta")?,
        "the exiled card's name is shut"
    );
    assert!(may_cast(&driver, "Gamma")?, "another name stays open");
    driver.pass()?;
    shatter_it(&mut driver, "Erasure")?;
    assert_eq!(
        sorted(&driver, Zone::Hand(pid(1))),
        ["Beta", "Beta", "Gamma"]
    );
    driver.pass()?;
    assert!(
        may_cast(&driver, "Beta")?,
        "the name opens once the card leaves exile"
    );
    replays(&driver);
    Ok(())
}
