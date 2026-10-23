//! Goal: a mana ability that removes X counters from its source and adds X mana in any combination of types is a source for each X up to the counters held and each combination of that many mana, numbered apart and no more than the sources one ability may have; using it removes exactly that many counters with the mana, and a payment takes one of them with its cost paid from the other sources; a combination counted by an amount is counted when it is offered.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::*;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::mana::{ManaPool, auto_payment, can_afford, check_payment, complete_payment};
use crate::mana_kinds::combinations_of;
use crate::spend::Spend;

const ME: PlayerId = PlayerId(0);

const STORAGE: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},{"RemoveCounters":{"counter":{"Other":"storage"},"count":"X"}}],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":"X","among":["Red","Green"]}}}}]}}"#;
const CRUCIBLE: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":{"Other":"storage"},"count":"X"}}],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":"X"}},"only":[{"Spell":{"subtypes":["Dragon"]}},{"Ability":{"subtypes":["Dragon"]}}]}}]}}"#;
const BY_CREATURES: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"among":["Red","Green"]}}}}]}}"#;

fn storage_counter() -> CounterKind {
    CounterKind::Other("storage".into())
}

/// A game with a land that has `json` among its rules and `counters` storage counters on it, and the `others` on the
/// battlefield beside it.
fn game(json: &str, counters: i32, others: Vec<Arc<CardDefinition>>) -> GameState {
    let land = CardDefinition::new("Reaches")
        .with_type(CardType::Land)
        .with_rules(serde_json::from_str(json).unwrap());
    let mut permanents = vec![Arc::new(land)];
    permanents.extend(others);
    let mut game = state(vec![vec![], vec![]], vec![permanents, vec![]]);
    let id = game.zone(Zone::Battlefield)[0];
    if counters > 0 {
        let on = Target::Object(game.object(id).unwrap().reference());
        let delta = counters;
        let changed = CountersChanged {
            on,
            counter: storage_counter(),
            delta,
        };
        apply_event(&mut game, &Event::CountersChanged(changed)).unwrap();
    }
    game
}

fn forest() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(Color::Green)))
}

/// The options of the land named Reaches.
fn reaches(game: &GameState) -> Vec<ManaOption> {
    let id = game.zone(Zone::Battlefield)[0];
    options(game, ME)
        .into_iter()
        .filter(|option| option.source.object == id)
        .collect()
}

#[rstest]
#[case::none_held(0, 0)]
#[case::one(1, 2)]
#[case::two(2, 5)]
#[case::three(3, 9)]
#[case::five(5, 20)]
#[case::seven(7, 35)]
fn the_ways_of_two_types_are_those_of_each_x_up_to_the_counters(
    #[case] counters: i32,
    #[case] ways: usize,
) {
    let found = reaches(&game(STORAGE, counters, Vec::new()));
    assert_eq!(found.len(), ways);
    let mut numbers: Vec<usize> = found.iter().map(|o| o.source.ability).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), ways, "every way is a source of its own");
    for x in 1..=counters as usize {
        let of_x = found.iter().filter(|o| o.made().len() == x).count();
        assert_eq!(of_x, x + 1, "x = {x}");
    }
    assert!(found.iter().all(|o| o.is_costly() && o.tapless && o.extra));
}

#[rstest]
#[case::two_counters(2, 20)]
#[case::four_counters(4, 125)]
#[case::five_counters_fill_what_is_left(5, 128)]
#[case::many_counters_stop_at_the_most(40, 128)]
fn the_ways_of_five_types_stop_at_the_sources_an_ability_may_have(
    #[case] counters: i32,
    #[case] ways: usize,
) {
    let found = reaches(&game(CRUCIBLE, counters, Vec::new()));
    assert_eq!(found.len(), ways);
    let mut numbers: Vec<usize> = found.iter().map(|o| o.source.ability).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), ways);
    // Every X up to the last full one is offered whole, and each of its ways once.
    let mut seen = found.iter().map(ManaOption::made).collect::<Vec<_>>();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), ways);
}

#[rstest]
#[case::room_for_all(4, 100, 70)]
#[case::exactly_the_room(2, 15, 15)]
#[case::too_many_ways_keep_the_one_type_picks_and_the_next_to_them(5, 100, 25)]
#[case::cut_to_the_limit(5, 10, 10)]
#[case::no_room(3, 0, 0)]
fn a_count_with_more_ways_than_are_left_offers_the_one_type_picks_first(
    #[case] count: u32,
    #[case] limit: usize,
    #[case] expected: usize,
) {
    let found = combinations_of(&[], [count], limit);
    assert_eq!(found.len(), expected);
    if limit >= 5 {
        for kind in COLORS {
            assert!(found.contains(&vec![kind; count as usize]), "{kind:?}");
        }
    }
    assert!(found.iter().all(|pick| pick.len() == count as usize));
    let mut distinct = found.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), found.len(), "each pick once");
}

