//! Goal: a static ability does what a continuous effect with the same modification does, whatever the modification (the same characteristics, controller, keywords, abilities and restrictions); it applies in the order of the layers among the effects of resolved spells and abilities, by the timestamp its source entered with; it stops when its source leaves, when its condition fails and when its source has lost the ability, and it is not run from a zone other than the battlefield; the reader lists the abilities it grants under addresses that name them and none that were taken away.
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::definition::ManaAbility;
use mtg_cards::keyword::Keyword;
use mtg_cards::{AbilityDef, CardDefinition};
use mtg_core::card::CardType;
use mtg_core::ids::EffectId;
use mtg_core::mana::ManaType;
use rstest::rstest;

use super::*;
use crate::abilities::{AbilityRef, AbilitySource};
use crate::ability_reader::{abilities_of, ability};
use crate::characteristics::{
    cant_attack, cant_be_blocked_by, cant_block, characteristics, controller_of,
};
use crate::effects::{ContinuousEffect, Duration, Selector};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::keywords::keywords_of;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);
const TRIGGER: &str =
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#;
const ACTIVATED: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#;

pub(super) fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A 2/3 with flying, a triggered ability and a plain mana ability: everything there is to lose.
pub(super) fn bear() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 3)
            .with_rules(Ability::Keyword(Keyword::Flying))
            .with_rules(rules(TRIGGER))
            .with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green,
            })),
    )
}

/// An artifact whose only ability is `ability`; it is the Totem of the tests.
fn totem(ability: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Totem")
            .with_type(CardType::Artifact)
            .with_rules(rules(ability)),
    )
}

/// A Totem that does nothing.
fn idle_totem() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Totem").with_type(CardType::Artifact))
}

/// A static ability of the Totem that changes the Bear with the modification given.
fn on_the_bear(modification: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Named":"Bear"}}]}},"mods":[{modification}]}}}}"#
    )
}

pub(super) fn named(game: &GameState, name: &str) -> ObjectRef {
    game.objects()
        .find(|o| o.card().name == name)
        .map(GameObject::reference)
        .unwrap()
}

/// A Bear of seat 0 beside the Totem of seat 0 and an Ogre of seat 1.
fn game_with(totem: Arc<CardDefinition>) -> GameState {
    state(
        vec![vec![], vec![]],
        vec![vec![bear(), totem], vec![creature("Ogre", 4, 4)]],
    )
}

/// Everything the rest of the engine reads of one permanent.
#[derive(Debug, PartialEq)]
struct View {
    chars: Characteristics,
    controller: PlayerId,
    keywords: Vec<Keyword>,
    abilities: Vec<Ability>,
    cant_attack: bool,
    cant_block: bool,
    /// The permanents that can't block it.
    kept_out: Vec<String>,
}

fn view(game: &GameState, name: &str) -> View {
    let id = named(game, name).id;
    View {
        chars: characteristics(game, id).unwrap(),
        controller: controller_of(game, id).unwrap(),
        keywords: keywords_of(game, id).into_iter().cloned().collect(),
        abilities: abilities_of(game, id)
            .into_iter()
            .map(|(_, rules)| (*rules).clone())
            .collect(),
        cant_attack: cant_attack(game, id),
        cant_block: cant_block(game, id),
        // The Totems that hold the static abilities are not part of the comparison.
        kept_out: game
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|other| cant_be_blocked_by(game, id, *other))
            .filter_map(|other| game.object(other))
            .map(|other| other.card().name.clone())
            .filter(|name| !name.starts_with("Totem"))
            .collect(),
    }
}

pub(super) fn pump(game: &GameState, name: &str) -> Option<(i32, i32)> {
    let chars = characteristics(game, named(game, name).id)?;
    Some((chars.power?, chars.toughness?))
}

/// A registered effect with `modification` on the object `name`.
pub(super) fn effect(game: &mut GameState, name: &str, modification: Modification, timestamp: u64) {
    let id = EffectId(game.effects.len() as u64);
    let on = Selector::Objects(vec![named(game, name)]);
    game.effects.push(ContinuousEffect::new(
        id,
        on,
        modification,
        Duration::Forever,
        timestamp,
    ));
    game.layers.reset();
}

