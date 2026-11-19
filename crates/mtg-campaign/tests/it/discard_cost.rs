//! Goal: an event's "give up a card" cost takes a Satchel card first, else a copy of the deck's library that the hero
//! has not already given up; a card of neither is refused and leaves the hero and the offer as they were.

use std::collections::BTreeMap;

use mtg_campaign::run::CONTENT;
use mtg_campaign::{DeckMakeup, HeroDeck, Offer, Pick, RoomKind, Run, RunEvent, Status, content};
use rstest::rstest;

/// A run of one hero at a room whose deck holds one Sol Ring and two Rancor beside a Satchel of one Llanowar Elves,
/// with `lost` already given up; `library` says whether the deck's card names were kept.
fn at_discard(library: bool, lost: &[&str]) -> Run {
    let content = content().unwrap();
    let makeup = DeckMakeup {
        library: if library {
            [("Sol Ring".to_owned(), 1), ("Rancor".to_owned(), 2)].into()
        } else {
            BTreeMap::new()
        },
        ..DeckMakeup::default()
    };
    let events = [
        RunEvent::Content { version: CONTENT },
        RunEvent::Joined {
            name: "Ana".into(),
            user: Some("u1".into()),
            bot: None,
        },
        RunEvent::DeckChosen {
            hero: 0,
            deck: HeroDeck {
                id: "precon:x".into(),
                name: "Deck".into(),
                colors: "RG".into(),
                commanders: Vec::new(),
                makeup: Some(makeup),
            },
        },
        RunEvent::Started,
    ];
    let mut run = Run::replay(content, "warrens", 3, 0, &events).unwrap();
    run.status = Status::Room;
    run.at = run
        .map
        .nodes
        .iter()
        .find(|n| n.kind != RoomKind::Boss)
        .map(|n| n.id);
    let hero = &mut run.heroes[0];
    hero.satchel = vec!["Llanowar Elves".into()];
    hero.lost = lost.iter().map(|card| (*card).to_owned()).collect();
    hero.offers = vec![Offer::Discard];
    run
}

fn answer(run: &mut Run, card: &str) -> Result<(), String> {
    let content = content().unwrap();
    let event = RunEvent::Picked {
        hero: 0,
        pick: Pick::Discard(card.into()),
    };
    run.accepts(content, &event)
        .and_then(|()| run.apply(content, &event))
        .map_err(|e| e.to_string())
}

#[rstest]
#[case::satchel_card(true, &[], "Llanowar Elves", true, &[], 0)]
#[case::deck_card(true, &[], "Sol Ring", true, &["Sol Ring"], 1)]
#[case::second_copy(true, &["Rancor"], "Rancor", true, &["Rancor", "Rancor"], 1)]
#[case::unknown_card(true, &[], "Black Lotus", false, &[], 1)]
#[case::already_lost_card(true, &["Sol Ring"], "Sol Ring", false, &["Sol Ring"], 1)]
#[case::every_copy_lost(true, &["Rancor", "Rancor"], "Rancor", false, &["Rancor", "Rancor"], 1)]
#[case::names_not_kept(false, &[], "Black Lotus", true, &["Black Lotus"], 1)]
fn a_card_given_up_is_in_the_satchel_or_the_deck(
    #[case] library: bool,
    #[case] lost: &[&str],
    #[case] card: &str,
    #[case] accepted: bool,
    #[case] lost_after: &[&str],
    #[case] satchel_after: usize,
) {
    let mut run = at_discard(library, lost);
    let result = answer(&mut run, card);
    let hero = &run.heroes[0];
    assert_eq!(result.is_ok(), accepted, "{result:?}");
    assert_eq!(hero.lost, lost_after);
    assert_eq!(hero.satchel.len(), satchel_after);
    // A refused card leaves the offer waiting; an accepted one answers it.
    assert_eq!(hero.offers.len(), usize::from(!accepted));
    if let Err(refusal) = result {
        assert!(refusal.contains("not in your deck or Satchel"), "{refusal}");
    }
}
