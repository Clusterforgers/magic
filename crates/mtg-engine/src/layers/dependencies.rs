//! Goal: within a layer an effect that depends on another (applying the other first changes whether it exists, what it applies to or what it does) applies after it whatever the timestamps say, one that does not goes by timestamp alone, effects that depend on each other in a loop go by timestamp, and what the effects read of the other objects is what the earlier layers and earlier effects left; the result does not depend on how the effects are stored.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::EffectId;
use rstest::rstest;

use super::tests::{bear, fold, named, pump, rules};
use super::*;
use crate::characteristics::characteristics;
use crate::effects::{ContinuousEffect, Duration, Selector};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::keywords::keywords_of;

const ME: PlayerId = PlayerId(0);

fn card(name: &str, kind: CardType, ability: Option<&str>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(kind);
    if let Some(ability) = ability {
        card = card.with_rules(rules(ability));
    }
    Arc::new(card)
}

fn static_over(affected: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":{affected},"mods":{mods}}}}}"#)
}

/// An enchantment with one static ability over the objects `affected` names.
fn enchantment(name: &str, affected: &str, mods: &str) -> Arc<CardDefinition> {
    card(
        name,
        CardType::Enchantment,
        Some(&static_over(affected, mods)),
    )
}

/// A creature with one static ability over the objects `affected` names.
fn lord(name: &str, power: i32, affected: &str, mods: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, power)
            .with_rules(rules(&static_over(affected, mods))),
    )
}

/// A game with `board` on the battlefield of seat 0 and `waiting` in their hand; the cards named in `order` enter in
/// that order, each taking the next timestamp, so the first to enter is the oldest.
fn game(
    board: Vec<Arc<CardDefinition>>,
    waiting: Vec<Arc<CardDefinition>>,
    order: &[&str],
) -> GameState {
    let mut game = state(vec![waiting, vec![]], vec![board, vec![]]);
    for name in order {
        let object = named(&game, name).id;
        fold(
            &mut game,
            &[Event::ZoneChanged {
                object,
                from: Zone::Hand(ME),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            }],
        );
    }
    game
}

fn has_subtype(game: &GameState, name: &str, subtype: &str) -> bool {
    let chars = characteristics(game, named(game, name).id).unwrap();
    chars.subtypes.contains(&Subtype(subtype.into()))
}

fn is_artifact(game: &GameState, name: &str) -> bool {
    characteristics(game, named(game, name).id)
        .unwrap()
        .types
        .contains(CardType::Artifact)
}

fn is_creature(game: &GameState, name: &str) -> bool {
    characteristics(game, named(game, name).id)
        .unwrap()
        .types
        .contains(CardType::Creature)
}

