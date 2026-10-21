//! `Effect::Sacrifice`: players sacrifice permanents they control (CR 701.21).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::controller_of;
use crate::choose::{Chooser, Picked, pick_cards};
use crate::events::{Event, ZoneCause};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::wording;
use crate::zones::Zone;

/// Each player the effect names who is still in the game sacrifices the number of permanents they control that match
/// the filter, in turn order. The player chooses which and is asked only when more match than they sacrifice; if no
/// more match, those are sacrificed, and with none nothing happens. Every sacrifice is chosen before any is made, then
/// the permanents go to their owners' graveyards together (`ZoneCause::Sacrificed`) and the step carries what they were.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Sacrifice { who, filter, count } = effect else {
        return Err(wrong_effect("Sacrifice"));
    };
    let take = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let matching = cx.scope.matching(filter)?;
    let choosers: Vec<Chooser> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .map(|player| Chooser {
            player,
            from: matching
                .iter()
                .filter(|permanent| {
                    state.object(permanent.id).is_some_and(|object| {
                        object.zone() == Zone::Battlefield
                            && controller_of(state, permanent.id) == Some(player)
                    })
                })
                .map(|permanent| permanent.id)
                .collect(),
            take,
            text: wording::sacrifice(filter, take),
        })
        .collect();
    let chosen = match pick_cards(cx, &choosers)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(chosen) => chosen,
    };
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for object in chosen.into_iter().flat_map(|(_, cards)| cards) {
        let Some(owner) = state.object(object).map(crate::object::GameObject::owner) else {
            continue;
        };
        snapshots.extend(snapshot(state, object));
        events.push(Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Sacrificed,
        });
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a sacrifice takes the number of permanents from each player it names in turn order, only from the permanents that player controls and that match, the player chooses and is asked only when more match than they sacrifice, none matching is no event, the question carries the words of the card, and the permanents go to their owners' graveyards with the cause and as they were.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn permanent(name: &str, card_type: CardType) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(card_type))
    }

    /// Seat 0 controls the Source, a Wolf and a Relic. Seat 1 controls a Bear, an Ogre, a Walker and a Relic.
    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    permanent("Mine", CardType::Artifact),
                ],
                vec![
                    creature("Bear", 2, 2),
                    creature("Ogre", 3, 4),
                    permanent("Walker", CardType::Planeswalker),
                    permanent("Theirs", CardType::Artifact),
                ],
            ],
        )
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectId {
        game.zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| game.object(*id).is_some_and(|o| o.card().name == name))
            .unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(named(game, "Source")).unwrap();
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

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    /// The sacrifices a finished run proposes, as (permanent, where it goes), and the objects its snapshots describe.
    fn sacrifices(run: Run) -> (Vec<(ObjectId, Zone)>, Vec<ObjectId>) {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the effect is done");
        };
        assert!(bound.is_empty() && next == Next::Sibling);
        let moves = events
            .into_iter()
            .map(|event| match event {
                Event::ZoneChanged {
                    object,
                    from: Zone::Battlefield,
                    to,
                    cause: ZoneCause::Sacrificed,
                } => (object, to),
                other => panic!("only sacrifices were expected, got {other:?}"),
            })
            .collect();
        (moves, snapshots.into_iter().map(|shot| shot.id).collect())
    }

    const CREATURE_OR_WALKER: &str = r#"[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#;

    #[test]
    fn the_player_is_asked_among_their_own_permanents_that_match() {
        let game = game();
        let sacrifice = effect(&format!(
            r#"{{"Sacrifice":{{"who":"Opponents","filter":{CREATURE_OR_WALKER}}}}}"#
        ));
        let decision = asked(run_with(&game, Vec::new(), &sacrifice).unwrap());
        assert_eq!(
            decision.player, FOE,
            "the opponent chooses, not the controller"
        );
        assert_eq!(
            decision.kind,
            DecisionKind::ChooseCards(CardsOptions {
                source: named(&game, "Source"),
                from: vec![
                    named(&game, "Bear"),
                    named(&game, "Ogre"),
                    named(&game, "Walker")
                ],
                min: 1,
                max: 1,
                text: "Sacrifice a creature or planeswalker.".into(),
                can_cancel: false,
            })
        );
    }

    #[test]
    fn the_answer_decides_which_permanent_goes_and_it_is_kept_as_it_was() {
        let game = game();
        let ogre = named(&game, "Ogre");
        let sacrifice = effect(&format!(
            r#"{{"Sacrifice":{{"who":"Opponents","filter":{CREATURE_OR_WALKER}}}}}"#
        ));
        let run = run_with(&game, vec![(FOE, Choice::Cards(vec![ogre]))], &sacrifice).unwrap();
        let Run::Done { snapshots, .. } = &run else {
            panic!("the effect is done");
        };
        // The snapshot is the Ogre as it was, on the battlefield.
        assert_eq!(snapshots[0].zone, Zone::Battlefield);
        assert_eq!(snapshots[0].characteristics.toughness, Some(4));
        assert_eq!(
            sacrifices(run),
            (vec![(ogre, Zone::Graveyard(FOE))], vec![ogre])
        );
    }

    #[rstest]
    #[case::the_only_one_that_matches(r#"[{"Type":"Planeswalker"}]"#, "Walker")]
    #[case::the_only_artifact(r#"[{"Type":"Artifact"}]"#, "Theirs")]
    fn a_single_match_is_sacrificed_without_a_question(#[case] filter: &str, #[case] name: &str) {
        let game = game();
        let sacrifice = effect(&format!(
            r#"{{"Sacrifice":{{"who":"Opponents","filter":{filter}}}}}"#
        ));
        let run = run_with(&game, Vec::new(), &sacrifice).unwrap();
        let object = named(&game, name);
        assert_eq!(
            sacrifices(run),
            (vec![(object, Zone::Graveyard(FOE))], vec![object])
        );
    }

    #[rstest]
    #[case::no_permanent_of_that_kind(r#"[{"Type":"Land"}]"#)]
    #[case::only_the_controllers_own_match(r#"[{"Named":"Wolf"}]"#)]
    #[case::a_zone_that_is_not_the_battlefield(r#"[{"In":"Graveyard"}]"#)]
    fn with_nothing_that_matches_nothing_happens(#[case] filter: &str) {
        let game = game();
        let sacrifice = effect(&format!(
            r#"{{"Sacrifice":{{"who":"Opponents","filter":{filter}}}}}"#
        ));
        let run = run_with(&game, Vec::new(), &sacrifice).unwrap();
        assert_eq!(sacrifices(run), (Vec::new(), Vec::new()));
    }

    #[test]
    fn a_player_sacrifices_only_what_they_control() {
        let game = game();
        // Seat 0 controls two creatures, so both go; the opponent's creatures never count.
        let sacrifice = effect(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":2}}"#);
        let run = run_with(&game, Vec::new(), &sacrifice).unwrap();
        assert_eq!(
            sacrifices(run).0,
            [
                (named(&game, "Source"), Zone::Graveyard(ME)),
                (named(&game, "Wolf"), Zone::Graveyard(ME))
            ]
        );
    }

    #[rstest]
    #[case::two_candidates_for_one(r#"[{"Type":"Creature"}]"#, 1, Some((1, "Sacrifice a creature.")))]
    #[case::three_candidates_for_two(
        r#"[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#,
        2,
        Some((2, "Sacrifice 2 creatures or planeswalkers."))
    )]
    #[case::two_candidates_for_two(r#"[{"Type":"Creature"}]"#, 2, None)]
    #[case::two_candidates_for_more(r#"[{"Type":"Creature"}]"#, 5, None)]
    fn the_count_decides_whether_a_question_is_asked_and_how_many_it_wants(
        #[case] filter: &str,
        #[case] count: u32,
        #[case] question: Option<(u32, &str)>,
    ) {
        let game = game();
        let sacrifice = effect(&format!(
            r#"{{"Sacrifice":{{"who":"Opponents","filter":{filter},"count":{count}}}}}"#
        ));
        match (run_with(&game, Vec::new(), &sacrifice).unwrap(), question) {
            (Run::Ask(decision), Some((wanted, text))) => {
                let DecisionKind::ChooseCards(options) = decision.kind else {
                    panic!("cards are chosen");
                };
                assert_eq!(
                    (options.min, options.max, options.text.as_str()),
                    (wanted, wanted, text)
                );
            }
            (run @ Run::Done { .. }, None) => {
                let (moves, shots) = sacrifices(run);
                assert_eq!(moves.len(), 2, "both creatures go");
                assert_eq!(shots, [named(&game, "Bear"), named(&game, "Ogre")]);
            }
            (other, wanted) => panic!("{other:?} but wanted {wanted:?}"),
        }
    }

    #[test]
    fn each_player_chooses_first_and_the_permanents_go_together() {
        let game = game();
        let each = effect(r#"{"Sacrifice":{"who":"EachPlayer","filter":[{"Type":"Creature"}]}}"#);
        // Seat 0 controls two creatures and is asked first, then seat 1 with two creatures.
        let first = asked(run_with(&game, Vec::new(), &each).unwrap());
        assert_eq!(first.player, ME);
        let (wolf, bear) = (named(&game, "Wolf"), named(&game, "Bear"));
        let second = asked(run_with(&game, vec![(ME, Choice::Cards(vec![wolf]))], &each).unwrap());
        assert_eq!(second.player, FOE);
        let run = run_with(
            &game,
            vec![
                (ME, Choice::Cards(vec![wolf])),
                (FOE, Choice::Cards(vec![bear])),
            ],
            &each,
        )
        .unwrap();
        assert_eq!(
            sacrifices(run).0,
            [(wolf, Zone::Graveyard(ME)), (bear, Zone::Graveyard(FOE))]
        );
    }

    #[test]
    fn a_player_who_has_left_the_game_sacrifices_nothing() {
        let mut game = game();
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let sacrifice = effect(r#"{"Sacrifice":{"who":"Opponents","filter":[]}}"#);
        // The player is out, so they are not asked and sacrifice nothing.
        let run = run_with(&game, Vec::new(), &sacrifice).unwrap();
        assert_eq!(sacrifices(run), (Vec::new(), Vec::new()));
    }

    #[rstest]
    #[case::a_permanent_of_another_player("Wolf")]
    #[case::a_permanent_that_does_not_match("Theirs")]
    fn an_answer_outside_the_candidates_is_a_bug(#[case] name: &str) {
        let game = game();
        let sacrifice = effect(&format!(
            r#"{{"Sacrifice":{{"who":"Opponents","filter":{CREATURE_OR_WALKER}}}}}"#
        ));
        let refused = run_with(
            &game,
            vec![(FOE, Choice::Cards(vec![named(&game, name)]))],
            &sacrifice,
        );
        assert!(
            matches!(refused, Err(ResolveError::Invalid(_))),
            "{refused:?}"
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let refused = run_with(&game, Vec::new(), &effect(r#"{"Draw":{}}"#));
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
