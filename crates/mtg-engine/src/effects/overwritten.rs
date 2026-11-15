//! Goal: an effect that sets a characteristic for ever replaces an identical one from the same source for the same objects
//! when no later effect of its layer came between them (CR 613.7), so a "becomes" used again and again stays one effect with
//! the result of all of them, while effects that add, stack, last a time, come from another source or have another effect
//! of their layer between them all stay; at two to four seats.
use mtg_core::card::{CardType, CardTypes, Subtype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::{EffectId, PlayerId};
use rstest::rstest;

use super::{ContinuousEffect, Duration, Modification, Selector};
use crate::characteristics::characteristics;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn game(seats: usize) -> GameState {
    let mut battlefields = vec![vec![creature("Wolf", 2, 2), creature("Source", 1, 1)]];
    battlefields.resize(seats, Vec::new());
    state(vec![Vec::new(); seats], battlefields)
}

fn named(game: &GameState, name: &str) -> ObjectRef {
    game.zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| game.object(*id))
        .find(|object| object.card().name == name)
        .map(GameObject::reference)
        .unwrap()
}

/// Folds the creation of the `n`th effect: on the Wolf from `source`, with an id and a timestamp that grow with `n`.
fn add_from(
    game: &mut GameState,
    source: &str,
    n: u64,
    modification: Modification,
    duration: Duration,
) {
    let effect = ContinuousEffect::new(
        EffectId(n),
        Selector::Objects(vec![named(game, "Wolf")]),
        modification,
        duration,
        n,
    )
    .from_source(named(game, source))
    .begun(ME, 1);
    apply_event(
        game,
        &Event::EffectCreated {
            effect: effect.into(),
        },
    )
    .unwrap();
}

/// Folds the creation of the `n`th effect on the object called `target`, from the Source.
fn add_on(
    game: &mut GameState,
    target: &str,
    n: u64,
    modification: Modification,
    duration: Duration,
) {
    let effect = ContinuousEffect::new(
        EffectId(n),
        Selector::Objects(vec![named(game, target)]),
        modification,
        duration,
        n,
    )
    .from_source(named(game, "Source"))
    .begun(ME, 1);
    apply_event(
        game,
        &Event::EffectCreated {
            effect: effect.into(),
        },
    )
    .unwrap();
}

/// [`add_from`] the Source.
fn add(game: &mut GameState, n: u64, modification: Modification, duration: Duration) {
    add_from(game, "Source", n, modification, duration);
}

fn types(list: &[CardType]) -> CardTypes {
    list.iter()
        .fold(CardTypes::default(), |all, ty| all.with(*ty))
}

fn enchantment() -> Modification {
    Modification::SetTypes(types(&[CardType::Enchantment]))
}

fn types_of_the_wolf(game: &GameState) -> CardTypes {
    characteristics(game, named(game, "Wolf").id).unwrap().types
}

#[rstest]
#[case::once(1)]
#[case::twice(2)]
#[case::a_hundred_times(100)]
#[case::two_hundred_times(200)]
fn a_set_used_again_and_again_stays_one_effect_with_the_result_of_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] uses: u64,
) {
    let mut game = game(seats);
    for n in 1..=uses {
        add(&mut game, n, enchantment(), Duration::Forever);
    }
    assert_eq!(game.effects().len(), 1);
    assert_eq!(game.effects()[0].timestamp, uses, "the newest stays");
    assert_eq!(types_of_the_wolf(&game), types(&[CardType::Enchantment]));
}

#[rstest]
#[case::power_and_toughness(Modification::SetPowerToughness { power: 2, toughness: 4 }, true)]
#[case::power(Modification::SetPower(3), true)]
#[case::card_types(enchantment(), true)]
#[case::creature_types(Modification::SetCreatureTypes(vec![Subtype("Soldier".into())]), true)]
#[case::colors(Modification::SetColors(ColorSet::default().with(Color::Red)), true)]
#[case::an_added_type(Modification::AddType(CardType::Artifact), false)]
#[case::a_stacking_boost(Modification::ModifyPowerToughness { power: 1, toughness: 1 }, false)]
fn only_a_set_is_overwritten_by_the_same_set(
    #[case] modification: Modification,
    #[case] compacted: bool,
) {
    let mut game = game(2);
    for n in 1..=3 {
        add(&mut game, n, modification.clone(), Duration::Forever);
    }
    assert_eq!(game.effects().len(), if compacted { 1 } else { 3 });
}

#[test]
fn a_boost_still_stacks_whatever_a_set_does_around_it() {
    let mut game = game(2);
    let wolf = named(&game, "Wolf").id;
    let boost = Modification::ModifyPowerToughness {
        power: 1,
        toughness: 1,
    };
    for n in 1..=3 {
        add(&mut game, n, boost.clone(), Duration::Forever);
    }
    assert_eq!(characteristics(&game, wolf).unwrap().power, Some(5));
}

#[rstest]
#[case::until_end_of_turn(Duration::UntilEndOfTurn)]
#[case::while_the_source_stays(Duration::WhileSourceOnBattlefield)]
fn an_effect_that_does_not_last_for_ever_is_not_compacted(#[case] duration: Duration) {
    let mut game = game(2);
    for n in 1..=3 {
        add(&mut game, n, enchantment(), duration.clone());
    }
    assert_eq!(game.effects().len(), 3);
}

#[test]
fn an_effect_of_another_source_stays() {
    let mut game = game(2);
    add(&mut game, 1, enchantment(), Duration::Forever);
    add_from(&mut game, "Wolf", 2, enchantment(), Duration::Forever);
    assert_eq!(game.effects().len(), 2);
}

#[test]
fn an_effect_of_the_layer_between_two_sets_keeps_the_older_one_and_the_result() {
    let mut game = game(2);
    add(&mut game, 1, enchantment(), Duration::Forever);
    add(
        &mut game,
        2,
        Modification::AddType(CardType::Artifact),
        Duration::Forever,
    );
    assert_eq!(
        types_of_the_wolf(&game),
        types(&[CardType::Enchantment, CardType::Artifact])
    );
    add(&mut game, 3, enchantment(), Duration::Forever);
    assert_eq!(game.effects().len(), 3, "the artifact came between them");
    assert_eq!(
        types_of_the_wolf(&game),
        types(&[CardType::Enchantment]),
        "the later set wins (CR 613.7)"
    );
    add(&mut game, 4, enchantment(), Duration::Forever);
    assert_eq!(game.effects().len(), 3, "the fourth replaces the third");
}

#[test]
fn an_effect_on_another_object_between_two_sets_does_not_keep_the_older_one() {
    let mut game = game(2);
    add(&mut game, 1, enchantment(), Duration::Forever);
    add_on(
        &mut game,
        "Source",
        2,
        Modification::AddType(CardType::Artifact),
        Duration::Forever,
    );
    add(&mut game, 3, enchantment(), Duration::Forever);
    assert_eq!(game.effects().len(), 2, "the Source's own effect stays");
    assert_eq!(types_of_the_wolf(&game), types(&[CardType::Enchantment]));
    let source = named(&game, "Source").id;
    assert!(
        characteristics(&game, source)
            .unwrap()
            .types
            .contains(CardType::Artifact)
    );
}
