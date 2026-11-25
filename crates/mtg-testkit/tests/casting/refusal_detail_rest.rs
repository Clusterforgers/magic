//! Goal: at tables of two to four seats the last refusals told by their reason alone name their detail: an ability whose cost falls short at its least X ("X can't be 0") names that X and its mana or other costs, a loyalty ability whose added mana is short names that mana, an ability paid only with mana of the chosen color names the color and the mana of it at hand, and a spell whose every payable X leaves no target names that target and the X it can pay.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::{TargetZone, Unplayable, Why};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, island, mountain, swamp, wastes};
use mtg_testkit::{Driver, ready};
use rstest::rstest;

use crate::r405_pieces::r405_throne;
use crate::refusal_detail::{card, lands, table, told_ability, told_card, x_target};

/// A 1/1 with the ability `json`.
fn keeper(json: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new("Life Keeper")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

fn least_x(least: u32, short: Option<(ManaCost, u32)>) -> Why {
    let (cost, at_hand) = short.unzip();
    Why::LeastX {
        least,
        cost,
        at_hand,
    }
}

/// The first activated ability of `name`, on seat 0's battlefield beside `mine`, with `counters` on it.
fn first_refusal(
    players: usize,
    name: &str,
    source: Arc<CardDefinition>,
    counters: Vec<(CounterKind, u32)>,
    mine: Vec<PresetPermanent>,
) -> Option<(Unplayable, Option<Why>)> {
    let mut mine = mine;
    mine.push(PresetPermanent {
        counters,
        ..ready(source)
    });
    let driver =
        Driver::from_scenario(table(players, Step::Main1, Vec::new(), mine, Vec::new())).unwrap();
    told_ability(&driver, driver.id_of(name).unwrap())
}

const MANA_X: &str = r#"{"Activated":{"cost":[{"Mana":"{X}"},{"XAtLeast":1},"Tap"],"effects":[{"GainLife":{"amount":"X"}}]}}"#;
const GREEN_X: &str = r#"{"Activated":{"cost":[{"Mana":"{X}{G}"},{"XAtLeast":1},"Tap"],"effects":[{"GainLife":{"amount":"X"}}]}}"#;
const TWO_X: &str = r#"{"Activated":{"cost":[{"Mana":"{X}"},{"XAtLeast":2}],"effects":[{"GainLife":{"amount":"X"}}]}}"#;

#[rstest]
#[case::mana_x_with_no_land(keeper(MANA_X), 1, Vec::new(), Unplayable::Mana, Some((cost(1, &[]), 0)))]
#[case::green_x_with_one_forest(keeper(GREEN_X), 1, lands(1, forest), Unplayable::Mana, Some((cost(1, &[Color::Green]), 1)))]
#[case::x_of_two_with_one_wastes(keeper(TWO_X), 2, lands(1, wastes), Unplayable::Mana, Some((cost(2, &[]), 1)))]
fn a_least_x_short_of_mana_names_its_mana(
    #[values(2, 3, 4)] players: usize,
    #[case] source: Arc<CardDefinition>,
    #[case] least: u32,
    #[case] mine: Vec<PresetPermanent>,
    #[case] reason: Unplayable,
    #[case] short: Option<(ManaCost, u32)>,
) {
    assert_eq!(
        first_refusal(players, "Life Keeper", source, Vec::new(), mine),
        Some((reason, Some(least_x(least, short))))
    );
}

#[rstest]
#[case::arcee_with_no_counters(
    "Arcee, Sharpshooter",
    "Arcee, Sharpshooter // Arcee, Acrobatic Coupe"
)]
#[case::rasputin_with_no_dream_counters("Rasputin, the Oneiromancer", "Rasputin, the Oneiromancer")]
fn a_least_x_short_of_counters_names_the_x(
    #[values(2, 3, 4)] players: usize,
    #[case] name: &str,
    #[case] printed: &str,
) {
    assert_eq!(
        first_refusal(players, name, card(printed), Vec::new(), lands(2, wastes)),
        Some((Unplayable::Cost, Some(least_x(1, None))))
    );
}

