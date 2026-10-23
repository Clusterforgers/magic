//! `Effect::Search`: players search a zone for cards that match and find some of them (CR 701.23).

use mtg_cards::effect::Effect;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{Picked, Selection, Target, pick_selections};
use crate::events::Event;
use crate::library::{CardsFound, Revealed};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rules::cant_search;
use crate::wording;
use crate::zones::Zone;

/// The zone of `kind` that `player` searches: their own library, hand or graveyard, or exile, which is shared.
fn zone_of(kind: ZoneKind, player: PlayerId) -> Result<Zone, ResolveError> {
    match kind {
        ZoneKind::Library => Ok(Zone::Library(player)),
        ZoneKind::Hand => Ok(Zone::Hand(player)),
        ZoneKind::Graveyard => Ok(Zone::Graveyard(player)),
        ZoneKind::Exile => Ok(Zone::Exile),
        ZoneKind::Battlefield | ZoneKind::Stack | ZoneKind::Command => Err(ResolveError::Invalid(
            format!("a search of {kind:?} is not a search the rules know"),
        )),
    }
}

/// Each player the effect names who is still in the game, in turn order, searches their zone for the cards that match
/// the filter and chooses between `count.min` and `count.max` of them; the cards chosen of all the players are bound to
/// the name. The searcher alone is asked and sees the matches, so nobody else learns what the zone holds. A hidden zone
/// does not have to give up what it holds (CR 701.23b), so in a library or a hand the fewest is 0 and a player may find
/// none even when cards match; a public zone gives up as many as `count.min` asks. Nothing matching is no question.
/// Everyone chooses before anything is logged. Each search is one `CardsFound`, private to the searcher whether
/// or not a card was found, and the cards are revealed to every seat in one `Revealed` after it when the effect says so.
/// A player who can't search libraries does not search one.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Search {
        who,
        zone,
        filter,
        count,
        reveal,
        bind,
    } = effect
    else {
        return Err(wrong_effect("Search"));
    };
    let state = cx.state();
    let most = match &count.max {
        None => usize::MAX,
        Some(amount) => usize::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(usize::MAX),
    };
    let required = usize::try_from(cx.scope.amount(&count.min)?.max(0)).unwrap_or(usize::MAX);
    let fewest = if matches!(zone, ZoneKind::Library | ZoneKind::Hand) {
        0
    } else {
        required
    };
    let text = wording::search(*zone, filter, required, most);
    let mut searches: Vec<(PlayerId, Zone)> = Vec::new();
    let mut selections = Vec::new();
    for player in cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
    {
        // A player who can't search libraries does not search: no question and no event.
        if *zone == ZoneKind::Library && cant_search(state, player) {
            continue;
        }
        let place = zone_of(*zone, player)?;
        let mut matches: Vec<ObjectId> = Vec::new();
        for id in state.zone(place) {
            if let Some(card) = state.object(*id)
                && cx.scope.selects(filter, card.reference())?
            {
                matches.push(*id);
            }
        }
        // By id, which says nothing of where the cards lie in the library.
        matches.sort_unstable();
        searches.push((player, place));
        selections.push(Selection {
            player,
            from: matches,
            min: fewest,
            max: most.max(fewest),
            text: text.clone(),
        });
    }
    let taken = match pick_selections(cx, &selections)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(taken) => taken,
    };
    let mut events = Vec::new();
    let mut found: Vec<Target> = Vec::new();
    for ((player, place), (_, cards)) in searches.into_iter().zip(taken) {
        found.extend(
            cards
                .iter()
                .filter_map(|id| state.object(*id))
                .map(|card| Target::Object(card.reference())),
        );
        events.push(Event::CardsFound(CardsFound {
            player,
            zone: place,
            cards: cards.clone(),
        }));
        if *reveal && !cards.is_empty() {
            events.push(Event::Revealed(Revealed { player, cards }));
        }
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), found)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a search asks the searcher alone for the cards of the zone that match the filter, as many as the effect allows and, in a library or a hand, none at all; it logs one private `CardsFound` for each search whether or not a card was found, reveals the cards to every seat only when the effect says so, binds exactly the cards found, asks nothing when no card matches or the zone is empty, takes the cards a public zone must give up without asking, follows the players the effect names in turn order, and refuses an answer it did not ask for and a zone that is no zone to search.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::{CardType, Supertype};
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn basic(subtype: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(subtype)
                .with_type(CardType::Land)
                .with_supertype(Supertype::Basic)
                .with_subtype(subtype),
        )
    }

    fn dual(name: &str, subtypes: [&str; 2]) -> Arc<CardDefinition> {
        let [first, second] = subtypes;
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Land)
                .with_subtype(first)
                .with_subtype(second),
        )
    }

    /// Seat 0's library holds a Forest, an Island, a Forest, a Wolf and a dual land; seat 1's holds a Plains and a Wolf.
    /// The Source is on the battlefield of seat 0 and two Forests and an Elf are in its graveyard.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.players[0].library = vec![
            basic("Forest"),
            basic("Island"),
            basic("Forest"),
            creature("Wolf", 2, 2),
            dual("Stomping Ground", ["Mountain", "Forest"]),
        ];
        game.players[1].library = vec![basic("Plains"), creature("Wolf", 2, 2)];
        game.players[0].graveyard = vec![basic("Forest"), basic("Forest"), creature("Elf", 1, 1)];
        match GameState::from_setup(&game) {
            Ok(state) => state,
            Err(error) => panic!("the setup is valid: {error}"),
        }
    }

    fn search(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn name(game: &GameState, id: ObjectId) -> String {
        game.object(id).unwrap().card().name.clone()
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the search asks");
        };
        decision
    }

    fn done(run: Run) -> (Vec<Event>, Vec<Target>) {
        let Run::Done {
            events,
            mut bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the search is done");
        };
        assert!(snapshots.is_empty() && next == Next::Sibling && bound.len() == 1);
        (events, bound.remove(0).1)
    }

    fn options_of(decision: &Decision) -> &crate::choose::CardsOptions {
        let DecisionKind::ChooseCards(options) = &decision.kind else {
            panic!("the cards are chosen");
        };
        options
    }

    const BASIC_LAND: &str = r#"[{"Supertype":"Basic"},{"Type":"Land"}]"#;

    #[rstest]
    #[case::a_basic_land(BASIC_LAND, "", &["Forest", "Island", "Forest"], 1, "Search your library for a basic land card.")]
    #[case::up_to_two(BASIC_LAND, r#","count":{"min":0,"max":2}"#, &["Forest", "Island", "Forest"], 2, "Search your library for up to 2 basic land cards.")]
    #[case::any_number(BASIC_LAND, r#","count":{"min":0,"max":null}"#, &["Forest", "Island", "Forest"], 3, "Search your library for any number of basic land cards.")]
    #[case::a_land_type(r#"[{"AnyOf":[{"Subtype":"Forest"},{"Subtype":"Island"}]}]"#, "", &["Forest", "Island", "Forest", "Stomping Ground"], 1, "Search your library for a Forest or Island card.")]
    #[case::a_name(r#"[{"Named":"Wolf"}]"#, "", &["Wolf"], 1, "Search your library for a card named Wolf.")]
    #[case::any_card("[]", "", &["Forest", "Island", "Forest", "Wolf", "Stomping Ground"], 1, "Search your library for a card.")]
    fn the_searcher_is_asked_for_the_matches_alone_and_may_find_none(
        #[case] filter: &str,
        #[case] count: &str,
        #[case] names: &[&str],
        #[case] max: u32,
        #[case] text: &str,
    ) {
        let game = game();
        let effect = search(&format!(
            r#"{{"Search":{{"filter":{filter}{count},"as":"found"}}}}"#
        ));
        let decision = asked(run_with(&game, Vec::new(), &effect).unwrap());
        let options = options_of(&decision);
        let mut got: Vec<String> = options.from.iter().map(|id| name(&game, *id)).collect();
        got.sort();
        let mut want: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
        want.sort();
        assert_eq!(got, want);
        assert_eq!(
            (
                decision.player,
                options.min,
                options.max,
                options.text.as_str()
            ),
            (ME, 0, max, text)
        );
        assert!(
            options.from.windows(2).all(|pair| pair[0] < pair[1]),
            "the cards are listed by id, which says nothing of their places in the library"
        );
    }

    #[rstest]
    #[case::none(0, false)]
    #[case::one(1, false)]
    #[case::two_revealed(2, true)]
    #[case::one_revealed(1, true)]
    fn the_cards_chosen_are_found_privately_revealed_when_the_effect_says_and_bound(
        #[case] chosen: usize,
        #[case] reveal: bool,
    ) {
        let game = game();
        let reveal_json = if reveal { r#","reveal":true"# } else { "" };
        let effect = search(&format!(
            r#"{{"Search":{{"filter":{BASIC_LAND},"count":{{"min":0,"max":2}}{reveal_json},"as":"found"}}}}"#
        ));
        let decision = asked(run_with(&game, Vec::new(), &effect).unwrap());
        let cards: Vec<ObjectId> = options_of(&decision).from[..chosen].to_vec();
        let journal = vec![(ME, Choice::Cards(cards.clone()))];
        let (events, found) = done(run_with(&game, journal, &effect).unwrap());
        let mut expected = vec![Event::CardsFound(CardsFound {
            player: ME,
            zone: Zone::Library(ME),
            cards: cards.clone(),
        })];
        if reveal && chosen > 0 {
            expected.push(Event::Revealed(Revealed {
                player: ME,
                cards: cards.clone(),
            }));
        }
        assert_eq!(events, expected);
        let bound: Vec<ObjectId> = found.iter().map(|t| t.object().unwrap().id).collect();
        assert_eq!(bound, cards);
    }

    #[rstest]
    #[case::no_match(r#"[{"Named":"Dragon"}]"#)]
    #[case::the_wrong_zone(r#"[{"In":"Graveyard"}]"#)]
    fn nothing_that_matches_is_no_question_but_the_search_still_happened(#[case] filter: &str) {
        let game = game();
        let effect = search(&format!(
            r#"{{"Search":{{"filter":{filter},"reveal":true,"as":"found"}}}}"#
        ));
        let (events, found) = done(run_with(&game, Vec::new(), &effect).unwrap());
        assert_eq!(
            events,
            [Event::CardsFound(CardsFound {
                player: ME,
                zone: Zone::Library(ME),
                cards: Vec::new()
            })]
        );
        assert_eq!(found, []);
    }

    #[test]
    fn an_empty_library_is_searched_and_nothing_is_found() {
        let mut empty = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        empty.players[0].library.clear();
        let game = GameState::from_setup(&empty).unwrap();
        let effect = search(&format!(
            r#"{{"Search":{{"filter":{BASIC_LAND},"as":"found"}}}}"#
        ));
        let (events, found) = done(run_with(&game, Vec::new(), &effect).unwrap());
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::CardsFound(f) if f.cards.is_empty()));
        assert_eq!(found, []);
    }

    #[test]
    fn each_player_the_effect_names_searches_their_own_library_in_turn_order() {
        let game = game();
        let effect = search(&format!(
            r#"{{"Search":{{"who":"EachPlayer","filter":{BASIC_LAND},"as":"found"}}}}"#
        ));
        let first = asked(run_with(&game, Vec::new(), &effect).unwrap());
        assert_eq!(first.player, ME);
        let journal = vec![(ME, Choice::Cards(vec![options_of(&first).from[0]]))];
        let second = asked(run_with(&game, journal.clone(), &effect).unwrap());
        let theirs = options_of(&second);
        assert_eq!(second.player, FOE);
        assert_eq!(
            theirs
                .from
                .iter()
                .map(|id| name(&game, *id))
                .collect::<Vec<_>>(),
            ["Plains"]
        );
        let journal = [journal, vec![(FOE, Choice::Cards(theirs.from.clone()))]].concat();
        let (events, found) = done(run_with(&game, journal, &effect).unwrap());
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[1], Event::CardsFound(f) if f.player == FOE && f.zone == Zone::Library(FOE))
        );
        assert_eq!(
            found.len(),
            2,
            "the cards of both players are bound together"
        );
    }

    /// The graveyard of seat 0 holds two Forests and an Elf.
    #[rstest]
    #[case::one_of_two_lands_is_asked_and_must_be_found(r#"[{"Type":"Land"}]"#, 1, 1, Some((1, 1)), 0)]
    #[case::up_to_two_of_two_lands_is_asked_and_may_be_none(r#"[{"Type":"Land"}]"#, 0, 2, Some((0, 2)), 0)]
    #[case::exactly_the_two_lands_are_taken(r#"[{"Type":"Land"}]"#, 2, 2, None, 2)]
    #[case::fewer_than_asked_is_all_there_is(r#"[{"Type":"Creature"}]"#, 2, 2, None, 1)]
    fn a_public_zone_gives_up_the_cards_it_must(
        #[case] filter: &str,
        #[case] min: u32,
        #[case] max: u32,
        #[case] asked_for: Option<(u32, u32)>,
        #[case] taken: usize,
    ) {
        let game = game();
        let effect = search(&format!(
            r#"{{"Search":{{"zone":"Graveyard","filter":{filter},"count":{{"min":{min},"max":{max}}},"as":"found"}}}}"#
        ));
        let result = run_with(&game, Vec::new(), &effect).unwrap();
        if let Some(bounds) = asked_for {
            let decision = asked(result);
            let options = options_of(&decision);
            assert_eq!((options.min, options.max), bounds);
            assert_eq!(options.from.len(), 2);
        } else {
            let (events, found) = done(result);
            assert_eq!(found.len(), taken);
            assert!(
                matches!(&events[0], Event::CardsFound(f) if f.zone == Zone::Graveyard(ME) && f.cards.len() == taken)
            );
        }
    }

    #[test]
    fn answers_the_search_did_not_ask_for_are_a_bug() {
        let game = game();
        let effect = search(&format!(
            r#"{{"Search":{{"filter":{BASIC_LAND},"as":"found"}}}}"#
        ));
        let decision = asked(run_with(&game, Vec::new(), &effect).unwrap());
        let from = options_of(&decision).from.clone();
        let wolf = game
            .zone(Zone::Library(ME))
            .iter()
            .copied()
            .find(|id| name(&game, *id) == "Wolf")
            .unwrap();
        for journal in [
            vec![(ME, Choice::Cards(vec![wolf]))],
            vec![(FOE, Choice::Cards(vec![from[0]]))],
            vec![(ME, Choice::Cards(from[..2].to_vec()))],
            vec![(ME, Choice::Order(vec![from[0]]))],
            vec![
                (ME, Choice::Cards(vec![from[0]])),
                (ME, Choice::Cards(vec![from[1]])),
            ],
        ] {
            let refused = run_with(&game, journal.clone(), &effect);
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
    }

    #[rstest]
    #[case::the_battlefield("Battlefield")]
    #[case::the_stack("Stack")]
    #[case::the_command_zone("Command")]
    fn a_zone_that_is_no_place_to_search_is_refused(#[case] zone: &str) {
        let game = game();
        let effect = search(&format!(
            r#"{{"Search":{{"zone":"{zone}","filter":[],"as":"found"}}}}"#
        ));
        let refused = run_with(&game, Vec::new(), &effect);
        assert!(
            matches!(refused, Err(ResolveError::Invalid(_))),
            "{refused:?}"
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let refused = run_with(&game, Vec::new(), &search(r#"{"Draw":{}}"#));
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