pub(super) fn fold(game: &mut GameState, events: &[Event]) {
    for event in events {
        apply_event(game, event).unwrap_or_else(|error| panic!("{event:?}: {error}"));
    }
}

fn leaves(game: &mut GameState, name: &str) {
    let object = named(game, name).id;
    let owner = game.object(object).unwrap().owner();
    fold(
        game,
        &[Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Destroyed,
        }],
    );
}

/// The modifications the engine runs.
const MODIFICATIONS: [&str; 24] = [
    r#"{"ModifyPT":{"power":2,"toughness":-1}}"#,
    r#"{"SetPT":{"power":5,"toughness":1}}"#,
    r#""SwitchPT""#,
    r#"{"AddType":"Artifact"}"#,
    r#"{"RemoveType":"Creature"}"#,
    r#"{"SetTypes":["Land"]}"#,
    r#"{"AddSubtype":"Elf"}"#,
    r#"{"SetCreatureTypes":["Weird"]}"#,
    r#""AddAllCreatureTypes""#,
    r#"{"SetColors":["Blue"]}"#,
    r#"{"AddKeyword":"Haste"}"#,
    r#"{"RemoveKeyword":"Flying"}"#,
    r#"{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}}"#,
    r#""LoseAbilities""#,
    r#"{"SetController":"Opponents"}"#,
    r#""CantAttack""#,
    r#""CantBlock""#,
    r#""DoesntUntap""#,
    r#""CantBeBlocked""#,
    r#"{"CantBeBlockedBy":[{"Type":"Creature"}]}"#,
    r#"{"CanBlockOnly":[{"HasKeyword":"Flying"}]}"#,
    r#"{"MinBlockers":2}"#,
    r#"{"MaxBlockers":1}"#,
    r#""Goaded""#,
];

#[test]
fn the_table_of_modifications_has_every_variant_of_the_dsl() {
    let named: std::collections::BTreeSet<String> = MODIFICATIONS
        .iter()
        .map(|json| {
            let wanted: mtg_cards::modify::Modification = serde_json::from_str(json).unwrap();
            match serde_json::to_value(&wanted).unwrap() {
                serde_json::Value::String(name) => name,
                serde_json::Value::Object(map) => map.keys().next().cloned().unwrap(),
                other => panic!("{other}"),
            }
        })
        .collect();
    assert_eq!(named.len(), MODIFICATIONS.len(), "one row for each variant");
    // Every variant of `Modification` is in the table: a match with no wildcard fails to compile when one is added.
    for json in MODIFICATIONS {
        match serde_json::from_str::<mtg_cards::modify::Modification>(json).unwrap() {
            mtg_cards::modify::Modification::ModifyPT { .. }
            | mtg_cards::modify::Modification::SetPT { .. }
            | mtg_cards::modify::Modification::SwitchPT
            | mtg_cards::modify::Modification::AddType(_)
            | mtg_cards::modify::Modification::RemoveType(_)
            | mtg_cards::modify::Modification::SetTypes(_)
            // Only the exceptions of a copy have it, which `CardDefinition::except` applies; it is not a row of the table.
            | mtg_cards::modify::Modification::RemoveSupertype(_)
            | mtg_cards::modify::Modification::AddSubtype(_)
            | mtg_cards::modify::Modification::SetCreatureTypes(_)
            | mtg_cards::modify::Modification::AddAllCreatureTypes
            | mtg_cards::modify::Modification::SetColors(_)
            | mtg_cards::modify::Modification::AddKeyword(_)
            // Only an effect has it, and it works out its keyword with an answer, so it is not a row of the table.
            | mtg_cards::modify::Modification::AddProtectionFromChosenColor
            | mtg_cards::modify::Modification::RemoveKeyword(_)
            | mtg_cards::modify::Modification::GrantAbility(_)
            | mtg_cards::modify::Modification::LoseAbilities
            | mtg_cards::modify::Modification::SetController(_)
            | mtg_cards::modify::Modification::CantAttack
            | mtg_cards::modify::Modification::CantAttackYou(_)
            | mtg_cards::modify::Modification::CantBlock
            | mtg_cards::modify::Modification::DoesntUntap
            | mtg_cards::modify::Modification::CantBeBlocked
            | mtg_cards::modify::Modification::CantBeBlockedBy(_)
            | mtg_cards::modify::Modification::CanBlockOnly(_)
            | mtg_cards::modify::Modification::MinBlockers(_)
            | mtg_cards::modify::Modification::MaxBlockers(_)
            | mtg_cards::modify::Modification::BlocksAdditional(_)
            | mtg_cards::modify::Modification::Goaded => {}
        }
    }
}