/// An enchantment that makes each planeswalker's loyalty abilities cost {2} more.
fn loyalty_tax() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"CostModifier":{"applies_to":[{"Type":"Planeswalker"}],"change":"Increase","mana":"{2}","activating":"Loyalty"}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Loyalty Tax")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

#[rstest]
#[case::tamiyo("Tamiyo, Compleated Sage")]
#[case::karn("Karn, Living Legacy")]
#[case::jace("Jace Beleren")]
#[case::garruk("Garruk Wildspeaker")]
fn a_loyalty_ability_short_of_its_added_mana_names_it(
    #[values(2, 3, 4)] players: usize,
    #[case] name: &str,
) {
    let mine = vec![
        ready(wastes()),
        PresetPermanent {
            counters: vec![(CounterKind::Loyalty, 3)],
            ..ready(card(name))
        },
    ];
    let scenario = table(players, Step::Main1, Vec::new(), mine, Vec::new())
        .battlefield(1, [ready(loyalty_tax())]);
    let driver = Driver::from_scenario(scenario).unwrap();
    let mana = Why::Mana {
        cost: cost(2, &[]),
        at_hand: 1,
        missing: Vec::new(),
        tax: 0,
    };
    assert_eq!(
        told_ability(&driver, driver.id_of(name).unwrap()),
        Some((Unplayable::Mana, Some(mana)))
    );
}

#[rstest]
fn an_ability_of_a_source_that_chose_no_color_says_so(
    #[values(2, 3, 4)] players: usize,
    #[values(mountain as fn() -> Arc<CardDefinition>, forest, island)] land: fn() -> Arc<
        CardDefinition,
    >,
) {
    let chosen = Why::ChosenColor {
        color: None,
        cost: None,
        at_hand: 0,
    };
    assert_eq!(
        first_refusal(
            players,
            "Throne of Eldraine",
            card("Throne of Eldraine"),
            Vec::new(),
            lands(3, land)
        ),
        Some((Unplayable::Mana, Some(chosen)))
    );
}

#[rstest]
#[case::one_mountain(&["Mountain", "Forest", "Forest"], 1)]
#[case::two_mountains(&["Mountain", "Mountain", "Forest"], 2)]
#[case::no_mountain(&["Forest", "Forest", "Forest"], 0)]
fn an_ability_paid_with_the_chosen_color_names_that_mana(
    #[values(2, 3, 4)] players: usize,
    #[case] lands: &[&str],
    #[case] red: u32,
) {
    let spell = card("Grizzly Bears");
    let driver = r405_throne(players, lands, spell).unwrap();
    let chosen = Why::ChosenColor {
        color: Some(Color::Red),
        cost: Some(cost(0, &[Color::Red, Color::Red, Color::Red])),
        at_hand: red,
    };
    assert_eq!(
        told_ability(&driver, driver.id_of("Throne of Eldraine").unwrap()),
        Some((Unplayable::Mana, Some(chosen)))
    );
}

#[rstest]
#[case::entrancing_melody(
    "Entrancing Melody",
    lands(4, island),
    "a creature with mana value X on the battlefield",
    2
)]
#[case::disembowel(
    "Disembowel",
    lands(3, swamp),
    "a creature with mana value X on the battlefield",
    2
)]
#[case::dominate(
    "Dominate",
    lands(4, island),
    "a creature with mana value X or less on the battlefield",
    1
)]
#[case::repeal(
    "Repeal",
    lands(3, island),
    "a nonland permanent with mana value X on the battlefield",
    2
)]
#[case::melody_with_three_islands(
    "Entrancing Melody",
    lands(3, island),
    "a creature with mana value X on the battlefield",
    1
)]
fn a_spell_whose_every_payable_x_has_no_target_names_it(
    #[values(2, 3, 4)] players: usize,
    #[case] name: &str,
    #[case] mine: Vec<PresetPermanent>,
    #[case] needs: &str,
    #[case] most: u32,
) {
    // Seat 1's Serra Angel has mana value 5, more than any X seat 0 can pay.
    let scenario = table(players, Step::Main1, vec![card(name)], mine, Vec::new())
        .battlefield(1, [ready(card("Serra Angel"))]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(
        told_card(&driver, name),
        Some((
            Unplayable::Targets,
            Some(x_target(needs, TargetZone::Battlefield, 0, most))
        ))
    );
}
