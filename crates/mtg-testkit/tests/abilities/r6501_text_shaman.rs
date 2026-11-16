//! Goal: Balduvian Shaman plays as printed at two to four seats: it targets only a white enchantment its controller
//! controls without cumulative upkeep, replaces one color word of that enchantment's text with another for good (CR
//! 612), and the enchantment gains "Cumulative upkeep {1}", whose first upkeep it is sacrificed unpaid.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A white enchantment for {W} with these rules.
fn r6501_enchantment(name: &str, json: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(rules),
    )
}

/// "Black creatures get +1/+1."
const R6501_ANTHEM: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Color":"Black"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;

/// Seat 0 controls Balduvian Shaman, the white Anthem, a white Elder Anthem with cumulative upkeep {1} and a black
/// enchantment; the last seat controls a white Anthem of its own, a black Brute and a red Hothead.
fn r6501_table(seats: usize) -> Scenario {
    let last = seats - 1;
    let elder = r#"{"Keyword":{"CumulativeUpkeep":"{1}"}}"#;
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Balduvian Shaman").unwrap()),
                ready(r6501_enchantment("Anthem", R6501_ANTHEM)),
                ready(Arc::new(
                    (*r6501_enchantment("Elder Anthem", R6501_ANTHEM))
                        .clone()
                        .with_rules(serde_json::from_str(elder).unwrap()),
                )),
                ready(Arc::new(
                    CardDefinition::new("Dark Anthem")
                        .with_type(CardType::Enchantment)
                        .with_mana_cost(cost(0, &[Color::Black])),
                )),
            ],
        )
        .battlefield(
            last,
            [
                ready(r6501_enchantment("Their Anthem", R6501_ANTHEM)),
                ready(creature("Brute", 0, &[Color::Black], 1, 1)),
                ready(creature("Hothead", 0, &[Color::Red], 1, 1)),
            ],
        )
}

/// The power and toughness of the named creature.
fn r6501_size(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Answers the two color questions and passes until the stack is empty.
fn r6501_resolve(driver: &mut Driver, words: &[Color]) -> Result<(), ScriptError> {
    let mut words = words.iter();
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            let word = words.next().expect("a color for every question");
            let at = options
                .options
                .iter()
                .position(|option| matches!(option, ChoiceOption::Color(color) if color == word))
                .expect("the color is offered");
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn r6501_balduvian_shaman_offers_only_its_own_white_enchantments_without_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6501_table(seats))?;
    driver.begin_activation("Balduvian Shaman")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    assert_eq!(options.legal, [driver.target_named("Anthem")?]);
    Ok(())
}

#[rstest]
fn r6501_balduvian_shaman_changes_a_color_word_and_grants_cumulative_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6501_table(seats))?;
    // The three white Anthems give the black Brute +1/+1 each.
    assert_eq!(r6501_size(&driver, "Brute"), (4, 4));
    assert_eq!(r6501_size(&driver, "Hothead"), (1, 1));
    driver.begin_activation("Balduvian Shaman")?;
    driver.choose_targets(&[driver.target_named("Anthem")?])?;
    r6501_resolve(&mut driver, &[Color::Black, Color::Red])?;
    assert_eq!(r6501_size(&driver, "Brute"), (3, 3));
    assert_eq!(r6501_size(&driver, "Hothead"), (2, 2));
    let anthem = driver.id_of("Anthem")?;
    let keywords =
        serde_json::to_string(&mtg_engine::keywords::keywords_of(driver.state(), anthem)).unwrap();
    assert_eq!(keywords, r#"[{"CumulativeUpkeep":"{1}"}]"#);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    // At seat 0's next upkeep it gets an age counter and, with no mana to pay, is sacrificed; the change stays gone.
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Draw)?;
    assert!(!driver.state().zone(Zone::Battlefield).contains(&anthem));
    assert_eq!(r6501_size(&driver, "Hothead"), (1, 1));
    Ok(())
}
