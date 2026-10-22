//! Goal: a mana ability with a mana cost (a Signet, a two-colour land, a Prism, a land with a big cost) pays for a spell only inside a payment, from mana the payment's other sources make or the pool holds, never from its own mana and never in a cycle; it is never offered to activate by itself, its cost is spent from the pool just before it taps, and whole bot games with these sources keep every invariant at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp, wastes};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, play_game_with, ready};
use rstest::rstest;

/// A card by name: a basic land of the testkit or a corpus card.
fn definition(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        "Wastes" => wastes(),
        other => card(other).unwrap(),
    }
}

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 holds the spell and has `mine` in play.
fn table(cost: &str, mine: &[&str]) -> Scenario {
    Scenario::new(2)
        .hand(0, [spell(cost)])
        .battlefield(0, mine.iter().map(|name| ready(definition(name))))
}

fn priority_of(driver: &Driver) -> &mtg_engine::decision::PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options,
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn floating(driver: &Driver) -> u32 {
    driver.state().player(pid(0)).unwrap().mana().total()
}

#[rstest]
#[case::a_land_and_a_signet_make_two_colours("{W}{U}", &["Forest", "Azorius Signet"], true, 0)]
#[case::a_signet_alone_has_nothing_to_pay_with("{W}{U}", &["Azorius Signet"], false, 0)]
#[case::the_second_mana_floats("{W}", &["Forest", "Azorius Signet"], true, 1)]
#[case::a_land_for_the_cost_and_one_for_the_spell("{W}{U}{G}", &["Forest", "Forest", "Azorius Signet"], true, 0)]
#[case::a_land_that_costs_mana_works_like_a_signet("{W}{U}", &["Forest", "Skycloud Expanse"], true, 0)]
#[case::two_signets_cannot_start_each_other("{W}{U}{B}{R}", &["Azorius Signet", "Azorius Signet"], false, 0)]
#[case::one_land_cannot_start_two_signets("{W}{U}{W}{U}", &["Forest", "Azorius Signet", "Azorius Signet"], false, 0)]
#[case::the_mana_of_a_signet_pays_for_a_prism("{W}{R}", &["Forest", "Azorius Signet", "Prismatic Lens"], true, 0)]
#[case::a_prism_makes_any_colour("{R}", &["Forest", "Prismatic Lens"], true, 0)]
#[case::the_free_ability_of_a_prism_pays_for_the_other("{R}", &["Prismatic Lens"], false, 0)]
#[case::five_lands_pay_for_a_quarry("{W}{U}{B}{R}{G}", &["Wastes", "Wastes", "Wastes", "Wastes", "Wastes", "Crystal Quarry"], true, 0)]
#[case::four_lands_do_not("{W}{U}{B}{R}{G}", &["Wastes", "Wastes", "Wastes", "Wastes", "Crystal Quarry"], false, 0)]
fn a_source_that_costs_mana_pays_only_from_other_mana(
    #[case] cost: &str,
    #[case] mine: &[&str],
    #[case] affordable: bool,
    #[case] spare: u32,
) {
    let mut driver = Driver::from_scenario(table(cost, mine)).unwrap();
    assert_eq!(!priority_of(&driver).castable.is_empty(), affordable);
    if affordable {
        driver.cast("Spell").unwrap();
        assert_eq!(floating(&driver), spare);
    }
}

#[rstest]
#[case::a_signet(&["Forest", "Azorius Signet"], 1)]
#[case::a_prism(&["Forest", "Prismatic Lens"], 2)]
#[case::a_quarry(&["Crystal Quarry"], 1)]
fn only_the_abilities_that_cost_nothing_are_offered_at_priority(
    #[case] mine: &[&str],
    #[case] offered: usize,
) {
    let driver = Driver::from_scenario(table("{W}", mine)).unwrap();
    assert_eq!(
        priority_of(&driver).mana_abilities.len(),
        offered,
        "{:?}",
        priority_of(&driver).mana_abilities
    );
    assert!(
        priority_of(&driver)
            .mana_abilities
            .iter()
            .all(|o| !o.is_costly())
    );
}

