//! Goal: a characteristic-defining ability (CR 604.3) defines power and toughness, colours, types and subtypes of its own object in every zone, counting the permanents as the layers leave them; on the battlefield it applies first in its layer, before the other effects of the layer whatever their timestamps; one that acts after layer 6 is lost with the abilities, one before it is not; and abilities that count each other's objects end.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::EffectId;
use rstest::rstest;

use super::tests::{bear, fold, named, rules};
use super::*;
use crate::characteristics::characteristics;
use crate::effects::{ContinuousEffect, Duration, Selector};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};

const ME: PlayerId = PlayerId(0);

const CREATURES_I_CONTROL: &str = r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;

/// A creature with no printed power or toughness whose power is the creatures its controller has and whose toughness is
/// one more.
fn hulk() -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetPT":{{"power":{CREATURES_I_CONTROL},"toughness":{{"Sum":[{CREATURES_I_CONTROL},1]}}}}}}],"defining":true}}}}"#
    );
    Arc::new(
        CardDefinition::new("Hulk")
            .with_type(CardType::Creature)
            .with_rules(rules(&ability)),
    )
}

/// A creature that is all colours by a characteristic-defining ability or, when `defining` is off, by an ordinary static
/// ability.
fn chameleon(defining: bool) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetColors":["White","Blue","Black","Red","Green"]}}],"defining":{defining}}}}}"#
    );
    Arc::new(
        CardDefinition::new("Chameleon")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules(&ability)),
    )
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Where {
    Battlefield,
    Hand,
    Library,
    Graveyard,
    Exile,
    Stack,
}

/// Seat 0 has a Bear and a Wolf on the battlefield and `card` in the zone `at`.
pub(super) fn game_with(card: Arc<CardDefinition>, at: Where) -> GameState {
    let mut game_setup = setup(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2), creature("Wolf", 3, 3)], vec![]],
    );
    let player = &mut game_setup.players[0];
    match at {
        Where::Battlefield => player
            .battlefield
            .push(crate::setup::PresetPermanent::new(card)),
        Where::Library => player.library.push(card),
        Where::Graveyard => player.graveyard.push(card),
        Where::Hand | Where::Exile | Where::Stack => player.hand.push(card),
    }
    let mut game = GameState::from_setup(&game_setup).unwrap();
    let to = match at {
        Where::Exile => Some(Zone::Exile),
        Where::Stack => Some(Zone::Stack),
        _ => None,
    };
    if let Some(to) = to {
        let object = game.objects().find(|o| o.card().name == "Hulk").unwrap().id;
        fold(
            &mut game,
            &[Event::ZoneChanged {
                object,
                from: Zone::Hand(ME),
                to,
                cause: ZoneCause::Moved,
            }],
        );
    }
    game
}

fn stats_of(game: &GameState, name: &str) -> (Option<i32>, Option<i32>) {
    let chars = characteristics(game, named(game, name).id).unwrap();
    (chars.power, chars.toughness)
}

#[rstest]
#[case::the_battlefield(Where::Battlefield, (3, 4))]
#[case::a_hand(Where::Hand, (2, 3))]
#[case::a_library(Where::Library, (2, 3))]
#[case::a_graveyard(Where::Graveyard, (2, 3))]
#[case::exile(Where::Exile, (2, 3))]
#[case::the_stack(Where::Stack, (2, 3))]
fn a_characteristic_defining_ability_works_in_every_zone(
    #[case] at: Where,
    #[case] expected: (i32, i32),
) {
    // The Hulk counts the creatures its controller has: the Bear and the Wolf, and itself when it is one of them.
    let game = game_with(hulk(), at);
    assert_eq!(
        stats_of(&game, "Hulk"),
        (Some(expected.0), Some(expected.1))
    );
    // The same each time asked, and by whoever asks (a clone, the snapshot of the object).
    assert_eq!(stats_of(&game, "Hulk"), stats_of(&game.clone(), "Hulk"));
}

