//! Goal: a mana ability that adds mana in any combination of types is one source for each combination (a multiset, in the order of the types), numbered apart, that the payment planner chooses among and the activation adds in one batch, up to the five mana of five types a card prints.
use crate::spend::Spend;
use mtg_cards::CardDefinition;
use rstest::rstest;

use super::*;
use crate::fixtures::state;
use crate::mana::{ManaPool, auto_payment, can_afford, check_payment};
use crate::mana_kinds::combinations;
use mtg_core::mana::ManaCost;

const ME: PlayerId = PlayerId(0);

fn artifact(json: &str) -> GameState {
    let card = CardDefinition::new("Cache")
        .with_type(CardType::Artifact)
        .with_rules(serde_json::from_str(json).unwrap());
    state(
        vec![vec![], vec![]],
        vec![vec![std::sync::Arc::new(card)], vec![]],
    )
}

fn cache(count: u32) -> GameState {
    artifact(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"AnyCombination":{{"count":{count}}}}}}}}}]}}}}"#
    ))
}

#[rstest]
#[case::one_of_five(&[], 1, 5)]
#[case::two_of_five(&[], 2, 15)]
#[case::three_of_five(&[], 3, 35)]
#[case::four_of_five(&[], 4, 70)]
#[case::five_of_five(&[], 5, 126)]
#[case::two_of_two(&[ManaType::Red, ManaType::Green], 2, 3)]
#[case::four_of_two(&[ManaType::Red, ManaType::Green], 4, 5)]
#[case::three_of_three(&[ManaType::Blue, ManaType::Black, ManaType::Red], 3, 10)]
fn the_combinations_of_a_pick_are_the_multisets_of_its_types(
    #[case] among: &[ManaType],
    #[case] count: u32,
    #[case] expected: usize,
) {
    let found = combinations(among, count);
    assert_eq!(found.len(), expected);
    let types = if among.is_empty() { 5 } else { among.len() };
    assert_eq!(mtg_cards::assemble::combinations(types, count), expected);
    let order = |mana: &ManaType| ManaType::ALL.iter().position(|t| t == mana);
    for pick in &found {
        assert_eq!(pick.len(), count as usize);
        assert!(
            pick.windows(2).all(
                |pair| among.is_empty() && order(&pair[0]) <= order(&pair[1])
                    || among.iter().position(|t| *t == pair[0])
                        <= among.iter().position(|t| *t == pair[1])
            ),
            "in the order of the types: {pick:?}"
        );
    }
    let mut distinct = found.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), found.len(), "each combination once");
}

#[rstest]
#[case::two(2, 15)]
#[case::three(3, 35)]
#[case::five(5, 126)]
fn a_combination_is_one_source_for_each_way_with_a_number_of_its_own(
    #[case] count: u32,
    #[case] ways: usize,
) {
    let game = cache(count);
    let found = options(&game, ME);
    assert_eq!(found.len(), ways);
    let mut numbers: Vec<usize> = found.iter().map(|o| o.source.ability).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), ways, "every way is a source of its own");
    assert!(found.iter().all(|o| o.made().len() == count as usize));
}

#[rstest]
#[case::each_color_once("{W}{U}{B}", true)]
#[case::one_color_three_times("{R}{R}{R}", true)]
#[case::generic_and_color("{2}{G}", true)]
#[case::too_much("{W}{U}{B}{R}", false)]
#[case::three_generic("{3}", true)]
fn the_planner_picks_the_combination_that_pays_the_cost_with_one_tap(
    #[case] cost: &str,
    #[case] affordable: bool,
) {
    let game = cache(3);
    let sources = options(&game, ME);
    let cost = ManaCost::parse(cost).unwrap();
    let pool = ManaPool::default();
    assert_eq!(
        can_afford(&cost, &pool, &sources, 20, &Spend::other()),
        affordable
    );
    let payment = auto_payment(&cost, &pool, &sources, 20, &Spend::other());
    assert_eq!(payment.is_some(), affordable);
    if let Some(payment) = payment {
        assert_eq!(payment.taps.len(), 1, "one tap pays: {payment:?}");
        assert!(check_payment(&cost, &pool, &sources, &[], 20, &Spend::other(), &payment).is_ok());
    }
}

#[test]
fn five_mana_of_five_colors_come_from_one_tap() {
    let game = cache(5);
    let sources = options(&game, ME);
    let cost = ManaCost::parse("{W}{U}{B}{R}{G}").unwrap();
    let payment = auto_payment(&cost, &ManaPool::default(), &sources, 20, &Spend::other()).unwrap();
    let tapped = sources
        .iter()
        .find(|o| o.source == payment.taps[0])
        .unwrap();
    assert_eq!(tapped.made(), COLORS);
}

#[test]
fn using_a_combination_adds_each_of_its_mana_with_the_tap() {
    let game = cache(2);
    let option = options(&game, ME)
        .into_iter()
        .find(|o| o.made() == [ManaType::White, ManaType::Blue])
        .unwrap();
    let id = option.source.object;
    let events = activation(&game, ME, &option, &[]).unwrap();
    let added: Vec<ManaType> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect();
    assert_eq!(events[0], Event::PermanentTapped { object: id });
    assert_eq!(added, [ManaType::White, ManaType::Blue]);
}

#[rstest]
#[case::more_than_fit(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":6}}}}]}}"#)]
#[case::added_twice(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2}},"times":2}}]}}"#)]
fn a_combination_with_too_many_ways_is_no_mana_ability_the_engine_runs(#[case] json: &str) {
    let found = options(&artifact(json), ME);
    assert_eq!(found.len(), 0, "{found:?}");
}
