//! `Effect::Conjure`: a player conjures cards by name into a zone (Alchemy).

use mtg_cards::effect::Effect;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::conjure::CardConjured;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The most cards one conjure makes, so an absurd count cannot stall a game.
const MOST: usize = 1000;

fn zone_of(kind: ZoneKind, player: PlayerId) -> Option<Zone> {
    match kind {
        ZoneKind::Hand => Some(Zone::Hand(player)),
        ZoneKind::Library => Some(Zone::Library(player)),
        ZoneKind::Graveyard => Some(Zone::Graveyard(player)),
        ZoneKind::Exile => Some(Zone::Exile),
        ZoneKind::Battlefield | ZoneKind::Stack | ZoneKind::Command => None,
    }
}

/// Each player named who is still in the game gets `count` new cards called `name` in the zone, taken from the cards the
/// game can conjure. The definition travels in the event, so a log folds with no card data. A name the game cannot
/// resolve, a zone that takes no new card and a count of nothing conjure nothing and leave the game going.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Conjure {
        who,
        name,
        zone,
        count,
    } = effect
    else {
        return Err(wrong_effect("Conjure"));
    };
    let state = cx.state();
    let Some(definition) = state.conjurable.get(name) else {
        return Ok(Run::events(Vec::new()));
    };
    let each = usize::try_from(cx.scope.amount(count)?.max(0))
        .unwrap_or(MOST)
        .min(MOST);
    let mut next = state.next_object().get();
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        let Some(place) = zone_of(*zone, player) else {
            continue;
        };
        for _ in 0..each {
            events.push(Event::CardConjured(CardConjured {
                object: ObjectId(next),
                owner: player,
                zone: place,
                name: definition.name.clone(),
                definition: std::sync::Arc::clone(definition),
            }));
            next += 1;
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a conjure creates the named card for each player named in the zone asked, as many as the count says and each a new object whose definition travels in the event; a name the game cannot resolve, a zone that takes no new card and a count of nothing conjure nothing and leave the game going; the log folds to the same state without any card data.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        let table = setup(
            vec![Vec::new(), Vec::new()],
            vec![vec![creature("Source", 1, 1)], Vec::new()],
        )
        .with_conjurable([creature("Bolt", 2, 1)]);
        GameState::from_setup(&table).unwrap()
    }

    fn events(game: &GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        match run(&cx, &effect).unwrap() {
            Run::Done { events, .. } => events,
            _ => panic!("a conjure asks nothing"),
        }
    }

    fn conjure(name: &str, zone: &str, extra: &str) -> String {
        format!(r#"{{"Conjure":{{"name":"{name}","zone":"{zone}"{extra}}}}}"#)
    }

    #[rstest]
    #[case::hand("Hand", Zone::Hand(ME))]
    #[case::library("Library", Zone::Library(ME))]
    #[case::graveyard("Graveyard", Zone::Graveyard(ME))]
    #[case::exile("Exile", Zone::Exile)]
    fn the_card_is_created_in_the_zone_and_the_log_folds_to_it(
        #[case] zone: &str,
        #[case] place: Zone,
    ) {
        let game = game();
        let made = events(&game, &conjure("Bolt", zone, ""));
        let [Event::CardConjured(card)] = made.as_slice() else {
            panic!("one card: {made:?}");
        };
        assert_eq!(
            (card.owner, card.zone, card.name.as_str()),
            (ME, place, "Bolt")
        );
        let mut folded = game.clone();
        crate::fold::apply_event(&mut folded, &made[0]).unwrap();
        let object = folded.object(card.object).unwrap();
        assert_eq!(object.card().name, "Bolt");
        assert_eq!(folded.zone(place).last(), Some(&card.object));
        assert_ne!(folded.state_hash(), game.state_hash());
        assert!(
            crate::fold::apply_event(&mut folded, &made[0]).is_err(),
            "an object is made once"
        );
    }

    #[rstest]
    #[case::three(r#","count":3"#, 3)]
    #[case::none(r#","count":0"#, 0)]
    #[case::every_player(r#","who":"EachPlayer""#, 2)]
    #[case::opponents(r#","who":"Opponents""#, 1)]
    fn the_count_and_the_players_say_how_many_cards(#[case] extra: &str, #[case] expected: usize) {
        let made = events(&game(), &conjure("Bolt", "Hand", extra));
        assert_eq!(made.len(), expected);
        let mut ids: Vec<_> = made
            .iter()
            .filter_map(|event| match event {
                Event::CardConjured(card) => Some(card.object),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), expected, "every card is a new object");
    }

    #[rstest]
    #[case::unknown_name(conjure("Nothing Of The Kind", "Hand", ""))]
    #[case::battlefield(conjure("Bolt", "Battlefield", ""))]
    #[case::stack(conjure("Bolt", "Stack", ""))]
    fn what_cannot_be_conjured_is_no_event(#[case] json: String) {
        assert!(events(&game(), &json).is_empty());
    }

    #[test]
    fn a_game_without_conjurable_cards_conjures_nothing() {
        let table = setup(
            vec![Vec::new(), Vec::new()],
            vec![vec![creature("Source", 1, 1)], Vec::new()],
        );
        assert!(table.conjurable.is_empty());
        let game = GameState::from_setup(&table).unwrap();
        assert!(events(&game, &conjure("Bolt", "Hand", "")).is_empty());
    }
}
