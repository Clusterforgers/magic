//! Goal: the filled amounts (a branch on a condition, the mana spent or held, the most common value, unlocked doors
//! and a seeded random number) read the game as the rules say and draw the same number again from the same game.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::{ManaCost, ManaType};
use rstest::rstest;

use super::{ResolveError, Scope};
use crate::dk1c5_rooms::Doors;
use crate::fixtures::setup;
use crate::state::GameState;
use crate::zones::Zone;

fn colored(name: &str, cost: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse(cost).unwrap())
            .with_power_toughness(power, power),
    )
}

fn game() -> GameState {
    let battlefields = vec![
        vec![
            colored("Green", "{1}{G}", 2),
            colored("Gold", "{R}{G}", 4),
            colored("Green Too", "{G}", 2),
        ],
        vec![colored("Blue", "{U}", 1)],
    ];
    GameState::from_setup(&setup(vec![vec![], vec![]], battlefields)).unwrap()
}

fn read(game: &GameState, json: &str) -> Result<i64, ResolveError> {
    let source = game.objects.values().next().unwrap().reference();
    let amount: Amount = serde_json::from_str(json).unwrap();
    Scope::announcing(game, PlayerId(0), source).amount(&amount)
}

fn value(game: &GameState, json: &str) -> i64 {
    read(game, json).unwrap()
}

#[rstest]
#[case::most_common_color(r#"{"MostCommon":{"of":[{"Type":"Creature"}],"stat":"Colors"}}"#, 3)]
#[case::most_common_color_of_theirs(
    r#"{"MostCommon":{"of":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"stat":"Colors"}}"#,
    1
)]
#[case::most_common_power(r#"{"MostCommon":{"of":[{"Type":"Creature"}],"stat":"Power"}}"#, 2)]
#[case::most_common_of_none(r#"{"MostCommon":{"of":[{"Type":"Artifact"}],"stat":"Colors"}}"#, 0)]
#[case::no_rooms(r#"{"Doors":[{"Type":"Creature"}]}"#, 0)]
#[case::nothing_spent(r#"{"ManaSpent":{"of":null}}"#, 0)]
#[case::nothing_in_the_pool(r#"{"ManaInPool":"You"}"#, 0)]
#[case::no_colors_in_the_pool(r#"{"ManaColors":"You"}"#, 0)]
fn the_group_amounts_count_the_biggest_group_and_empty_reads_zero(
    #[case] json: &str,
    #[case] expected: i64,
) {
    assert_eq!(value(&game(), json), expected);
}

#[rstest]
#[case::holds_then(
    r#"{"Compare":{"left":{"Players":"EachPlayer"},"op":"Eq","right":2}}"#,
    7
)]
#[case::fails_otherwise(
    r#"{"Compare":{"left":{"Players":"EachPlayer"},"op":"Gt","right":2}}"#,
    1
)]
#[case::negated(
    r#"{"Not":{"Compare":{"left":{"Players":"EachPlayer"},"op":"Gt","right":2}}}"#,
    7
)]
fn a_when_amount_takes_the_branch_the_condition_picks(
    #[case] condition: &str,
    #[case] expected: i64,
) {
    let json = format!(r#"{{"When":{{"condition":{condition},"then":7,"otherwise":1}}}}"#);
    assert_eq!(value(&game(), &json), expected);
}

#[test]
fn a_when_amount_nests_and_reads_only_its_branch() {
    let json = r#"{"When":{"condition":{"Compare":{"left":{"Players":"EachPlayer"},"op":"Eq","right":2}},
        "then":{"When":{"condition":{"Not":{"Compare":{"left":1,"op":"Eq","right":1}}},"then":4,
            "otherwise":{"Times":{"by":3,"of":{"Players":"You"}}}}},
        "otherwise":"Triggering"}}"#;
    assert_eq!(value(&game(), json), 3);
}

#[rstest]
#[case::all(None, 5)]
#[case::red(Some(ManaType::Red), 2)]
#[case::green(Some(ManaType::Green), 1)]
#[case::colorless(Some(ManaType::Colorless), 2)]
#[case::blue(Some(ManaType::Blue), 0)]
fn mana_spent_counts_the_mana_of_a_type_or_all(
    #[case] of: Option<ManaType>,
    #[case] expected: i64,
) {
    let mut game = game();
    let source = game
        .objects
        .values()
        .find(|object| object.zone() == Zone::Battlefield)
        .unwrap()
        .reference();
    let spent = vec![
        ManaType::Red,
        ManaType::Red,
        ManaType::Green,
        ManaType::Colorless,
        ManaType::Colorless,
    ];
    game.objects
        .get_mut(&source.id)
        .and_then(|object| object.permanent.as_mut())
        .unwrap()
        .spent = spent;
    let amount = Amount::ManaSpent { of };
    let scope = Scope::announcing(&game, PlayerId(0), source);
    assert_eq!(scope.amount(&amount), Ok(expected));
}

