//! Phases added after the first combat phase of the turn rather than after the current phase (CR 500.8, 505.1a): "After
//! the first combat phase this turn, there's an additional combat phase." Before the first combat phase has begun, the
//! phases ahead are written out up to it and the added phase goes right behind it; while it is under way the phase is
//! added after the current one; once it has ended nothing is added. Each added combat phase gets its combat mark.

use crate::events::Event;
use crate::resolve::{Cx, Run};
use crate::state::GameState;
use crate::turn::{Phase, Step, TurnState};

/// `Effect::R110AddCombatAfterFirst`: a combat phase is added after the first combat phase of the turn, while it is still
/// to come or under way; nothing before the first turn or once it has ended.
pub(crate) fn r110_run(cx: &Cx<'_>) -> Run {
    let turn = cx.state().turn();
    if turn.number() == 0 || !r110_first_combat_ahead(turn) || turn.step().phase() == Phase::Ending
    {
        return Run::events(Vec::new());
    }
    Run::events(vec![Event::PhasesAdded {
        steps: vec![Step::BeginCombat],
        lone: false,
        r110_after_first_combat: true,
    }])
}

/// Whether phases added after the first combat phase can still come: none has begun yet, or the first is under way.
pub(crate) fn r110_first_combat_ahead(turn: &TurnState) -> bool {
    turn.combats() == 0 || (turn.combats() == 1 && turn.step().phase() == Phase::Combat)
}

/// Folds `PhasesAdded` with `r110_after_first_combat`: `steps` begin right after the first combat phase of the turn.
pub(crate) fn r110_add_after_first_combat(state: &mut GameState, steps: &[Step]) {
    let turn = &mut state.turn;
    if !r110_first_combat_ahead(turn) || turn.step().phase() == Phase::Ending {
        return;
    }
    if turn.ahead.is_empty() {
        // After a step on its own, the phase that follows the one it came after is still held.
        let Some(held) = turn.step().phase().next_start() else {
            return;
        };
        r110_insert(turn, 0, &[held], false);
    }
    let at = if turn.step().phase() == Phase::Combat {
        0
    } else {
        // The phases ahead are written out until the first combat phase is among them.
        loop {
            if let Some(at) = turn
                .ahead
                .iter()
                .position(|step| *step == Step::BeginCombat)
            {
                break at + 1;
            }
            let Some(next) = turn.ahead.last().and_then(|last| last.phase().next_start()) else {
                return;
            };
            let end = turn.ahead.len();
            r110_insert(turn, end, &[next], false);
        }
    };
    r110_insert(turn, at, steps, true);
}

/// Inserts `steps` among the phases ahead at `at`, each a whole phase; an added combat phase gets the next combat mark
/// (`Duration::R109ThatCombat`) when `added`; a phase written out of the regular sequence is not added and gets none.
fn r110_insert(turn: &mut TurnState, at: usize, steps: &[Step], added: bool) {
    if added && steps.contains(&Step::BeginCombat) && turn.r109_marks.is_empty() {
        turn.r109_marks = vec![0; turn.ahead.len()];
    }
    if !turn.r109_marks.is_empty() {
        turn.r109_marks.resize(turn.ahead.len(), 0);
        let mut marks = Vec::with_capacity(steps.len());
        for step in steps {
            if added && *step == Step::BeginCombat {
                turn.r109_given += 1;
                marks.push(turn.r109_given);
            } else {
                marks.push(0);
            }
        }
        turn.r109_marks.splice(at..at, marks);
    }
    turn.ahead.splice(at..at, steps.iter().copied());
    if !turn.r5500_lone.is_empty() {
        turn.r5500_lone
            .resize(turn.ahead.len() - steps.len(), false);
        turn.r5500_lone.splice(at..at, steps.iter().map(|_| false));
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a combat phase added after the first combat phase lands right behind it from any step before it (the phases
    //! ahead written out with no marks of their own), right after the current phase while the first combat is under way,
    //! nowhere once it has ended, and each added phase gets the next combat mark at two to four seats.
    use rstest::rstest;

    use super::r110_add_after_first_combat;
    use crate::fixtures::state;
    use crate::turn::Step;

    #[rstest]
    #[case::upkeep(Step::Upkeep, 0, vec![Step::Main1, Step::BeginCombat, Step::BeginCombat], vec![0, 0, 1])]
    #[case::precombat_main(Step::Main1, 0, vec![Step::BeginCombat, Step::BeginCombat], vec![0, 1])]
    #[case::first_combat(Step::DeclareAttackers, 1, vec![Step::BeginCombat, Step::Main2], vec![1, 0])]
    #[case::after_it(Step::Main2, 1, vec![], vec![])]
    #[case::second_combat(Step::BeginCombat, 2, vec![], vec![])]
    fn r110_a_combat_lands_behind_the_first(
        #[case] step: Step,
        #[case] combats: u32,
        #[case] ahead: Vec<Step>,
        #[case] marks: Vec<u32>,
        #[values(2, 3, 4)] seats: usize,
    ) {
        let mut game = state(vec![Vec::new(); seats], vec![Vec::new(); seats]);
        game.turn.number = 2;
        game.turn.step = step;
        game.turn.combats = combats;
        r110_add_after_first_combat(&mut game, &[Step::BeginCombat]);
        assert_eq!(game.turn.ahead, ahead);
        assert_eq!(game.turn.r109_marks, marks);
        if !ahead.is_empty() {
            r110_add_after_first_combat(&mut game, &[Step::BeginCombat]);
            assert_eq!(
                game.turn.r109_latest_mark(),
                2,
                "a second visit adds a second phase"
            );
            assert_eq!(game.turn.ahead.len(), ahead.len() + 1);
        }
    }
}