#[test]
fn the_cost_of_a_source_is_spent_from_the_pool_just_before_it_taps() {
    let mut driver = Driver::from_scenario(table("{W}{U}", &["Forest", "Azorius Signet"])).unwrap();
    let events = driver.cast("Spell").unwrap();
    let kinds: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentTapped { .. } => Some("tap".to_owned()),
            Event::ManaAdded { mana, .. } => Some(format!("add {mana:?}")),
            Event::ManaSpent { mana, .. } => Some(format!("spend {mana:?}")),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "tap",
            "add Green",
            "spend [Green]",
            "tap",
            "add White",
            "add Blue",
            "spend [White, Blue]"
        ]
    );
}

#[rstest]
#[case::a_land_and_the_signet(&["Forest", "Azorius Signet"], true)]
#[case::the_signet_alone(&["Azorius Signet"], false)]
#[case::the_land_alone(&["Forest"], false)]
fn a_payment_chosen_by_hand_is_checked_as_a_whole(#[case] taps: &[&str], #[case] accepted: bool) {
    let mut driver = Driver::from_scenario(table("{W}{U}", &["Forest", "Azorius Signet"])).unwrap();
    let result = driver.cast_tapping("Spell", taps);
    assert_eq!(result.is_ok(), accepted, "{result:?}");
}

#[test]
fn an_unfinished_payment_is_completed_with_the_land_the_signet_needs() {
    let mut driver = Driver::from_scenario(table("{W}{U}", &["Forest", "Azorius Signet"])).unwrap();
    driver.begin_cast("Spell").unwrap();
    let events = driver
        .answer(mtg_engine::decision::Answer::PayMana(
            mtg_engine::decision::PayChoice::Complete(mtg_engine::mana::Payment {
                taps: vec![driver.pay_options().unwrap().sources[1].source],
                ..mtg_engine::mana::Payment::default()
            }),
        ))
        .unwrap();
    let spent = events
        .iter()
        .filter(|e| matches!(e, Event::ManaSpent { .. }))
        .count();
    assert_eq!(spent, 2, "the cost of the Signet and the cost of the spell");
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::PermanentTapped { .. }))
            .count(),
        2
    );
    assert_eq!(floating(&driver), 0);
}

/// Sixty cards: lands of every kind of this task and basic lands, with creatures and spells to pay for.
fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 14),
        ("Island", 7),
        ("Swamp", 4),
        ("Plains", 5),
        ("Wastes", 3),
        ("Azorius Signet", 3),
        ("Skycloud Expanse", 2),
        ("Prismatic Lens", 2),
        ("Crystal Quarry", 2),
        ("Llanowar Elves", 3),
        ("Grizzly Bears", 4),
        ("Serra Angel", 2),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
        ("Counterspell", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name)))
    .collect()
}

/// Pays of a source's cost seen in a campaign: the mana is spent and the source taps right after.
fn paid_costs(events: &[Event]) -> u64 {
    events
        .windows(2)
        .filter(|pair| {
            matches!(
                (&pair[0], &pair[1]),
                (Event::ManaSpent { .. }, Event::PermanentTapped { .. })
            )
        })
        .count() as u64
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_sources_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(0u64);
    let games: u64 = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        *seen.lock().unwrap() += paid_costs(&outcome.events);
    });
    assert!(
        seen.into_inner().unwrap() > 0,
        "the games should pay the cost of a source from the pool"
    );
}

#[test]
fn a_seeded_game_with_these_sources_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(11), play(11));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}

#[test]
fn the_mana_a_source_makes_is_typed_in_the_events() {
    let mut driver = Driver::from_scenario(table("{R}", &["Forest", "Prismatic Lens"])).unwrap();
    let events = driver.cast("Spell").unwrap();
    let added: Vec<ManaType> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect();
    assert_eq!(added, [ManaType::Green, ManaType::Red]);
}