#[rstest]
#[case::one_mana(vec![ManaType::Red], 0, "You", 1, 1)]
#[case::two_of_a_color(vec![ManaType::Red, ManaType::Red], 0, "You", 2, 1)]
#[case::colorless_is_no_color(vec![ManaType::Colorless, ManaType::Green], 0, "You", 2, 1)]
#[case::five_colors(
    vec![ManaType::White, ManaType::Blue, ManaType::Black, ManaType::Red, ManaType::Green],
    0, "You", 5, 5
)]
#[case::restricted_mana_counts(vec![ManaType::Red], 2, "You", 3, 2)]
#[case::the_other_pool_is_not_yours(vec![ManaType::Red], 0, "Opponents", 0, 0)]
#[case::each_pool(vec![ManaType::Red], 0, "EachPlayer", 1, 1)]
fn the_pool_amounts_count_the_mana_held(
    #[case] free: Vec<ManaType>,
    #[case] restricted: u32,
    #[case] who: &str,
    #[case] total: i64,
    #[case] colors: i64,
) {
    let mut game = game();
    for mana in free {
        game.players[0].mana.add(mana);
    }
    for _ in 0..restricted {
        game.players[0].mana.add_restricted(
            ManaType::Blue,
            vec![SpendOn::CostWithX],
            ManaLasts::Step,
        );
    }
    assert_eq!(value(&game, &format!(r#"{{"ManaInPool":"{who}"}}"#)), total);
    assert_eq!(
        value(&game, &format!(r#"{{"ManaColors":"{who}"}}"#)),
        colors
    );
}

#[rstest]
#[case::locked(0b00, 0)]
#[case::first(0b01, 1)]
#[case::second(0b10, 1)]
#[case::both(0b11, 2)]
fn doors_counts_the_unlocked_doors_of_rooms(#[case] unlocked: u8, #[case] expected: i64) {
    let mut game = game();
    let room = game
        .objects
        .values()
        .find(|object| object.zone() == Zone::Battlefield)
        .unwrap()
        .reference();
    game.objects
        .get_mut(&room.id)
        .and_then(|object| object.permanent.as_mut())
        .unwrap()
        .doors = Doors(0x80 | unlocked);
    assert_eq!(value(&game, r#"{"Doors":[{"Type":"Creature"}]}"#), expected);
}

#[rstest]
#[case::die(1, 6)]
#[case::coin(0, 1)]
#[case::negative(-3, 3)]
#[case::one_value(5, 5)]
fn a_random_amount_stays_in_its_range_and_reaches_every_value(#[case] min: i32, #[case] max: i32) {
    let mut seen = BTreeSet::new();
    let mut game = game();
    let json = format!(r#"{{"Random":{{"min":{min},"max":{max}}}}}"#);
    for seed in 0..400 {
        game.seed = seed;
        let drawn = value(&game, &json);
        assert!(
            (i64::from(min)..=i64::from(max)).contains(&drawn),
            "{drawn}"
        );
        seen.insert(drawn);
    }
    assert_eq!(seen.len(), usize::try_from(max - min + 1).unwrap());
}

#[test]
fn a_random_amount_is_the_same_for_the_same_game_and_moves_with_the_stream() {
    let json = r#"{"Random":{"min":0,"max":1000000}}"#;
    let mut game = game();
    let first = value(&game, json);
    assert_eq!(value(&game, json), first);
    game.rng_index += 1;
    assert_ne!(value(&game, json), first);
    game.rng_index -= 1;
    game.seed += 1;
    assert_ne!(value(&game, json), first);
}

#[test]
fn two_sources_draw_their_own_random_numbers() {
    let game = game();
    let amount: Amount = serde_json::from_str(r#"{"Random":{"min":0,"max":1000000}}"#).unwrap();
    let drawn: BTreeSet<i64> = game
        .objects
        .values()
        .map(|object| {
            let scope = Scope::announcing(&game, PlayerId(0), object.reference());
            scope.amount(&amount).unwrap()
        })
        .collect();
    assert_eq!(drawn.len(), game.objects.len());
}

#[test]
fn an_inverted_random_range_is_the_minimum() {
    assert_eq!(value(&game(), r#"{"Random":{"min":5,"max":2}}"#), 5);
}