#[rstest]
fn every_modification_gives_the_same_view_as_a_static_ability_and_as_an_effect(
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)] which: usize,
) {
    let modification = MODIFICATIONS[which];
    let mut by_effect = game_with(idle_totem());
    let wanted: mtg_cards::modify::Modification = serde_json::from_str(modification).unwrap();
    let source = named(&by_effect, "Totem");
    let lowered = lower(&Scope::announcing(&by_effect, ME, source), &wanted).unwrap();
    effect(&mut by_effect, "Bear", lowered, 0);
    let by_static = game_with(totem(&on_the_bear(modification)));
    let unchanged = game_with(idle_totem());
    let (effect_view, static_view) = (view(&by_effect, "Bear"), view(&by_static, "Bear"));
    assert_eq!(effect_view, static_view, "{modification}");
    assert_ne!(
        effect_view,
        view(&unchanged, "Bear"),
        "{modification} changes the Bear"
    );
    // What the modification does to other objects is the same too: nothing.
    assert_eq!(view(&by_static, "Ogre"), view(&unchanged, "Ogre"));
}

#[test]
fn a_static_pump_waits_for_a_set_whatever_the_age_of_either() {
    // The Totem's pump entered with the game (timestamp 0); the set is an effect made later: the set comes first.
    let mut game = game_with(totem(&on_the_bear(
        r#"{"ModifyPT":{"power":1,"toughness":1}}"#,
    )));
    assert_eq!(pump(&game, "Bear"), Some((3, 4)));
    effect(
        &mut game,
        "Bear",
        Modification::SetPowerToughness {
            power: 1,
            toughness: 1,
        },
        9,
    );
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
}

#[test]
fn within_a_layer_the_later_timestamp_wins_a_static_ability_that_entered_later_included() {
    let set = |power: i32| {
        on_the_bear(&format!(
            r#"{{"SetPT":{{"power":{power},"toughness":{power}}}}}"#
        ))
    };
    let mut game = state(
        vec![
            vec![Arc::new(
                CardDefinition::new("Late")
                    .with_type(CardType::Artifact)
                    .with_rules(rules(&set(7))),
            )],
            vec![],
        ],
        vec![vec![bear(), totem(&set(4))], vec![]],
    );
    assert_eq!(
        pump(&game, "Bear"),
        Some((4, 4)),
        "the Totem entered with the game"
    );
    // The Late artifact enters now, with the next timestamp, so its set applies after the Totem's.
    let late = named(&game, "Late").id;
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: late,
            from: Zone::Hand(ME),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        }],
    );
    assert_eq!(pump(&game, "Bear"), Some((7, 7)));
    // An effect made after it, and one made before it, order the same way.
    effect(
        &mut game,
        "Bear",
        Modification::SetPowerToughness {
            power: 9,
            toughness: 9,
        },
        0,
    );
    assert_eq!(
        pump(&game, "Bear"),
        Some((7, 7)),
        "the set of timestamp 0 is older than the Late artifact's"
    );
    effect(
        &mut game,
        "Bear",
        Modification::SetPowerToughness {
            power: 9,
            toughness: 9,
        },
        1_000,
    );
    assert_eq!(pump(&game, "Bear"), Some((9, 9)));
}

#[test]
fn a_static_ability_stops_when_its_source_leaves_and_starts_again_with_a_new_one() {
    let mut game = game_with(totem(&on_the_bear(
        r#"{"ModifyPT":{"power":1,"toughness":1}}"#,
    )));
    assert_eq!(pump(&game, "Bear"), Some((3, 4)));
    let totem = named(&game, "Totem");
    leaves(&mut game, "Totem");
    assert_eq!(pump(&game, "Bear"), Some((2, 3)), "the source left");
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: totem.id,
            from: Zone::Graveyard(ME),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        }],
    );
    assert_eq!(
        pump(&game, "Bear"),
        Some((3, 4)),
        "it came back as a new object, with its ability"
    );
}