#[rstest]
#[case::on_the_battlefield(Where::Battlefield)]
#[case::in_hand(Where::Hand)]
fn what_a_characteristic_defining_ability_counts_is_read_as_the_layers_leave_it(#[case] at: Where) {
    // An effect that makes the Bear an artifact creature is the same as another creature for a count of artifacts: the
    // Hulk counts "artifacts you control" and finds the Bear (layer 4) and the Gem.
    let ability = r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"toughness":0}}],"defining":true}}"#;
    let counter = Arc::new(
        CardDefinition::new("Hulk")
            .with_type(CardType::Creature)
            .with_rules(rules(ability)),
    );
    let mut game = game_with(counter, at);
    assert_eq!(stats_of(&game, "Hulk"), (Some(0), Some(0)));
    let bear = named(&game, "Bear");
    game.effects.push(ContinuousEffect::new(
        EffectId(0),
        Selector::Objects(vec![bear]),
        Modification::AddType(CardType::Artifact),
        Duration::Forever,
        1,
    ));
    game.layers.reset();
    assert_eq!(stats_of(&game, "Hulk"), (Some(1), Some(0)));
}

#[rstest]
#[case::defining_comes_first_so_the_newer_blue_wins(true, ColorSet::EMPTY.with(Color::Blue))]
#[case::an_ordinary_static_is_ordered_by_timestamp_so_it_comes_last(
    false,
    ColorSet::EMPTY.with(Color::White).with(Color::Blue).with(Color::Black).with(Color::Red).with(Color::Green)
)]
fn a_characteristic_defining_ability_applies_first_in_its_layer(
    #[case] defining: bool,
    #[case] colors: ColorSet,
) {
    // "Becomes blue" is a registered effect made at the start (timestamp 0, before the permanent's own static ability in
    // the order of ties), so without the flag the permanent's ability would come second and win.
    let mut game = game_with_chameleon(defining);
    let chameleon = named(&game, "Chameleon");
    game.effects.push(ContinuousEffect::new(
        EffectId(0),
        Selector::Objects(vec![chameleon]),
        Modification::SetColors(ColorSet::EMPTY.with(Color::Blue)),
        Duration::Forever,
        0,
    ));
    game.layers.reset();
    let chars = characteristics(&game, chameleon.id).unwrap();
    assert_eq!(chars.colors, colors);
}

fn game_with_chameleon(defining: bool) -> GameState {
    let mut game_setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
    game_setup.players[0]
        .battlefield
        .push(crate::setup::PresetPermanent::new(chameleon(defining)));
    GameState::from_setup(&game_setup).unwrap()
}

#[test]
fn a_characteristic_defining_ability_is_applied_in_the_layer_of_what_it_defines() {
    let game = game_with(hulk(), Where::Battlefield);
    let applied = &game.layers().get(named(&game, "Hulk").id).unwrap().applied;
    let layers: Vec<Option<Layer>> = applied.iter().map(|applied| applied.layer).collect();
    assert_eq!(layers, [Some(Layer::PtCharacteristicDefining)]);
}

#[test]
fn the_characteristic_defining_power_comes_before_the_other_effects_of_power_and_toughness() {
    // 7a defines 3/4, then 7b sets 1/1 and 7c adds +2/+2, 7d switches nothing: whatever the stamps of the effects.
    let mut game = game_with(hulk(), Where::Battlefield);
    let hulk = named(&game, "Hulk");
    for (n, (modification, stamp)) in [
        (
            Modification::ModifyPowerToughness {
                power: 2,
                toughness: 2,
            },
            0,
        ),
        (
            Modification::SetPowerToughness {
                power: 1,
                toughness: 1,
            },
            9,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        game.effects.push(ContinuousEffect::new(
            EffectId(n as u64),
            Selector::Objects(vec![hulk]),
            modification,
            Duration::Forever,
            stamp,
        ));
    }
    game.layers.reset();
    assert_eq!(stats_of(&game, "Hulk"), (Some(3), Some(3)));
}

#[rstest]
#[case::power_and_toughness_come_after_layer_six_and_are_lost_with_the_abilities(hulk(), (None, None), false)]
#[case::colours_come_before_it_and_stay(chameleon(true), (Some(1), Some(1)), true)]
fn an_ability_that_acts_after_layer_six_is_lost_with_the_abilities_one_before_it_is_not(
    #[case] card: Arc<CardDefinition>,
    #[case] stats: (Option<i32>, Option<i32>),
    #[case] colours_stay: bool,
) {
    let name = card.name.clone();
    let mut game = game_with(card, Where::Battlefield);
    let object = named(&game, &name);
    game.effects.push(ContinuousEffect::new(
        EffectId(0),
        Selector::Objects(vec![object]),
        Modification::LoseAbilities,
        Duration::Forever,
        5,
    ));
    game.layers.reset();
    let chars = characteristics(&game, object.id).unwrap();
    assert_eq!((chars.power, chars.toughness), stats);
    assert_eq!(chars.colors.iter().count() == 5, colours_stay);
}

#[rstest]
#[case::a_type("AddType", r#""Artifact""#)]
#[case::a_subtype("AddSubtype", r#""Elf""#)]
fn a_characteristic_defining_ability_can_define_types_and_subtypes(
    #[case] kind: &str,
    #[case] value: &str,
    #[values(Where::Battlefield, Where::Hand, Where::Graveyard)] at: Where,
) {
    let ability = format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"{kind}":{value}}}],"defining":true}}}}"#
    );
    let card = Arc::new(
        CardDefinition::new("Hulk")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules(&ability)),
    );
    let game = game_with(card, at);
    let chars = characteristics(&game, named(&game, "Hulk").id).unwrap();
    assert_eq!(chars.types.contains(CardType::Artifact), kind == "AddType");
    assert_eq!(
        chars.subtypes.contains(&Subtype("Elf".into())),
        kind == "AddSubtype"
    );
}

