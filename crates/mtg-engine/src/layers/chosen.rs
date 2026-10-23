//! Goal: protection a static ability gives "from the chosen color" or "from creatures of the chosen type" is fixed by what the source of the ability chose as it entered, whoever holds it, and stays as written while the source chose nothing.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::tests::rules;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::keywords::keywords_of;
use crate::object::Chosen;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn totem(quality: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}}]}},"mods":[{{"AddKeyword":{{"Protection":{quality}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Totem")
            .with_type(CardType::Artifact)
            .with_rules(rules(&ability)),
    )
}

#[rstest]
#[case::a_color(
    r#"["ChosenColor"]"#,
    Some(Chosen::Color(Color::Blue)),
    vec![Pred::Color(Color::Blue)]
)]
#[case::a_creature_type(
    r#"[{"Type":"Creature"},"ChosenSubtype"]"#,
    Some(Chosen::CreatureType(Subtype("Rat".to_owned()))),
    vec![Pred::Type(CardType::Creature), Pred::Subtype(Subtype("Rat".to_owned()))]
)]
#[case::nothing_chosen(r#"["ChosenColor"]"#, None, vec![Pred::ChosenColor])]
fn a_granted_protection_names_what_its_source_chose(
    #[case] quality: &str,
    #[case] chosen: Option<Chosen>,
    #[case] expected: Vec<Pred>,
) {
    let mut game = state(
        vec![vec![], vec![]],
        vec![
            vec![totem(quality), creature("Bear", 2, 2)],
            vec![creature("Foe", 1, 1)],
        ],
    );
    let on = game.zone(Zone::Battlefield).to_vec();
    let entered = Event::Entered {
        object: on[0],
        tapped: false,
        controller: ME,
        chosen,
    };
    apply_event(&mut game, &entered).unwrap();
    // Every creature has it, the opponent's too: the holder made no choice of its own.
    for holder in [on[1], on[2]] {
        assert!(
            keywords_of(&game, holder)
                .iter()
                .any(|held| **held == Keyword::Protection(Selector(expected.clone()))),
            "{:?}",
            keywords_of(&game, holder)
        );
    }
}
