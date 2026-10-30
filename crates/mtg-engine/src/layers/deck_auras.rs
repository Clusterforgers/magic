//! Goal: a landwalk an Aura grants "of the chosen type" names the land type its controller chose as it entered, on the creature it enchants now (it follows the Aura to another host and goes when the Aura leaves), and stays as written, matching no land, while nothing or another kind of thing was chosen.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::tests::{bear, effect, named, pump, rules};
use crate::ability_reader::abilities_of;
use crate::characteristics::characteristics;
use crate::effects::Modification;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::fold::apply_event;
use crate::keywords::keywords_of;
use crate::object::Chosen;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);
const CLOAK: &str =
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Landwalk":["ChosenSubtype"]}}]}}"#;

fn swamp() -> Subtype {
    Subtype("Swamp".to_owned())
}

fn walk(pred: Pred) -> Keyword {
    Keyword::Landwalk(Selector(vec![pred]))
}

/// Seat 0's board: two Bears and the Cloak, an Aura attached to the first.
fn board() -> GameState {
    let cloak = Arc::new(
        CardDefinition::new("Cloak")
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_rules(rules(CLOAK)),
    );
    let mut game = setup(
        vec![vec![], vec![]],
        vec![
            vec![creature("Bear", 2, 2), creature("Cub", 1, 1), cloak],
            vec![],
        ],
    );
    game.players[0].battlefield[2].attached_to = Some((0, 0));
    GameState::from_setup(&game).unwrap()
}

/// The Cloak entered with this choice.
fn entered(game: &mut GameState, chosen: Option<Chosen>) {
    let object = named(game, "Cloak").id;
    let event = Event::Entered {
        object,
        tapped: false,
        controller: ME,
        chosen,
    };
    apply_event(game, &event).unwrap();
}

fn walks(game: &GameState, name: &str) -> Vec<Keyword> {
    let id = named(game, name).id;
    keywords_of(game, id)
        .into_iter()
        .filter(|keyword| matches!(keyword, Keyword::Landwalk(_)))
        .cloned()
        .collect()
}

#[rstest]
#[case::a_land_type(Some(Chosen::LandType(swamp())), walk(Pred::Subtype(swamp())))]
#[case::a_creature_type_is_no_land_type(
    Some(Chosen::CreatureType(swamp())),
    walk(Pred::ChosenSubtype)
)]
#[case::a_color(Some(Chosen::Color(Color::Black)), walk(Pred::ChosenSubtype))]
#[case::nothing(None, walk(Pred::ChosenSubtype))]
fn the_landwalk_names_the_land_type_the_aura_chose(
    #[case] chosen: Option<Chosen>,
    #[case] expected: Keyword,
) {
    let mut game = board();
    entered(&mut game, chosen);
    assert_eq!(walks(&game, "Bear"), [expected]);
    assert_eq!(walks(&game, "Cub"), Vec::<Keyword>::new());
}

#[test]
fn the_landwalk_follows_the_aura_to_another_host_and_goes_with_it() {
    let mut game = board();
    entered(&mut game, Some(Chosen::LandType(swamp())));
    let (cloak, cub) = (named(&game, "Cloak").id, named(&game, "Cub").id);
    apply_event(
        &mut game,
        &Event::Attached {
            object: cloak,
            to: cub,
        },
    )
    .unwrap();
    assert_eq!(walks(&game, "Bear"), Vec::<Keyword>::new());
    assert_eq!(walks(&game, "Cub"), [walk(Pred::Subtype(swamp()))]);
    apply_event(
        &mut game,
        &Event::ZoneChanged {
            object: cloak,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Destroyed,
        },
    )
    .unwrap();
    assert_eq!(walks(&game, "Cub"), Vec::<Keyword>::new());
}

const TOY: &str = r#"{"Static":{"affected":"Attached","mods":["LoseAbilities",{"AddType":"Artifact"},{"AddType":"Creature"},{"AddSubtype":"Toy"},{"SetPT":{"power":0,"toughness":2}}]}}"#;

/// The Bear of seat 0 (a 2/3 with flying) with an Aura that makes it a Toy.
fn toyed() -> GameState {
    let aura = Arc::new(
        CardDefinition::new("Toy Maker")
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_rules(rules(TOY)),
    );
    let mut game = setup(vec![vec![], vec![]], vec![vec![bear(), aura], vec![]]);
    game.players[0].battlefield[1].attached_to = Some((0, 0));
    GameState::from_setup(&game).unwrap()
}

#[rstest]
#[case::alone(None, (0, 2), &[])]
#[case::a_later_size(Some(Modification::SetPowerToughness { power: 5, toughness: 5 }), (5, 5), &[])]
#[case::a_later_pump(Some(Modification::ModifyPowerToughness { power: 1, toughness: 1 }), (1, 3), &[])]
#[case::a_keyword_granted_after_the_loss(Some(Modification::AddKeyword(Keyword::Haste)), (0, 2), &[Keyword::Haste])]
fn an_aura_that_makes_its_creature_a_toy_takes_the_abilities_of_the_layers_before_a_later_effect(
    #[case] later: Option<Modification>,
    #[case] size: (i32, i32),
    #[case] keywords: &[Keyword],
) {
    let mut game = toyed();
    let plain = pump(&game, "Bear");
    assert_eq!(plain, Some((0, 2)));
    if let Some(later) = later {
        effect(&mut game, "Bear", later, 99);
    }
    assert_eq!(pump(&game, "Bear"), Some(size));
    let bear = named(&game, "Bear").id;
    let chars = characteristics(&game, bear).unwrap();
    assert!(chars.types.contains(CardType::Artifact) && chars.types.contains(CardType::Creature));
    assert!(chars.has_subtype(&[], &Subtype("Toy".to_owned())));
    let held: Vec<Keyword> = keywords_of(&game, bear).into_iter().cloned().collect();
    assert_eq!(held, keywords, "the printed flying is gone");
    // Only the abilities that were printed are lost: the trigger and the mana ability of the Bear.
    assert_eq!(abilities_of(&game, bear).len(), 0);
}

#[test]
fn a_creature_the_aura_left_has_its_own_abilities_and_size_again() {
    let mut game = toyed();
    let aura = named(&game, "Toy Maker").id;
    apply_event(
        &mut game,
        &Event::ZoneChanged {
            object: aura,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Destroyed,
        },
    )
    .unwrap();
    let bear = named(&game, "Bear").id;
    assert_eq!(pump(&game, "Bear"), Some((2, 3)));
    assert_eq!(keywords_of(&game, bear), [&Keyword::Flying]);
    assert!(abilities_of(&game, bear).len() > 1);
}
