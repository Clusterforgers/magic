//! `Effect::If`: a condition picks the effects that follow.

use mtg_cards::effect::Effect;
use mtg_cards::value::Condition;

use super::may::performed_name;
use super::wrong_effect;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The condition is read in the state the earlier steps left. When it holds the resolution enters `then`, else
/// `otherwise`; a list with nothing in it goes straight on to the next effect (`resolve/tree.rs`). "If you do" holds
/// when the effect just before this one in its list is a `May` that was accepted. A condition the evaluator cannot read
/// is refused by name, and `Caps` keeps the cards that use one out of every game.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::If { condition, .. } = effect else {
        return Err(wrong_effect("If"));
    };
    let branch = usize::from(!cx.scope.holds(&decided(cx, condition))?);
    Ok(Run::Done {
        events: Vec::new(),
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Enter(branch),
    })
}

/// The condition with each "if you do" in it decided: always true or always false, however it is wrapped.
fn decided(cx: &Cx<'_>, condition: &Condition) -> Condition {
    match condition {
        Condition::Performed if performed(cx) => Condition::All(Vec::new()),
        Condition::Performed => Condition::Any(Vec::new()),
        Condition::Not(inner) => Condition::Not(Box::new(decided(cx, inner))),
        Condition::All(items) => Condition::All(items.iter().map(|c| decided(cx, c)).collect()),
        Condition::Any(items) => Condition::Any(items.iter().map(|c| decided(cx, c)).collect()),
        other => other.clone(),
    }
}

/// Whether the effect before the running `If` in its list is a `May` that was accepted: the step of the `May` bound
/// its answer under the name of its position (`may::performed_name`). Nothing before it, or no `May`, is no.
fn performed(cx: &Cx<'_>) -> bool {
    let position = cx.resolving.position();
    let Some((last, holder)) = position.split_last() else {
        return false;
    };
    let Some(before) = last.checked_sub(1) else {
        return false;
    };
    let before = [holder, &[before]].concat();
    cx.scope
        .bindings
        .get(&performed_name(&before))
        .is_some_and(|accepted| !accepted.is_empty())
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
    #[case::the_monarch(r#"{"Monarch":"You"}"#, "the monarch")]
    #[case::devotion(
        r#"{"Compare":{"left":{"Devotion":["Red"]},"op":"Ge","right":1}}"#,
        "devotion"
    )]
    #[case::divided(
        r#"{"Compare":{"left":"Divided","op":"Ge","right":1}}"#,
        "a divided amount outside the damage or counters it divides"
    )]
    #[case::wrapped_in_not(r#"{"Not":{"Monarch":"You"}}"#, "the monarch")]
    #[case::wrapped_in_all(r#"{"All":["YourTurn",{"Monarch":"You"}]}"#, "the monarch")]
    #[case::wrapped_in_any(r#"{"Any":[{"Not":"YourTurn"},{"Monarch":"You"}]}"#, "the monarch")]
    fn a_condition_the_evaluator_cannot_read_is_refused_by_name(
        #[case] condition: &str,
        #[case] lacks: &'static str,
    ) {
        assert_eq!(
            run_as(&game(), ME, &if_(condition)).unwrap_err(),
            ResolveError::Unsupported(lacks)
        );
    }

    /// The branch the `If` at `position` enters when the `May`s at `mays` were accepted (`true`) or declined (`false`).
    fn after_mays(condition: &str, position: &str, mays: &[(&str, bool)]) -> usize {
        // A position is written with dots: "2.0.1" is the second effect of the first list of the third effect.
        let path =
            |text: &str| -> Vec<usize> { text.split('.').map(|n| n.parse().unwrap()).collect() };
        let position = path(position);
        let game = game();
        let bindings: crate::resolve::Bindings = mays
            .iter()
            .map(|(at, accepted)| {
                let who = if *accepted {
                    vec![crate::choose::Target::Player(ME)]
                } else {
                    Vec::new()
                };
                (performed_name(&path(at)), who)
            })
            .collect();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            position,
            ..Resolving::begin(source.id())
        };
        let mut scope = Scope::announcing(&game, ME, source.reference());
        scope.bindings = &bindings;
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        entered(run(&cx, &if_(condition)).unwrap())
    }

    #[rstest]
    #[case::accepted(r#""Performed""#, "1", &[("0", true)], 0)]
    #[case::declined(r#""Performed""#, "1", &[("0", false)], 1)]
    #[case::nothing_before_it(r#""Performed""#, "0", &[], 1)]
    #[case::no_may_before_it(r#""Performed""#, "1", &[], 1)]
    #[case::the_may_after_it_does_not_count(r#""Performed""#, "0", &[("1", true)], 1)]
    #[case::a_may_two_before_does_not_count(r#""Performed""#, "2", &[("0", true), ("1", false)], 1)]
    #[case::the_one_just_before_counts(r#""Performed""#, "2", &[("0", false), ("1", true)], 0)]
    #[case::inside_a_branch_accepted(r#""Performed""#, "2.0.1", &[("2.0.0", true)], 0)]
    #[case::inside_a_branch_declined(r#""Performed""#, "2.0.1", &[("2.0.0", false), ("0", true)], 1)]
    #[case::a_may_of_the_outer_list_is_not_inside(r#""Performed""#, "2.0.1", &[("1", true)], 1)]
    #[case::not_accepted(r#"{"Not":"Performed"}"#, "1", &[("0", true)], 1)]
    #[case::not_declined(r#"{"Not":"Performed"}"#, "1", &[("0", false)], 0)]
    #[case::all_accepted(r#"{"All":["Performed","YourTurn"]}"#, "1", &[("0", true)], 0)]
    #[case::all_declined(r#"{"All":["Performed","YourTurn"]}"#, "1", &[("0", false)], 1)]
    #[case::any_declined(r#"{"Any":["Performed","YourTurn"]}"#, "1", &[("0", false)], 0)]
    fn if_you_do_holds_when_the_may_just_before_it_was_accepted(
        #[case] condition: &str,
        #[case] position: &str,
        #[case] mays: &[(&str, bool)],
        #[case] branch: usize,
    ) {
        assert_eq!(after_mays(condition, position, mays), branch);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_as(&game(), ME, &effect(r#"{"Draw":{}}"#)).unwrap_err(),
            ResolveError::Invalid("the If primitive was given another effect".into())
        );
    }
}