#[rstest]
#[case::life_at_the_least(r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#, true)]
#[case::life_above(r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":21}}"#, false)]
#[case::their_turn(r#"{"Not":"YourTurn"}"#, false)]
#[case::our_turn(r#""YourTurn""#, true)]
fn a_static_ability_works_only_while_its_condition_holds(
    #[case] condition: &str,
    #[case] works: bool,
) {
    let json = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Named":"Bear"}}]}},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}],"condition":{condition}}}}}"#
    );
    let game = game_with(totem(&json));
    assert_eq!(
        pump(&game, "Bear"),
        Some(if works { (3, 4) } else { (2, 3) })
    );
}

#[test]
fn a_condition_follows_the_game_as_it_changes() {
    let json = r#"{"Static":{"affected":{"Each":[{"Named":"Bear"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}}}"#;
    let mut game = game_with(totem(json));
    assert_eq!(pump(&game, "Bear"), Some((3, 4)));
    fold(
        &mut game,
        &[Event::LifeLost {
            player: ME,
            amount: 1,
        }],
    );
    assert_eq!(pump(&game, "Bear"), Some((2, 3)));
    fold(
        &mut game,
        &[Event::LifeGained {
            player: ME,
            amount: 1,
        }],
    );
    assert_eq!(pump(&game, "Bear"), Some((3, 4)));
}

#[test]
fn an_ability_that_works_from_another_zone_does_nothing_on_the_battlefield() {
    let json = r#"{"Static":{"affected":{"Each":[{"Named":"Bear"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"from":"Graveyard"}}"#;
    let game = game_with(totem(json));
    assert_eq!(pump(&game, "Bear"), Some((2, 3)));
}

#[rstest]
#[case::this("\"This\"", false, false)]
#[case::every_creature_of_mine(
    r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    true,
    false
)]
#[case::every_creature_of_theirs(
    r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#,
    false,
    true
)]
#[case::every_creature(r#"{"Each":[{"Type":"Creature"}]}"#, true, true)]
#[case::the_big_ones(
    r#"{"Each":[{"Compare":{"stat":"Power","op":"Ge","to":3}}]}"#,
    false,
    true
)]
#[case::other_than_the_source(r#"{"Each":[{"Type":"Creature"},"Other"]}"#, true, true)]
#[case::not_an_artifact(r#"{"Each":[{"Not":{"Type":"Artifact"}}]}"#, true, true)]
fn a_static_ability_affects_the_objects_its_reference_names_now(
    #[case] affected: &str,
    #[case] bear_pumped: bool,
    #[case] ogre_pumped: bool,
) {
    let json = format!(
        r#"{{"Static":{{"affected":{affected},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}]}}}}"#
    );
    // "This" is the Totem, which is no creature: the Bear and the Ogre are not what it names.
    let game = game_with(totem(&json));
    let shown = |name| pump(&game, name);
    assert_eq!(
        shown("Bear"),
        Some(if bear_pumped { (3, 4) } else { (2, 3) })
    );
    assert_eq!(
        shown("Ogre"),
        Some(if ogre_pumped { (5, 5) } else { (4, 4) })
    );
}

#[test]
fn a_static_ability_that_changes_what_it_selects_by_is_read_by_the_layers_before_it() {
    // "Artifact creatures get +2/+2" next to "Creatures you control are artifacts": the type comes first (layer 4), so the
    // selector of the pump finds the new artifacts.
    let artifacts = on_the_bear(r#"{"AddType":"Artifact"}"#);
    let pump_artifacts = r#"{"Static":{"affected":{"Each":[{"Type":"Artifact"},{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}]}}"#;
    let game = state(
        vec![vec![], vec![]],
        vec![
            vec![
                bear(),
                totem(&artifacts),
                Arc::new(
                    CardDefinition::new("Smith")
                        .with_type(CardType::Artifact)
                        .with_rules(rules(pump_artifacts)),
                ),
            ],
            vec![creature("Ogre", 4, 4)],
        ],
    );
    assert_eq!(pump(&game, "Bear"), Some((4, 5)));
    assert_eq!(pump(&game, "Ogre"), Some((4, 4)));
}

/// Two sources of Humility: all creatures lose all abilities and are 1/1.
const HUMILITY: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["LoseAbilities",{"SetPT":{"power":1,"toughness":1}}]}}"#;
const LORD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other"]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;

fn lord() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Lord")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(rules(LORD)),
    )
}

