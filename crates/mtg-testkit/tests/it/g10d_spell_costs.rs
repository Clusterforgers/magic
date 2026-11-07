//! Goal: a cost change for a time (`GameRule::SpellCost`, "artifact spells you cast this turn cost {1} less", "spells your opponents cast cost {1} more until your next turn") changes every matching spell while it lasts, not only the next one, reads "you" as the rule's controller at 2 to 4 seats, and leaves other spells alone; "the color or colors of your choice" sets the colours chosen one at a time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, CostChange};
use mtg_cards::modify::Duration;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::Answer;
use mtg_engine::rules::Rule;
use mtg_testkit::cards::{cost, creature, forest};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free sorcery with the target specifications `targets` and the effects `effects` (JSON).
fn sorcery(name: &str, targets: &str, effects: &str) -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"targets":[{targets}],"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

/// The `SpellCost` rule over `applies_to` (JSON) by {1}, for the turn.
fn prelude(applies_to: &str, change: &str) -> Arc<CardDefinition> {
    sorcery(
        "Prelude",
        "",
        &format!(
            r#"[{{"Rule":{{"rule":{{"SpellCost":{{"applies_to":{applies_to},"change":"{change}","mana":"{{1}}"}}}},"duration":"UntilEndOfTurn"}}}}]"#
        ),
    )
}

fn written(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// Every seat passes in turn: the top of the stack resolves.
fn pass_all(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts Prelude, then `before` Grizzly Bears (resolving each), and starts casting one more: its total cost.
fn bears_cost(
    seats: usize,
    prelude: Arc<CardDefinition>,
    before: usize,
) -> Result<String, ScriptError> {
    let mut hand = vec![prelude];
    for _ in 0..=before {
        hand.push(corpus::card("Grizzly Bears").unwrap());
    }
    let scenario = Scenario::new(seats)
        .battlefield(0, lands(8, forest))
        .hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Prelude")?;
    pass_all(&mut driver, seats)?;
    for _ in 0..before {
        driver.cast("Grizzly Bears")?;
        pass_all(&mut driver, seats)?;
    }
    driver.begin_cast("Grizzly Bears")?;
    Ok(driver.pay_options()?.cost.to_string())
}

const YOUR_CREATURES: &str = r#"["Spell",{"Type":"Creature"},{"ControlledBy":"You"}]"#;

#[rstest]
#[case::creatures_you_cast_cost_less(YOUR_CREATURES, "Reduce", 0, "{G}")]
#[case::the_second_creature_too(YOUR_CREATURES, "Reduce", 1, "{G}")]
#[case::the_third_creature_too(YOUR_CREATURES, "Reduce", 2, "{G}")]
#[case::artifacts_leave_creatures(
    r#"["Spell",{"Type":"Artifact"},{"ControlledBy":"You"}]"#,
    "Reduce",
    0,
    "{1}{G}"
)]
#[case::your_opponents_spells_leave_yours(
    r#"["Spell",{"ControlledBy":"Opponents"}]"#,
    "Increase",
    0,
    "{1}{G}"
)]
#[case::every_spell_costs_more(r#"["Spell"]"#, "Increase", 1, "{2}{G}")]
fn a_spell_cost_rule_changes_every_matching_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] applies_to: &str,
    #[case] change: &str,
    #[case] before: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    assert_eq!(
        bears_cost(seats, prelude(applies_to, change), before)?,
        written(expected)
    );
    Ok(())
}

/// A rule an opponent's permanent made reads "your opponents" from that opponent: seat 0's spell costs more.
#[rstest]
fn the_rule_of_an_opponent_taxes_your_spells(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, lands(8, forest))
        .battlefield(1, vec![ready(creature("Taxer", 1, &[], 1, 1))])
        .hand(0, vec![corpus::card("Grizzly Bears").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let taxer = driver.id_of("Taxer")?;
    let applies_to = serde_json::from_str(r#"["Spell",{"ControlledBy":"Opponents"}]"#).unwrap();
    driver.add_rule_from(
        taxer,
        Rule::SpellCost {
            applies_to,
            change: CostChange::Increase,
            mana: "{2}".parse().unwrap(),
        },
        Duration::UntilYourNextTurn,
    )?;
    driver.begin_cast("Grizzly Bears")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), written("{3}{G}"));
    Ok(())
}

/// Seat 0 casts a sorcery that makes Grizzly Bears the colours it picks, answering each question with `picks` (the
/// index among the options asked), and the Bears' colours after it resolves.
fn colors_after(seats: usize, picks: &[usize]) -> Result<Vec<Color>, ScriptError> {
    let paint = sorcery(
        "Paint",
        r#"{"what":{"Object":[{"Type":"Creature"}]}}"#,
        r#"[{"ChooseValue":{"kind":"Colors","as":"hue"}},{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetColorsChosen":"hue"}]}}]"#,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(corpus::card("Grizzly Bears").unwrap())])
        .hand(0, vec![paint]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Paint", &[target])?;
    pass_all(&mut driver, seats)?;
    for pick in picks {
        driver.answer(Answer::ChooseOption(OptionChoice(*pick)))?;
    }
    let id = driver.id_of("Grizzly Bears")?;
    let colors = characteristics(driver.state(), id).unwrap().colors;
    Ok(Color::ALL
        .into_iter()
        .filter(|color| colors.contains(*color))
        .collect())
}

#[rstest]
// The first question offers the five colours; each later one "no more" first, then the colours not chosen yet.
#[case::one_color(&[0, 0], &[Color::White])]
#[case::two_colors(&[0, 1, 0], &[Color::White, Color::Blue])]
#[case::a_later_color(&[4, 1, 0], &[Color::White, Color::Green])]
#[case::every_color(&[0, 1, 1, 1, 1], &Color::ALL)]
fn the_color_or_colors_of_your_choice(
    #[values(2, 4)] seats: usize,
    #[case] picks: &[usize],
    #[case] expected: &[Color],
) -> Result<(), ScriptError> {
    assert_eq!(colors_after(seats, picks)?, expected);
    Ok(())
}
