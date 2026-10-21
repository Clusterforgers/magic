//! The legend rule (CR 704.5j): a player who controls two or more legendary permanents with the same name chooses one
//! and puts the rest into their owners' graveyards. The choice is a decision of the state-based action pass.

use std::collections::BTreeMap;

use mtg_core::card::Supertype;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::controller_of_object;
use crate::choose::{CardsChoice, CardsOptions, check_cards};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The legendary permanents one player controls that share a name, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    /// The player who controls them and chooses.
    pub(crate) player: PlayerId,
    /// The name they share.
    pub(crate) name: String,
    /// The permanents, by id.
    pub(crate) permanents: Vec<ObjectId>,
}

/// Every group the rule applies to: by player in turn order from the active player, then by name.
pub(crate) fn groups(state: &GameState) -> Vec<Group> {
    if !state.possible.legends {
        return Vec::new();
    }
    let mut found: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if !object.card().supertypes.contains(&Supertype::Legendary) {
            continue;
        }
        let controller = controller_of_object(state, object);
        if state.is_alive(controller) {
            found
                .entry((controller, object.card().name.clone()))
                .or_default()
                .push(id);
        }
    }
    let order = state.turn_order_from(state.turn().active());
    let place = |player: PlayerId| order.iter().position(|seat| *seat == player);
    let mut groups: Vec<Group> = found
        .into_iter()
        .filter(|(_, permanents)| permanents.len() > 1)
        .map(|((player, name), mut permanents)| {
            permanents.sort_unstable();
            Group {
                player,
                name,
                permanents,
            }
        })
        .collect();
    groups.sort_by_key(|group| place(group.player));
    groups
}

/// The question the rule asks now: the first group's controller chooses the one to keep. `None` when no player controls two
/// legendary permanents with one name.
pub(crate) fn decision(state: &GameState) -> Option<Decision> {
    let group = groups(state).into_iter().next()?;
    let source = *group.permanents.first()?;
    Some(Decision {
        player: group.player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            source,
            from: group.permanents,
            min: 1,
            max: 1,
            text: format!("Legend rule: choose the {} to keep.", group.name),
            can_cancel: false,
        }),
    })
}