/// "Creatures are Elves" and "Elves are artifacts", both in layer 4.
fn veil_and_smith(reads: &str) -> Vec<Arc<CardDefinition>> {
    vec![
        enchantment(
            "Veil",
            r#"{"Each":[{"Type":"Creature"}]}"#,
            r#"[{"AddSubtype":"Elf"}]"#,
        ),
        enchantment(
            "Smith",
            &format!(r#"{{"Each":[{{"Subtype":"{reads}"}}]}}"#),
            r#"[{"AddType":"Artifact"}]"#,
        ),
    ]
}

#[rstest]
#[case::the_type_comes_first_and_is_older(&["Veil", "Smith"])]
#[case::the_type_comes_first_though_it_is_newer(&["Smith", "Veil"])]
fn an_effect_that_selects_by_what_another_gives_waits_for_it_whatever_their_ages(
    #[case] order: &[&str],
) {
    // The second applies to what the first made, so it is the second that waits.
    let game = game(vec![creature("Bear", 2, 2)], veil_and_smith("Elf"), order);
    assert!(has_subtype(&game, "Bear", "Elf"));
    assert!(is_artifact(&game, "Bear"));
}

#[rstest]
#[case(&["Veil", "Smith"])]
#[case(&["Smith", "Veil"])]
fn an_effect_that_does_not_read_what_another_writes_goes_by_timestamp_alone(
    #[case] order: &[&str],
) {
    // "Goblins are artifacts" reads subtypes too, but the Bear is no Goblin before or after the Elf is added.
    let game = game(
        vec![creature("Bear", 2, 2)],
        veil_and_smith("Goblin"),
        order,
    );
    assert!(has_subtype(&game, "Bear", "Elf"));
    assert!(!is_artifact(&game, "Bear"));
}

#[rstest]
#[case::the_pump_is_older(&["All", "Big"])]
#[case::the_pump_is_newer(&["Big", "All"])]
fn an_effect_that_applies_to_what_another_changes_waits_for_it(#[case] order: &[&str]) {
    // "Creatures with power 3 or more get +1/+1" and "creatures get +1/+0", the Ogre 3/3 and the Wolf 2/2: the Wolf has
    // power 3 only once "All" has applied, so "Big" must find it there.
    let big = enchantment(
        "Big",
        r#"{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]}"#,
        r#"[{"ModifyPT":{"power":1,"toughness":1}}]"#,
    );
    let all = enchantment(
        "All",
        r#"{"Each":[{"Type":"Creature"}]}"#,
        r#"[{"ModifyPT":{"power":1,"toughness":0}}]"#,
    );
    let game = game(
        vec![creature("Ogre", 3, 3), creature("Wolf", 2, 2)],
        vec![big, all],
        order,
    );
    assert_eq!(pump(&game, "Ogre"), Some((5, 4)));
    assert_eq!(pump(&game, "Wolf"), Some((4, 3)));
}

#[rstest]
#[case(&["All", "Counter"])]
#[case(&["Counter", "All"])]
fn an_amount_that_counts_what_another_changes_waits_for_it(#[case] order: &[&str]) {
    // The Ogre gets +X/+0, X the creatures with power 3 or more: "All" decides who they are.
    let count = r#"{"Count":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]}"#;
    let counter = enchantment(
        "Counter",
        r#"{"Each":[{"Named":"Ogre"}]}"#,
        &format!(r#"[{{"ModifyPT":{{"power":{count},"toughness":0}}}}]"#),
    );
    let all = enchantment(
        "All",
        r#"{"Each":[{"Type":"Creature"}]}"#,
        r#"[{"ModifyPT":{"power":1,"toughness":0}}]"#,
    );
    let game = game(
        vec![creature("Ogre", 3, 3), creature("Wolf", 2, 2)],
        vec![counter, all],
        order,
    );
    // After "All" both are 3 or more: the Ogre is 4, plus the two it counts.
    assert_eq!(pump(&game, "Ogre"), Some((6, 3)));
    assert_eq!(pump(&game, "Wolf"), Some((3, 2)));
}

#[rstest]
#[case(&["Set", "Hold"], (3, 3))]
#[case(&["Hold", "Set"], (5, 5))]
fn an_effect_the_other_changes_nothing_for_goes_by_timestamp_though_it_could_have(
    #[case] order: &[&str],
    #[case] ogre: (i32, i32),
) {
    // "Creatures with power 3 or more are 5/5" reads power and "Ogres are 3/3" writes it, but the Ogre has power 3
    // before and after, so neither depends on the other: the newer of the two wins.
    let big = enchantment(
        "Set",
        r#"{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]}"#,
        r#"[{"SetPT":{"power":5,"toughness":5}}]"#,
    );
    let hold = enchantment(
        "Hold",
        r#"{"Each":[{"Named":"Ogre"}]}"#,
        r#"[{"SetPT":{"power":3,"toughness":3}}]"#,
    );
    let game = game(vec![creature("Ogre", 3, 3)], vec![big, hold], order);
    assert_eq!(pump(&game, "Ogre"), Some(ogre));
}

/// An artifact that is no Elf and an artifact creature that is one, with, in hand, the two effects of a loop ("artifacts
/// are Elves" and "Elves are not artifacts") and an effect that waits for the loop ("Elves are creatures").
fn loop_game(order: &[&str]) -> GameState {
    let elven = enchantment(
        "Elven",
        r#"{"Each":[{"Type":"Artifact"}]}"#,
        r#"[{"AddSubtype":"Elf"}]"#,
    );
    let rust = enchantment(
        "Rust",
        r#"{"Each":[{"Subtype":"Elf"}]}"#,
        r#"[{"RemoveType":"Artifact"}]"#,
    );
    let tin = enchantment(
        "Tin",
        r#"{"Each":[{"Subtype":"Elf"}]}"#,
        r#"[{"AddType":"Creature"}]"#,
    );
    let gem = card("Gem", CardType::Artifact, None);
    let golem = Arc::new(
        CardDefinition::new("Golem")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_power_toughness(1, 1),
    );
    game(vec![gem, golem], vec![elven, rust, tin], order)
}

#[rstest]
#[case::elves_first(&["Elven", "Rust"], false)]
#[case::rust_first(&["Rust", "Elven"], true)]
fn effects_that_depend_on_each_other_in_a_loop_go_by_timestamp(
    #[case] order: &[&str],
    #[case] gem_stays_an_artifact: bool,
) {
    // Each changes who the other applies to (the Golem stops being an artifact; the Gem becomes an Elf), so they depend
    // on each other (CR 613.8b) and the older goes first.
    let game = loop_game(order);
    assert!(has_subtype(&game, "Gem", "Elf"));
    assert_eq!(is_artifact(&game, "Gem"), gem_stays_an_artifact);
    assert!(!is_artifact(&game, "Golem"));
}

#[rstest]
#[case(&["Tin", "Elven", "Rust"])]
#[case(&["Elven", "Tin", "Rust"])]
#[case(&["Elven", "Rust", "Tin"])]
#[case(&["Rust", "Elven", "Tin"])]
#[case(&["Tin", "Rust", "Elven"])]
fn an_effect_that_depends_on_a_loop_waits_for_the_loop_and_not_for_its_age(#[case] order: &[&str]) {
    // "Elves are creatures" reads who is an Elf, which "artifacts are Elves" decides, so it comes after it, wherever
    // the loop's two effects stand in time.
    let game = loop_game(order);
    assert!(is_creature(&game, "Gem"), "{order:?}");
    assert!(is_creature(&game, "Golem"));
}

#[test]
fn a_registered_effect_waits_for_the_registered_effect_it_depends_on() {
    // "Every creature is an Elf" (older) reads who is a creature; "the Gem is a creature" (newer) decides that.
    let mut game = state(
        vec![vec![], vec![]],
        vec![
            vec![
                card("Gem", CardType::Artifact, None),
                creature("Bear", 2, 2),
            ],
            vec![],
        ],
    );
    let gem = named(&game, "Gem");
    game.effects.push(ContinuousEffect::new(
        EffectId(0),
        Selector::AllCreatures,
        Modification::AddSubtype(Subtype("Elf".into())),
        Duration::Forever,
        1,
    ));
    game.effects.push(ContinuousEffect::new(
        EffectId(1),
        Selector::objects(&game, [gem.id]),
        Modification::AddType(CardType::Creature),
        Duration::Forever,
        2,
    ));
    game.layers.reset();
    assert!(has_subtype(&game, "Gem", "Elf"));
    assert!(has_subtype(&game, "Bear", "Elf"));
}

#[rstest]
#[case(&["Wings", "Winds"])]
#[case(&["Winds", "Wings"])]
fn what_a_static_reads_of_other_objects_has_the_effects_of_the_layers_before_it(
    #[case] order: &[&str],
) {
    // "Creatures have flying" (layer 6) and "creatures with flying get +1/+1" (layer 7c): the second finds the flying the
    // first gives, whichever entered first.
    let wings = enchantment(
        "Wings",
        r#"{"Each":[{"Type":"Creature"}]}"#,
        r#"[{"AddKeyword":"Flying"}]"#,
    );
    let winds = enchantment(
        "Winds",
        r#"{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"}]}"#,
        r#"[{"ModifyPT":{"power":1,"toughness":1}}]"#,
    );
    let game = game(vec![creature("Mouse", 1, 1)], vec![wings, winds], order);
    assert_eq!(pump(&game, "Mouse"), Some((2, 2)));
    assert_eq!(keywords_of(&game, named(&game, "Mouse").id).len(), 1);
}

const LORD: &str = r#"[{"AddKeyword":"Haste"},{"ModifyPT":{"power":1,"toughness":1}}]"#;
const OTHER_CREATURES: &str = r#"{"Each":[{"Type":"Creature"},"Other"]}"#;

#[rstest]
#[case(&["Lord", "Humility"])]
#[case(&["Humility", "Lord"])]
fn an_effect_exists_only_while_its_source_has_the_ability_so_it_waits_for_the_effect_that_takes_it_away(
    #[case] order: &[&str],
) {
    // A lord that gives haste and a pump, and a Humility: the lord's effect in layer 6 depends on the one that makes it
    // lose its abilities, so it does not exist whichever is older, and the pump of layer 7c never begins.
    let chief = lord("Lord", 2, OTHER_CREATURES, LORD);
    let humility = enchantment(
        "Humility",
        r#"{"Each":[{"Type":"Creature"}]}"#,
        r#"["LoseAbilities",{"SetPT":{"power":1,"toughness":1}}]"#,
    );
    let game = game(vec![creature("Mouse", 1, 1)], vec![chief, humility], order);
    let mouse = named(&game, "Mouse").id;
    assert!(keywords_of(&game, mouse).is_empty(), "no haste");
    assert_eq!(pump(&game, "Mouse"), Some((1, 1)), "no pump either");
}

#[test]
fn an_effect_whose_source_keeps_its_ability_applies() {
    let chief = lord("Lord", 2, OTHER_CREATURES, LORD);
    let game = game(vec![chief, creature("Mouse", 1, 1)], vec![], &[]);
    let mouse = named(&game, "Mouse").id;
    assert_eq!(keywords_of(&game, mouse).len(), 1);
    assert_eq!(pump(&game, "Mouse"), Some((2, 2)));
}

mod properties {
    //! Goal: however the effects of a state are stored, the characteristics, the controller and the keywords the layers make are the same, and a clone of the state agrees.
    use proptest::collection::vec;
    use proptest::prelude::{prop_assert_eq, proptest};

    use super::*;
    use crate::characteristics::controller_of;

    fn effect_for(which: u8, stamp: u64, target: ObjectRef) -> ContinuousEffect {
        let (selector, modification) = match which % 8 {
            0 => (
                Selector::AllCreatures,
                Modification::AddSubtype(Subtype("Elf".into())),
            ),
            1 => (
                Selector::Objects(vec![target]),
                Modification::AddType(CardType::Artifact),
            ),
            2 => (
                Selector::Objects(vec![target]),
                Modification::AddType(CardType::Creature),
            ),
            3 => (
                Selector::AllCreatures,
                Modification::AddKeyword(mtg_cards::keyword::Keyword::Flying),
            ),
            4 => (
                Selector::CreaturesControlledBy(ME),
                Modification::ModifyPowerToughness {
                    power: 1,
                    toughness: 1,
                },
            ),
            5 => (
                Selector::AllCreatures,
                Modification::SetPowerToughness {
                    power: 3,
                    toughness: 3,
                },
            ),
            6 => (Selector::Objects(vec![target]), Modification::LoseAbilities),
            _ => (
                Selector::AllCreatures,
                Modification::RemoveType(CardType::Artifact),
            ),
        };
        ContinuousEffect::new(
            EffectId(stamp),
            selector,
            modification,
            Duration::Forever,
            stamp,
        )
    }

    fn view(game: &GameState) -> Vec<(Option<i32>, Option<i32>, bool, usize, PlayerId)> {
        ["Gem", "Bear"]
            .iter()
            .map(|name| {
                let id = named(game, name).id;
                let chars = characteristics(game, id).unwrap();
                (
                    chars.power,
                    chars.toughness,
                    chars.types.contains(CardType::Artifact),
                    keywords_of(game, id).len() + chars.subtypes.len(),
                    controller_of(game, id).unwrap(),
                )
            })
            .collect()
    }

    fn board() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![card("Gem", CardType::Artifact, None), bear()], vec![]],
        )
    }

    proptest! {
        #[test]
        fn the_order_the_effects_are_stored_in_changes_nothing(
            which in vec(0u8..8, 1..7),
            turn in 0usize..1000,
        ) {
            let mut stored = board();
            let gem = named(&stored, "Gem");
            let effects: Vec<ContinuousEffect> = which
                .iter()
                .enumerate()
                .map(|(n, w)| effect_for(*w, n as u64 + 1, gem))
                .collect();
            stored.effects = effects.clone();
            stored.layers.reset();
            let mut shuffled = board();
            let mut order = effects;
            // A rotation and a reversal: enough orders to tell storage from timestamps.
            let len = order.len();
            order.rotate_left(turn % len);
            if turn % 2 == 0 {
                order.reverse();
            }
            shuffled.effects = order;
            shuffled.layers.reset();
            prop_assert_eq!(view(&stored), view(&shuffled));
            // Asking again, and asking a clone, agree.
            prop_assert_eq!(view(&stored), view(&stored));
            prop_assert_eq!(view(&stored), view(&stored.clone()));
        }
    }
}
