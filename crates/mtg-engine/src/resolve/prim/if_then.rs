//! `Effect::If`: a condition picks the effects that follow.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The condition is read in the state the earlier steps left. When it holds the resolution enters `then`, else
/// `otherwise`; a list with nothing in it goes straight on to the next effect (`resolve/tree.rs`). A condition the
/// evaluator cannot read is refused by name, and `Caps` keeps the cards that use one out of every game.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::If { condition, .. } = effect else {
        return Err(wrong_effect("If"));
    };
    let branch = usize::from(!cx.scope.holds(condition)?);
    Ok(Run::Done {
        events: Vec::new(),
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Enter(branch),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: an `If` enters `then` when its condition holds and `otherwise` when it does not, reads the condition from the state as the earlier steps left it (life, the board, whose turn it is, from the controller's point of view), goes on past the effect when the list it picks is empty, takes no other step, and refuses by name a condition the evaluator cannot read, however it is wrapped.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope, tree};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 controls the Source (a 1/1) and an Ogre (a 3/4), seat 1 a Bear (a 2/2); seat 0 is on turn.
    fn game() -> GameState {
        state(
            vec![Vec::new(), Vec::new()],
            vec![
                vec![creature("Source", 1, 1), creature("Ogre", 3, 4)],
                vec![creature("Bear", 2, 2)],
            ],
        )
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn run_as(
        game: &GameState,
        controller: PlayerId,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, controller, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    /// The branch an `If` entered, when it took no other step.
    fn entered(run: Run) -> usize {
        let Run::Done {
            events,
            bound,
            snapshots,
            next: Next::Enter(branch),
        } = run
        else {
            panic!("an If is done at once and enters a branch");
        };
        assert!(events.is_empty() && bound.is_empty() && snapshots.is_empty());
        branch
    }

    fn if_(condition: &str) -> Effect {
        effect(&format!(
            r#"{{"If":{{"condition":{condition},"then":[{{"Draw":{{}}}}],"otherwise":[{{"GainLife":{{"amount":1}}}}]}}}}"#
        ))
    }

    #[rstest]
    #[case::your_turn_on_your_turn(r#""YourTurn""#, ME, 0)]
    #[case::your_turn_on_another_players(r#""YourTurn""#, FOE, 1)]
    #[case::not_your_turn(r#"{"Not":"YourTurn"}"#, FOE, 0)]
    #[case::life_at_least(r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#, ME, 0)]
    #[case::life_above(r#"{"Compare":{"left":{"Life":"You"},"op":"Gt","right":20}}"#, ME, 1)]
    #[case::the_life_of_an_opponent(
        r#"{"Compare":{"left":{"Life":"Opponents"},"op":"Eq","right":20}}"#,
        ME,
        0
    )]
    #[case::the_life_of_the_controller_is_the_one_asked(
        r#"{"Compare":{"left":{"Life":"You"},"op":"Eq","right":20}}"#,
        FOE,
        0
    )]
    #[case::a_count_of_the_board(
        r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#,
        ME,
        0
    )]
    #[case::fewer_than_that_for_the_other_seat(
        r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#,
        FOE,
        1
    )]
    #[case::the_source_matches(
        r#"{"Matches":{"subject":"This","selector":[{"Type":"Creature"}]}}"#,
        ME,
        0
    )]
    #[case::the_source_does_not_match(
        r#"{"Matches":{"subject":"This","selector":[{"Color":"Green"}]}}"#,
        ME,
        1
    )]
    #[case::a_power_read(
        r#"{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Lt","right":2}}"#,
        ME,
        0
    )]
    #[case::all_of_them(
        r#"{"All":["YourTurn",{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}]}"#,
        ME,
        0
    )]
    #[case::all_but_one(
        r#"{"All":["YourTurn",{"Compare":{"left":{"Life":"You"},"op":"Gt","right":20}}]}"#,
        ME,
        1
    )]
    #[case::any_of_them(
        r#"{"Any":[{"Not":"YourTurn"},{"Compare":{"left":1,"op":"Lt","right":2}}]}"#,
        ME,
        0
    )]
    #[case::none_of_them(
        r#"{"Any":[{"Not":"YourTurn"},{"Compare":{"left":2,"op":"Lt","right":2}}]}"#,
        ME,
        1
    )]
    fn a_condition_that_holds_enters_then_and_one_that_does_not_enters_otherwise(
        #[case] condition: &str,
        #[case] controller: PlayerId,
        #[case] branch: usize,
    ) {
        assert_eq!(
            entered(run_as(&game(), controller, &if_(condition)).unwrap()),
            branch
        );
    }

    #[test]
    fn the_condition_reads_the_state_as_it_is_now() {
        let mut game = game();
        let enters = |game: &GameState| {
            entered(
                run_as(
                    game,
                    ME,
                    &if_(r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#),
                )
                .unwrap(),
            )
        };
        assert_eq!(enters(&game), 0);
        crate::fold::apply_event(
            &mut game,
            &crate::events::Event::LifeLost {
                player: ME,
                amount: 1,
            },
        )
        .unwrap();
        assert_eq!(enters(&game), 1);
    }

    /// `[If, Gain]`: the effect that follows the `If`, by position, for each shape of its lists.
    #[rstest]
    #[case::then_taken(r#"[{"Draw":{}}]"#, r#"[{"GainLife":{"amount":1}}]"#, 0, vec![0, 0, 0])]
    #[case::otherwise_taken(r#"[{"Draw":{}}]"#, r#"[{"GainLife":{"amount":1}}]"#, 1, vec![0, 1, 0])]
    #[case::then_empty("[]", r#"[{"GainLife":{"amount":1}}]"#, 0, vec![1])]
    #[case::otherwise_empty(r#"[{"Draw":{}}]"#, "[]", 1, vec![1])]
    #[case::two_effects_in_then(r#"[{"Draw":{}},{"Draw":{}}]"#, "[]", 0, vec![0, 0, 0])]
    fn the_position_after_an_if_follows_the_branch_it_enters(
        #[case] then: &str,
        #[case] otherwise: &str,
        #[case] branch: usize,
        #[case] position: Vec<usize>,
    ) {
        let effects: Vec<Effect> = serde_json::from_str(&format!(
            r#"[{{"If":{{"condition":"YourTurn","then":{then},"otherwise":{otherwise}}}}},{{"GainLife":{{"amount":2}}}}]"#
        ))
        .unwrap();
        assert_eq!(
            tree::after(&effects, &[], &[0], Next::Enter(branch)),
            position
        );
    }

    #[rstest]
    #[case::kicked(r#""Kicked""#, "kicker")]
    #[case::x(r#"{"Compare":{"left":"X","op":"Ge","right":1}}"#, "X")]
    #[case::divided(
        r#"{"Compare":{"left":"Divided","op":"Ge","right":1}}"#,
        "divided amounts"
    )]
    #[case::wrapped_in_not(r#"{"Not":"Kicked"}"#, "kicker")]
    #[case::wrapped_in_all(r#"{"All":["YourTurn","Kicked"]}"#, "kicker")]
    #[case::wrapped_in_any(r#"{"Any":[{"Not":"YourTurn"},"Kicked"]}"#, "kicker")]
    fn a_condition_the_evaluator_cannot_read_is_refused_by_name(
        #[case] condition: &str,
        #[case] lacks: &'static str,
    ) {
        assert_eq!(
            run_as(&game(), ME, &if_(condition)).unwrap_err(),
            ResolveError::Unsupported(lacks)
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_as(&game(), ME, &effect(r#"{"Draw":{}}"#)).unwrap_err(),
            ResolveError::Invalid("the If primitive was given another effect".into())
        );
    }
}