/// The answer to that question: the permanents not chosen are put into their owners' graveyards, all together. They are
/// not destroyed.
pub(crate) fn answer(
    state: &GameState,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    let events = options
        .from
        .iter()
        .filter(|id| !choice.0.contains(id))
        .filter_map(|id| state.object(*id))
        .map(|object| Event::ZoneChanged {
            object: object.id(),
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::StateBased,
        })
        .collect();
    Ok(Outcome::Events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a group is two or more legendary permanents of one name that one player controls, found for each player and name and ordered by turn order from the active player and then by name; copies of other players, a single legend, permanents that are not legendary and players who left make none; the question asks the first group's controller for one permanent among them; and the answer puts every other one into its owner's graveyard as a state-based action, refusing an answer that does not name exactly one of them.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::error::IllegalAnswer;
    use crate::fixtures::setup;
    use crate::setup::Start;
    use crate::turn::Step;

    fn legend(name: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_supertype(Supertype::Legendary)
                .with_power_toughness(2, 2),
        )
    }

    fn plain(name: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2),
        )
    }

    /// A game of as many seats as `boards` where the first turn is `active`'s, each seat holding the cards named in its
    /// string (a name that starts with Plain is not legendary).
    fn game(boards: &[&str], active: usize) -> GameState {
        let cards = |names: &str| -> Vec<Arc<CardDefinition>> {
            names
                .split_whitespace()
                .map(|name| {
                    if name.starts_with("Plain") {
                        plain(name)
                    } else {
                        legend(name)
                    }
                })
                .collect()
        };
        let mut game = setup(
            boards.iter().map(|_| Vec::new()).collect(),
            boards.iter().map(|names| cards(names)).collect(),
        );
        game.start = Start::At {
            turn: 1,
            active: PlayerId::from_index(active),
            step: Step::Main1,
        };
        GameState::from_setup(&game).unwrap()
    }

    /// (controller seat, name, how many) for each group.
    fn found(game: &GameState) -> Vec<(usize, String, usize)> {
        groups(game)
            .into_iter()
            .map(|group| (group.player.index(), group.name, group.permanents.len()))
            .collect()
    }

    #[rstest]
    #[case::nothing(&["", ""], 0, &[])]
    #[case::one_legend(&["Ana", ""], 0, &[])]
    #[case::two_copies(&["Ana Ana", ""], 0, &[(0, "Ana", 2)])]
    #[case::four_copies(&["Ana Ana Ana Ana", ""], 0, &[(0, "Ana", 4)])]
    #[case::copies_of_other_players_do_not_count(&["Ana", "Ana"], 0, &[])]
    #[case::two_names_by_name(&["Cy Ana Cy Ana", ""], 0, &[(0, "Ana", 2), (0, "Cy", 2)])]
    #[case::permanents_that_are_not_legendary_make_none(&["Plain Plain", ""], 0, &[])]
    #[case::in_turn_order_from_the_active_player(
        &["Ana Ana", "Ana Ana", "Bo Bo"],
        1,
        &[(1, "Ana", 2), (2, "Bo", 2), (0, "Ana", 2)]
    )]
    fn the_groups_are_the_names_a_player_controls_twice(
        #[case] boards: &[&str],
        #[case] active: usize,
        #[case] expected: &[(usize, &str, usize)],
    ) {
        let game = game(boards, active);
        let expected: Vec<(usize, String, usize)> = expected
            .iter()
            .map(|(seat, name, count)| (*seat, (*name).to_owned(), *count))
            .collect();
        assert_eq!(found(&game), expected);
    }

    #[test]
    fn a_player_who_left_has_no_group() {
        let mut game = game(&["Ana Ana", "Bo Bo"], 0);
        crate::fold::apply_event(
            &mut game,
            &crate::events::Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        assert_eq!(found(&game), [(0, "Ana".to_owned(), 2)]);
    }

    #[test]
    fn the_question_asks_the_controller_of_the_first_group_for_one_of_its_permanents() {
        let game = game(&["Ana", "Bo Bo Bo"], 0);
        let Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        } = decision(&game).unwrap()
        else {
            panic!("a choice of cards");
        };
        assert_eq!(player, PlayerId(1));
        assert_eq!((options.min, options.max), (1, 1));
        assert_eq!(options.from.len(), 3);
        assert_eq!(options.source, options.from[0]);
        assert_eq!(options.text, "Legend rule: choose the Bo to keep.");
        assert!(decision(&self::game(&["Ana", "Ana"], 0)).is_none());
    }

    #[rstest]
    #[case::keep_the_first(0)]
    #[case::keep_the_middle(1)]
    #[case::keep_the_last(2)]
    fn the_answer_puts_every_other_permanent_into_its_owners_graveyard(#[case] kept: usize) {
        let game = game(&["Ana Ana Ana", ""], 0);
        let Some(Decision {
            kind: DecisionKind::ChooseCards(options),
            ..
        }) = decision(&game)
        else {
            panic!("the rule asks");
        };
        let Ok(Outcome::Events(events)) =
            answer(&game, &options, &CardsChoice(vec![options.from[kept]]))
        else {
            panic!("a legal answer is events");
        };
        let expected: Vec<Event> = options
            .from
            .iter()
            .copied()
            .filter(|id| *id != options.from[kept])
            .map(|object| Event::ZoneChanged {
                object,
                from: Zone::Battlefield,
                to: Zone::Graveyard(PlayerId(0)),
                cause: ZoneCause::StateBased,
            })
            .collect();
        assert_eq!(events, expected);
    }

    #[rstest]
    #[case::none(vec![], IllegalAnswer::WrongCount { expected: 1, got: 0 })]
    #[case::two(vec![0, 1], IllegalAnswer::WrongCount { expected: 1, got: 2 })]
    #[case::the_same_twice(vec![0, 0], IllegalAnswer::Duplicate("card"))]
    #[case::a_card_that_is_not_there(vec![99], IllegalAnswer::NotOffered("card"))]
    fn an_answer_that_does_not_name_exactly_one_is_refused(
        #[case] picks: Vec<usize>,
        #[case] error: IllegalAnswer,
    ) {
        let game = game(&["Ana Ana", ""], 0);
        let Some(Decision {
            kind: DecisionKind::ChooseCards(options),
            ..
        }) = decision(&game)
        else {
            panic!("the rule asks");
        };
        let cards = picks
            .iter()
            .map(|at| options.from.get(*at).copied().unwrap_or(ObjectId(99)))
            .collect();
        assert!(matches!(
            answer(&game, &options, &CardsChoice(cards)),
            Err(EngineError::Illegal(found)) if found == error
        ));
    }

    #[test]
    fn a_game_without_a_legend_asks_nothing() {
        let game = game(&["Plain Plain", "Plain"], 0);
        assert!(!game.possible.legends);
        assert!(groups(&game).is_empty() && decision(&game).is_none());
    }
}
