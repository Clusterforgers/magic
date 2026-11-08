//! Goal: a static ability that makes objects "the chosen type in addition to their other types" or "the chosen color" adds the creature type or sets the colour its source chose as it entered, for every object it affects at any table size, and changes nothing while the source chose nothing or chose another kind of value.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::tests::rules;
use crate::characteristics::characteristics;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::object::Chosen;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// An artifact whose static ability gives every creature the modification.
fn totem(modification: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}}]}},"mods":["{modification}"]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Totem")
            .with_type(CardType::Artifact)
            .with_rules(rules(&ability)),
    )
}

fn rat() -> Subtype {
    Subtype("Rat".to_owned())
}

#[rstest]
#[case::a_type_at_two_seats(2, "AddChosenSubtype", Some(Chosen::CreatureType(rat())), true, None)]
#[case::a_type_at_four_seats(4, "AddChosenSubtype", Some(Chosen::CreatureType(rat())), true, None)]
#[case::no_type_chosen(2, "AddChosenSubtype", None, false, None)]
#[case::a_colour_is_no_type(2, "AddChosenSubtype", Some(Chosen::Color(Color::Blue)), false, None)]
#[case::a_colour_at_three_seats(
    3,
    "SetChosenColor",
    Some(Chosen::Color(Color::Blue)),
    false,
    Some(Color::Blue)
)]
#[case::no_colour_chosen(2, "SetChosenColor", None, false, None)]
#[case::a_type_is_no_colour(2, "SetChosenColor", Some(Chosen::CreatureType(rat())), false, None)]
fn the_static_reads_what_its_source_chose(
    #[case] seats: usize,
    #[case] modification: &str,
    #[case] chosen: Option<Chosen>,
    #[case] is_rat: bool,
    #[case] color: Option<Color>,
) {
    let mut battlefields = vec![vec![totem(modification), creature("Bear", 2, 2)]];
    battlefields.extend((1..seats).map(|_| vec![creature("Foe", 1, 1)]));
    let mut game = state(vec![vec![]; seats], battlefields);
    let on = game.zone(Zone::Battlefield).to_vec();
    let printed: Vec<ColorSet> = on
        .iter()
        .map(|id| characteristics(&game, *id).unwrap().colors)
        .collect();
    apply_event(
        &mut game,
        &Event::Entered {
            object: on[0],
            tapped: false,
            controller: ME,
            chosen,
        },
    )
    .unwrap();
    // Every creature, the opponents' too, and never the artifact itself.
    for (index, id) in on.iter().enumerate() {
        let chars = characteristics(&game, *id).unwrap();
        let creature = index > 0;
        assert_eq!(
            chars.subtypes.contains(&rat()),
            creature && is_rat,
            "{chars:?}"
        );
        let wanted = match color {
            Some(color) if creature => ColorSet::EMPTY.with(color),
            _ => printed[index],
        };
        assert_eq!(chars.colors, wanted, "{chars:?}");
    }
}