#[test]
fn humility_takes_every_ability_and_sets_power_and_toughness_and_a_lord_stops_pumping() {
    let board = |humility: bool| {
        state(
            vec![vec![], vec![]],
            vec![
                vec![bear(), lord()]
                    .into_iter()
                    .chain(humility.then(|| totem(HUMILITY)))
                    .collect(),
                vec![creature("Ogre", 4, 4)],
            ],
        )
    };
    let without = board(false);
    assert_eq!(
        pump(&without, "Bear"),
        Some((3, 4)),
        "the Lord pumps the Bear"
    );
    assert_eq!(pump(&without, "Lord"), Some((2, 2)));
    assert_eq!(
        keywords_of(&without, named(&without, "Bear").id),
        [&Keyword::Flying]
    );
    let with = board(true);
    for name in ["Bear", "Lord", "Ogre"] {
        assert_eq!(
            pump(&with, name),
            Some((1, 1)),
            "{name} is 1/1, and the Lord's pump is lost with its ability"
        );
    }
    let bear = named(&with, "Bear").id;
    assert_eq!(
        keywords_of(&with, bear),
        Vec::<&Keyword>::new(),
        "flying is lost"
    );
    assert_eq!(
        abilities_of(&with, bear),
        Vec::new(),
        "every ability is lost"
    );
    assert!(
        characteristics(&with, bear).unwrap().abilities.is_empty(),
        "the mana ability too"
    );
    assert_eq!(abilities_of(&with, named(&with, "Lord").id), Vec::new());
}

#[test]
fn what_is_given_after_the_ability_loss_counts_and_what_was_given_before_it_does_not() {
    let humility = Arc::new(
        CardDefinition::new("Humility")
            .with_type(CardType::Enchantment)
            .with_rules(rules(HUMILITY)),
    );
    let mut game = state(vec![vec![humility], vec![]], vec![vec![bear()], vec![]]);
    effect(
        &mut game,
        "Bear",
        Modification::AddKeyword(Keyword::Haste),
        0,
    );
    // Humility enters after the effect of timestamp 0 was made, so it takes what the effect gave.
    let id = named(&game, "Humility").id;
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: id,
            from: Zone::Hand(ME),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        }],
    );
    let bear = named(&game, "Bear").id;
    assert_eq!(
        keywords_of(&game, bear),
        Vec::<&Keyword>::new(),
        "an older grant is lost"
    );
    effect(
        &mut game,
        "Bear",
        Modification::AddKeyword(Keyword::Vigilance),
        500,
    );
    assert_eq!(
        keywords_of(&game, bear),
        [&Keyword::Vigilance],
        "a newer one is kept"
    );
}

#[test]
fn an_ability_that_acts_before_layer_six_is_kept_when_its_source_loses_its_abilities() {
    // "Creatures you control are artifacts" begins in layer 4, so Humility's layer 6 does not stop it (CR 613.6).
    let mut cards = vec![Arc::new(
        CardDefinition::new("Lord")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(rules(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other"]},"mods":[{"AddType":"Artifact"},{"ModifyPT":{"power":1,"toughness":1}}]}}"#)),
    )];
    cards.push(bear());
    cards.push(totem(HUMILITY));
    let game = state(vec![vec![], vec![]], vec![cards, vec![]]);
    let bear = named(&game, "Bear").id;
    let chars = characteristics(&game, bear).unwrap();
    assert!(
        chars.types.contains(CardType::Artifact),
        "the type change begun in layer 4 stays"
    );
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(2), Some(2)),
        "and so does the pump that comes with it: 1/1 and +1/+1"
    );
}

#[test]
fn a_control_changing_static_ability_changes_who_controls_the_permanent() {
    let steal =
        r#"{"Static":{"affected":{"Each":[{"Named":"Ogre"}]},"mods":[{"SetController":"You"}]}}"#;
    let mut game = game_with(totem(steal));
    let ogre = named(&game, "Ogre");
    assert_eq!(controller_of(&game, ogre.id), Some(ME));
    assert_eq!(
        game.object(ogre.id).unwrap().controller(),
        FOE,
        "the stored controller is not changed"
    );
    assert_eq!(snapshot_controller(&game, ogre.id), ME);
    leaves(&mut game, "Totem");
    assert_eq!(controller_of(&game, ogre.id), Some(FOE));
}

