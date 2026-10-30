//! Goal: a keyword counter gives its permanent the keyword in layer 6 (CR 122.1b) in a game with no effect and no static ability: the keyword is there with one counter or several and held once beside the same printed keyword, gone when the last counter is removed, kept while another counter of the kind is left, taken away by an effect that removes the keyword or the abilities and applied before it, kept when the counter was put on after the effect, and a counter of any other kind gives none.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use rstest::rstest;

use super::tests::{effect, fold, named};
use super::*;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::keywords::keywords_of;
use crate::setup::PresetPermanent;

fn counter(name: &str) -> CounterKind {
    CounterKind::Other(name.to_owned())
}

/// Seat 0 controls a Bear with these counters and a Falcon that prints flying.
fn game_with(counters: &[(CounterKind, u32)]) -> GameState {
    let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
    let falcon = Arc::new(
        CardDefinition::new("Falcon")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Keyword(Keyword::Flying)),
    );
    setup.players[0].battlefield = vec![
        PresetPermanent {
            counters: counters.to_vec(),
            ..PresetPermanent::new(creature("Bear", 2, 2))
        },
        PresetPermanent::new(falcon),
    ];
    GameState::from_setup(&setup).unwrap()
}

fn change(game: &mut GameState, name: &str, kind: &str, delta: i32) {
    let on = Target::Object(named(game, name));
    fold(
        game,
        &[Event::CountersChanged(CountersChanged {
            on,
            counter: counter(kind),
            delta,
        })],
    );
}

fn has(game: &GameState, name: &str) -> Vec<Keyword> {
    keywords_of(game, named(game, name).id)
        .into_iter()
        .cloned()
        .collect()
}

#[rstest]
#[case::no_counter(&[], &[])]
#[case::one_counter(&[("lifelink", 1)], &[Keyword::Lifelink])]
#[case::three_counters_of_one_kind(&[("lifelink", 3)], &[Keyword::Lifelink])]
#[case::two_kinds(&[("deathtouch", 1), ("reach", 2)], &[Keyword::Deathtouch, Keyword::Reach])]
#[case::a_counter_of_another_kind(&[("shield", 1), ("stun", 1)], &[])]
fn the_keywords_of_a_permanent_are_those_of_its_keyword_counters(
    #[case] counters: &[(&str, u32)],
    #[case] expected: &[Keyword],
) {
    let counters: Vec<_> = counters
        .iter()
        .map(|(name, count)| (counter(name), *count))
        .collect();
    let game = game_with(&counters);
    assert!(game.effects.is_empty() && !game.statics_possible);
    let mut got = has(&game, "Bear");
    got.sort_by_key(|keyword| format!("{keyword:?}"));
    let mut want = expected.to_vec();
    want.sort_by_key(|keyword| format!("{keyword:?}"));
    assert_eq!(got, want);
}

#[test]
fn the_keyword_comes_and_goes_with_the_last_counter() {
    let mut game = game_with(&[]);
    assert_eq!(has(&game, "Bear"), Vec::<Keyword>::new());
    change(&mut game, "Bear", "lifelink", 1);
    assert_eq!(has(&game, "Bear"), [Keyword::Lifelink]);
    change(&mut game, "Bear", "lifelink", 1);
    change(&mut game, "Bear", "lifelink", -1);
    assert_eq!(has(&game, "Bear"), [Keyword::Lifelink], "one is left");
    change(&mut game, "Bear", "lifelink", -1);
    assert_eq!(has(&game, "Bear"), Vec::<Keyword>::new());
}

#[test]
fn a_counter_beside_the_same_printed_keyword_adds_it_once() {
    let mut game = game_with(&[]);
    change(&mut game, "Falcon", "flying", 2);
    assert_eq!(has(&game, "Falcon"), [Keyword::Flying]);
    change(&mut game, "Falcon", "flying", -2);
    assert_eq!(
        has(&game, "Falcon"),
        [Keyword::Flying],
        "the card still prints it"
    );
}

#[rstest]
#[case::the_keyword_removed(Modification::RemoveKeyword(Keyword::Lifelink))]
#[case::all_abilities_lost(Modification::LoseAbilities)]
fn an_effect_of_layer_six_applies_after_the_counters(#[case] modification: Modification) {
    let mut game = game_with(&[(counter("lifelink"), 1)]);
    effect(&mut game, "Bear", modification, 0);
    assert_eq!(has(&game, "Bear"), Vec::<Keyword>::new());
}

#[test]
fn an_effect_that_gives_another_keyword_adds_to_the_counters_keyword() {
    let mut game = game_with(&[(counter("lifelink"), 1)]);
    effect(
        &mut game,
        "Bear",
        Modification::AddKeyword(Keyword::Haste),
        5,
    );
    assert_eq!(has(&game, "Bear"), [Keyword::Lifelink, Keyword::Haste]);
}

#[rstest]
#[case::the_keyword_removed(Modification::RemoveKeyword(Keyword::Lifelink))]
#[case::all_abilities_lost(Modification::LoseAbilities)]
fn a_counter_put_on_after_the_effect_keeps_its_keyword(#[case] modification: Modification) {
    let mut game = game_with(&[]);
    let before = game.next_timestamp;
    effect(&mut game, "Bear", modification, before);
    game.next_timestamp = before + 1;
    change(&mut game, "Bear", "lifelink", 1);
    assert_eq!(has(&game, "Bear"), [Keyword::Lifelink]);
    assert_eq!(
        game.next_timestamp,
        before + 2,
        "the counter took a timestamp"
    );
}

#[test]
fn a_counter_put_on_again_after_the_last_one_left_is_newer_than_the_effect() {
    let mut game = game_with(&[(counter("lifelink"), 1)]);
    effect(&mut game, "Bear", Modification::LoseAbilities, 3);
    assert_eq!(has(&game, "Bear"), Vec::<Keyword>::new());
    change(&mut game, "Bear", "lifelink", -1);
    game.next_timestamp = 4;
    change(&mut game, "Bear", "lifelink", 1);
    assert_eq!(has(&game, "Bear"), [Keyword::Lifelink]);
}

#[test]
fn more_counters_of_a_kind_keep_the_timestamp_of_the_first() {
    let mut game = game_with(&[]);
    game.next_timestamp = 10;
    change(&mut game, "Bear", "lifelink", 1);
    effect(&mut game, "Bear", Modification::LoseAbilities, 11);
    change(&mut game, "Bear", "lifelink", 1);
    assert_eq!(
        has(&game, "Bear"),
        Vec::<Keyword>::new(),
        "a second counter of the kind does not renew the first"
    );
}
