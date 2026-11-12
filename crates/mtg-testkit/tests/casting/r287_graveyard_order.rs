//! Goal: the cards above a card in its owner's graveyard (`Amount::CardsAbove`) count only the cards put there after it,
//! in the order the graveyard keeps, at two to four seats and replayed to the live state: "with a creature card directly
//! above it" (Krovikan Horror, Death Spark) looks at the one card on top of it, "with three or more creature cards above
//! it" (Nether Shadow) at every card above it, and a card below it never counts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn card(name: &str, kind: CardType, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// A card that gains its owner 1 life at their end step while it lies in their graveyard with enough creature cards above
/// it.
fn horror(directly: bool, at: u32) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{{"Compare":{{"left":{{"CardsAbove":{{"filter":[{{"Type":"Creature"}}],"directly":{directly}}}}},"op":"Ge","right":{at}}}}},"from":"Graveyard","effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    card("Horror", CardType::Creature, &[&rule])
}

/// The graveyard of seat 0 from bottom to top, `H` the horror, `C` a creature card and `S` a sorcery card.
fn graveyard(order: &str, directly: bool, at: u32) -> Vec<Arc<CardDefinition>> {
    order
        .chars()
        .map(|letter| match letter {
            'H' => horror(directly, at),
            'C' => card("Bear", CardType::Creature, &[]),
            _ => card("Ritual", CardType::Sorcery, &[]),
        })
        .collect()
}

#[rstest]
#[case::directly_above("HC", true, 1, 1)]
#[case::a_sorcery_between("HSC", true, 1, 0)]
#[case::below_only("CH", true, 1, 0)]
#[case::on_the_bottom_with_none("H", true, 1, 0)]
#[case::three_above("HCSCC", false, 3, 1)]
#[case::two_above_one_below("CHCC", false, 3, 0)]
fn r287_cards_above_count_the_graveyard_order(
    #[values(2, 3, 4)] seats: usize,
    #[case] order: &str,
    #[case] directly: bool,
    #[case] at: u32,
    #[case] gained: i64,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .graveyard(0, graveyard(order, directly, at));
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    for _ in 0..400 {
        if driver.state().turn().number() != 3 {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let life = i64::from(driver.state().player(PlayerId(0)).unwrap().life());
    assert_eq!(life - 20, gained);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