fn snapshot_controller(game: &GameState, id: ObjectId) -> PlayerId {
    crate::snapshot::snapshot(game, id).unwrap().controller
}

#[test]
fn a_permanent_that_changes_hands_is_held_back_until_its_new_controller_has_had_it_for_a_turn() {
    let steal =
        r#"{"Static":{"affected":{"Each":[{"Named":"Ogre"}]},"mods":[{"SetController":"You"}]}}"#;
    let mut game = game_with(totem(steal));
    let ogre = named(&game, "Ogre").id;
    let sick = |game: &GameState| {
        game.object(ogre)
            .and_then(GameObject::permanent)
            .is_some_and(|permanent| permanent.sick_for(controller_of(game, ogre).unwrap()))
    };
    assert!(
        sick(&game),
        "it was seat 1's, and seat 0 has not had it since their turn began"
    );
    fold(
        &mut game,
        &[Event::TurnBegan {
            number: 3,
            active: ME,
            extra: false,
        }],
    );
    assert!(!sick(&game), "seat 0's turn began with it in their control");
    leaves(&mut game, "Totem");
    assert!(
        sick(&game),
        "it is back with seat 1, who has not had it since their turn began"
    );
}

#[rstest]
#[case::cant_attack(r#""CantAttack""#, true, false)]
#[case::cant_block(r#""CantBlock""#, false, true)]
fn a_restriction_applies_to_what_the_static_ability_names(
    #[case] modification: &str,
    #[case] attack: bool,
    #[case] block: bool,
) {
    let game = game_with(totem(&on_the_bear(modification)));
    let bear = named(&game, "Bear").id;
    assert_eq!(
        (cant_attack(&game, bear), cant_block(&game, bear)),
        (attack, block)
    );
    let ogre = named(&game, "Ogre").id;
    assert_eq!(
        (cant_attack(&game, ogre), cant_block(&game, ogre)),
        (false, false)
    );
}

/// The addresses of the abilities of the Bear that are not printed.
fn granted(game: &GameState) -> Vec<(AbilityRef, Arc<Ability>)> {
    let bear = named(game, "Bear").id;
    abilities_of(game, bear)
        .into_iter()
        .filter(|(address, _)| !address.source.is_printed())
        .collect()
}

#[test]
fn the_reader_lists_a_granted_ability_after_the_printed_ones_under_an_address_that_names_it() {
    let game = game_with(totem(&on_the_bear(&format!(
        r#"{{"GrantAbility":{ACTIVATED}}}"#
    ))));
    let bear = named(&game, "Bear").id;
    let all = abilities_of(&game, bear);
    assert_eq!(all.len(), 3, "flying, the trigger, and the one granted");
    let (address, granted_rules) = all.last().cloned().unwrap();
    let totem = named(&game, "Totem").id;
    assert_eq!(
        address,
        AbilityRef::granted(
            bear,
            Grant::Static {
                source: totem,
                rule: 0,
                index: 0
            }
        )
    );
    assert_eq!(*granted_rules, rules(ACTIVATED));
    let read = ability(&game, address).unwrap();
    assert!(
        Arc::ptr_eq(&read, &granted_rules),
        "the address reads the very ability that is listed"
    );
    assert_eq!(
        abilities_of(&game, bear),
        abilities_of(&game, bear),
        "the same state lists the same addresses"
    );
}

#[test]
fn a_granted_address_is_read_from_the_data_so_it_survives_what_granted_it() {
    let mut game = game_with(totem(&on_the_bear(&format!(
        r#"{{"GrantAbility":{ACTIVATED}}}"#
    ))));
    let (address, listed) = granted(&game).pop().unwrap();
    leaves(&mut game, "Totem");
    assert!(granted(&game).is_empty(), "the Bear no longer has it");
    let read =
        ability(&game, address).expect("the address still names the ability the Totem printed");
    assert!(Arc::ptr_eq(&read, &listed));
    // An address with an index of its own names nothing.
    let mut wrong = address;
    wrong.ability = 1;
    assert!(ability(&game, wrong).is_none());
    // Nor does one that names a modification that grants nothing.
    let AbilitySource::Granted(Grant::Static { source, rule, .. }) = address.source else {
        panic!("a static grant");
    };
    let elsewhere = AbilityRef::granted(
        address.object,
        Grant::Static {
            source,
            rule,
            index: 3,
        },
    );
    assert!(ability(&game, elsewhere).is_none());
}