#[test]
fn abilities_that_count_each_other_end_and_give_the_same_answer_each_time() {
    // "Power: the creature cards in graveyards with power 1 or more", twice over: each counts the other and itself.
    let ability = r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Count":[{"In":"Graveyard"},{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":1}}]},"toughness":1}}],"defining":true}}"#;
    let twin = |name: &str| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_rules(rules(ability)),
        )
    };
    let mut game_setup = setup(vec![vec![], vec![]], vec![vec![bear()], vec![]]);
    game_setup.players[0].graveyard = vec![twin("Left"), twin("Right")];
    let game = GameState::from_setup(&game_setup).unwrap();
    let first = (stats_of(&game, "Left"), stats_of(&game, "Right"));
    assert_eq!(first, (stats_of(&game, "Left"), stats_of(&game, "Right")));
    assert_eq!(first.0.1, Some(1));
}

/// A card whose power and toughness are the creature cards in all graveyards.
fn goyf(name: &str) -> Arc<CardDefinition> {
    let ability = r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Count":[{"Type":"Creature"},{"In":"Graveyard"}]},"toughness":{"Count":[{"Type":"Creature"},{"In":"Graveyard"}]}}}],"defining":true}}"#;
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_rules(rules(ability)),
    )
}

#[test]
fn cards_in_graveyards_that_count_the_creature_cards_of_graveyards_are_not_worked_out_for_each_other()
 {
    // Each counts the types of the others and not their power, so none needs another's count: sixteen of them take
    // sixteen counts, not every order the cards can be counted in.
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut game_setup = setup(vec![vec![], vec![]], vec![vec![goyf("Alive")], vec![]]);
        game_setup.players[0].graveyard = (0..14).map(|n| goyf(&format!("Dead {n}"))).collect();
        game_setup.players[1].graveyard = vec![goyf("Dead 14"), goyf("Dead 15")];
        let game = GameState::from_setup(&game_setup).unwrap();
        let _ = sent.send((stats_of(&game, "Alive"), stats_of(&game, "Dead 0")));
    });
    let (alive, dead) = received
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("the counts did not end");
    assert_eq!(alive, (Some(16), Some(16)));
    assert_eq!(dead, (Some(16), Some(16)));
}

#[test]
fn a_count_that_reads_power_works_out_the_power_of_the_cards_it_looks_at() {
    // The total power of the creature cards in graveyards, read by a card in one: a Bones of 2, a goyf that counts the
    // three cards and the reader itself, which is read as printed while its own ability is under way.
    let ability = r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Total":{"of":[{"Type":"Creature"},{"In":"Graveyard"}],"stat":"Power"}},"toughness":1}}],"defining":true}}"#;
    let reader = Arc::new(
        CardDefinition::new("Reader")
            .with_type(CardType::Creature)
            .with_rules(rules(ability)),
    );
    let mut game_setup = setup(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    game_setup.players[0].graveyard = vec![creature("Bones", 2, 2), goyf("Goyf"), reader];
    let game = GameState::from_setup(&game_setup).unwrap();
    assert_eq!(stats_of(&game, "Goyf"), (Some(3), Some(3)));
    assert_eq!(stats_of(&game, "Reader"), (Some(5), Some(1)));
}