#[test]
fn a_tapped_land_still_removes_counters_for_mana() {
    let mut game = game(STORAGE, 2, vec![forest()]);
    let id = game.zone(Zone::Battlefield)[0];
    apply_event(&mut game, &Event::PermanentTapped { object: id }).unwrap();
    assert_eq!(reaches(&game).len(), 5);
}

#[rstest]
#[case::one(1, vec![ManaType::Green])]
#[case::two(2, vec![ManaType::Red, ManaType::Green])]
#[case::three(3, vec![ManaType::Red, ManaType::Red, ManaType::Green])]
fn using_it_removes_the_x_counters_and_adds_x_mana_without_a_tap(
    #[case] x: usize,
    #[case] made: Vec<ManaType>,
) {
    let game = game(STORAGE, 3, Vec::new());
    let option = reaches(&game)
        .into_iter()
        .find(|o| o.made() == made)
        .unwrap();
    assert_eq!(option.made().len(), x);
    let events = activation(&game, ME, &option, &[]).unwrap();
    let removed: Vec<i32> = events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) => Some(changed.delta),
            _ => None,
        })
        .collect();
    assert_eq!(removed, [-i32::try_from(x).unwrap()]);
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, Event::PermanentTapped { .. }))
    );
    let added: Vec<ManaType> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect();
    assert_eq!(added, made);
}

#[test]
fn a_land_that_taps_for_it_taps_and_keeps_the_restriction_on_its_mana() {
    let game = game(CRUCIBLE, 2, Vec::new());
    let option = reaches(&game)
        .into_iter()
        .find(|o| o.made() == [ManaType::Red, ManaType::Green])
        .unwrap();
    assert!(!option.tapless);
    assert_eq!(option.only.len(), 2);
    let events = activation(&game, ME, &option, &[]).unwrap();
    assert!(matches!(events[0], Event::CountersChanged(_)));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::PermanentTapped { .. }))
    );
    assert!(events.iter().all(|event| match event {
        Event::ManaAdded { only, .. } => only.len() == 2,
        _ => true,
    }));
}

#[rstest]
#[case::two_counters_and_a_forest_pay_red_and_green("{R}{G}", 2, true)]
#[case::the_cost_of_the_ability_is_paid_by_the_forest("{R}{R}", 2, true)]
#[case::three_counters_make_three("{R}{G}{G}", 3, true)]
#[case::more_than_the_counters_make("{R}{G}{G}", 2, false)]
#[case::a_color_the_ability_never_makes("{B}", 2, false)]
#[case::no_counters_no_mana("{R}", 0, false)]
fn a_payment_takes_the_ability_with_its_cost_paid_by_another_source(
    #[case] cost: &str,
    #[case] counters: i32,
    #[case] affordable: bool,
) {
    let game = game(STORAGE, counters, vec![forest()]);
    let sources = options(&game, ME);
    let cost = ManaCost::parse(cost).unwrap();
    let pool = ManaPool::default();
    assert_eq!(
        can_afford(&cost, &pool, &sources, 20, &Spend::other()),
        affordable
    );
    let payment = auto_payment(&cost, &pool, &sources, 20, &Spend::other());
    assert_eq!(payment.is_some(), affordable, "{payment:?}");
    if let Some(payment) = payment {
        assert!(check_payment(&cost, &pool, &sources, &[], 20, &Spend::other(), &payment).is_ok());
        let storage = game.zone(Zone::Battlefield)[0];
        let used: Vec<_> = payment
            .taps
            .iter()
            .filter(|tap| tap.object == storage)
            .collect();
        assert_eq!(used.len(), 1, "one of its ways: {payment:?}");
        // The sources a payment names are the ones that complete it.
        let done = complete_payment(
            &cost,
            &pool,
            &sources,
            &[],
            20,
            &Spend::other(),
            &payment,
        );
        assert_eq!(done.as_ref(), Ok(&payment));
    }
}

#[rstest]
#[case::no_creatures(0, 0)]
#[case::one(1, 2)]
#[case::three(3, 4)]
#[case::six(6, 7)]
fn a_combination_counted_by_an_amount_is_counted_when_it_is_offered(
    #[case] creatures: usize,
    #[case] ways: usize,
) {
    let bear = Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    );
    let mut others = vec![bear; creatures];
    others.insert(0, forest());
    let game = game(BY_CREATURES, 0, others);
    let found = reaches(&game);
    assert_eq!(found.len(), ways);
    assert!(found.iter().all(|o| o.made().len() == creatures));
}