#[test]
fn an_effect_that_grants_an_ability_names_it_by_the_effect_and_it_ends_with_the_effect() {
    let mut game = game_with(idle_totem());
    let wanted = Modification::GrantAbility(Arc::new(rules(ACTIVATED)));
    effect(&mut game, "Bear", wanted, 0);
    let bear = named(&game, "Bear").id;
    let (address, listed) = granted(&game).pop().unwrap();
    assert_eq!(
        address,
        AbilityRef::granted(bear, Grant::Effect(EffectId(0)))
    );
    assert!(Arc::ptr_eq(&ability(&game, address).unwrap(), &listed));
    fold(&mut game, &[Event::EffectExpired { id: EffectId(0) }]);
    assert_eq!(granted(&game).len(), 0);
    assert!(
        ability(&game, address).is_none(),
        "an effect that is gone names nothing"
    );
}

#[test]
fn the_abilities_that_are_lost_are_not_listed_and_a_keyword_taken_away_goes_with_them() {
    let game = game_with(totem(&on_the_bear(r#"{"RemoveKeyword":"Flying"}"#)));
    let bear = named(&game, "Bear").id;
    let kinds: Vec<bool> = abilities_of(&game, bear)
        .iter()
        .map(|(_, rules)| matches!(**rules, Ability::Triggered { .. }))
        .collect();
    assert_eq!(
        kinds,
        [true],
        "the trigger is left, the printed keyword is not"
    );
    assert_eq!(keywords_of(&game, bear), Vec::<&Keyword>::new());
    // The address of what is left is still the index it has among the printed rules.
    assert_eq!(abilities_of(&game, bear)[0].0, AbilityRef::printed(bear, 1));
}

#[test]
fn a_state_without_an_effect_or_a_static_ability_works_out_no_layers() {
    let game = game_with(idle_totem());
    assert!(!game.statics_possible);
    assert_eq!(game.effects(), []);
    assert!(game.layers().get(named(&game, "Bear").id).is_none());
    let with = game_with(totem(&on_the_bear(
        r#"{"ModifyPT":{"power":5,"toughness":5}}"#,
    )));
    assert!(with.layers().get(named(&with, "Bear").id).is_some());
    assert_eq!(pump(&with, "Bear"), Some((7, 8)));
}

#[test]
fn lifelink_gains_life_for_whoever_controls_the_source_now() {
    let steal =
        r#"{"Static":{"affected":{"Each":[{"Named":"Ogre"}]},"mods":[{"SetController":"You"}]}}"#;
    let lifelinker = Arc::new(
        CardDefinition::new("Ogre")
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4)
            .with_rules(Ability::Keyword(Keyword::Lifelink)),
    );
    let game = state(
        vec![vec![], vec![]],
        vec![vec![totem(steal)], vec![lifelinker]],
    );
    let ogre = named(&game, "Ogre").id;
    let dealer = crate::damage::Dealer::live(&game, ogre);
    let events =
        crate::damage::dealt_by(&game, &dealer, crate::combat::DamageTarget::Player(FOE), 3);
    assert!(
        events.contains(&Event::LifeGained {
            player: ME,
            amount: 3
        }),
        "{events:?}"
    );
}

/// A token with the rules `json` says, or none, enters the battlefield of seat 0.
fn token(game: &mut GameState, json: Option<&str>) {
    let object = game.next_object();
    let mut definition = CardDefinition::new("Gem").with_type(CardType::Artifact);
    if let Some(json) = json {
        definition = definition.with_rules(rules(json));
    }
    let created = crate::tokens::TokenCreated {
        object,
        copy_of: None,
        controller: ME,
        name: "Gem".into(),
        tapped: false,
        attacking: None,
        definition: Arc::new(definition),
    };
    fold(game, &[Event::TokenCreated(created)]);
}

/// An effect that gives the Bear the ability `json`.
fn granting(game: &mut GameState, json: &str) {
    let created = ContinuousEffect::new(
        EffectId(game.effects.len() as u64),
        Selector::Objects(vec![named(game, "Bear")]),
        Modification::GrantAbility(Arc::new(rules(json))),
        Duration::Forever,
        5,
    );
    fold(game, &[Event::EffectCreated { effect: created }]);
}

#[rstest]
#[case::a_token_that_triggers(Some(TRIGGER), (true, false))]
#[case::a_token_with_a_static_ability(Some(r#"{"Static":{"affected":"This","mods":["CantBlock"]}}"#), (false, true))]
#[case::a_token_with_an_activated_ability(Some(ACTIVATED), (false, false))]
#[case::a_token_with_no_ability(None, (false, false))]
#[case::a_token_that_gives_a_trigger(
    Some(r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}}]}}"#),
    (true, true)
)]
fn what_can_trigger_and_what_is_static_follows_the_tokens_that_enter_play(
    #[case] json: Option<&str>,
    #[case] expected: (bool, bool),
) {
    let mut game = state(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    assert_eq!(
        (game.triggers_possible, game.statics_possible),
        (false, false)
    );
    assert!(!game.watches());
    token(&mut game, json);
    assert_eq!((game.triggers_possible, game.statics_possible), expected);
    assert_eq!(game.watches(), expected.0);
}

#[rstest]
#[case::a_triggered_ability(TRIGGER, true)]
#[case::an_activated_ability(ACTIVATED, false)]
fn what_can_trigger_follows_the_effects_that_give_an_ability(
    #[case] json: &str,
    #[case] triggers: bool,
) {
    let mut game = state(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    assert!(!game.watches());
    granting(&mut game, json);
    assert_eq!(game.triggers_possible, triggers);
    assert!(!game.statics_possible, "an effect is not a static ability");
}

#[test]
fn the_flags_only_ever_turn_on() {
    let mut game = state(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    token(&mut game, Some(TRIGGER));
    let gem = named(&game, "Gem").id;
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: gem,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Sacrificed,
        }],
    );
    assert!(
        game.triggers_possible,
        "nothing turns what a game has seen off"
    );
}

mod properties {
    //! Any list of modifications, in any order, changes a permanent the same way as effects of resolved spells and abilities and as static abilities of other permanents that entered in that order.
    use proptest::collection::vec;
    use proptest::prelude::{ProptestConfig, prop_assert_eq, proptest};

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        #[test]
        fn a_list_of_modifications_is_the_same_as_effects_and_as_static_abilities(
            which in vec(0..MODIFICATIONS.len(), 1..6),
        ) {
            let mut by_effect = game_with(idle_totem());
            let bear = named(&by_effect, "Bear");
            for (n, index) in which.iter().enumerate() {
                let wanted: mtg_cards::modify::Modification =
                    serde_json::from_str(MODIFICATIONS[*index]).unwrap();
                let lowered = lower(&Scope::announcing(&by_effect, ME, bear), &wanted).unwrap();
                by_effect.effects.push(ContinuousEffect::new(
                    EffectId(n as u64),
                    Selector::Objects(vec![bear]),
                    lowered,
                    Duration::Forever,
                    0,
                ));
            }
            by_effect.layers.reset();
            let mut battlefield = vec![bear_card()];
            battlefield.extend(which.iter().map(|index| totem(&on_the_bear(MODIFICATIONS[*index]))));
            let by_static = state(
                vec![vec![], vec![]],
                vec![battlefield, vec![creature("Ogre", 4, 4)]],
            );
            prop_assert_eq!(view(&by_effect, "Bear"), view(&by_static, "Bear"), "{:?}", which);
            // Asking again of the same state gives the same view: what a state gathers does not depend on the asking.
            prop_assert_eq!(view(&by_static, "Bear"), view(&by_static, "Bear"));
        }
    }

    fn bear_card() -> Arc<CardDefinition> {
        bear()
    }
}

#[test]
fn a_clone_gathers_its_own_effects_and_an_event_drops_them() {
    let mut game = game_with(totem(&on_the_bear(
        r#"{"ModifyPT":{"power":1,"toughness":1}}"#,
    )));
    assert_eq!(pump(&game, "Bear"), Some((3, 4)));
    let copy = game.clone();
    leaves(&mut game, "Totem");
    assert_eq!(pump(&game, "Bear"), Some((2, 3)));
    assert_eq!(
        pump(&copy, "Bear"),
        Some((3, 4)),
        "the copy was made before the Totem left"
    );
}
