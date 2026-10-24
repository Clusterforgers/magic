//! "If you do" after an effect that is not a "may": whether the effect took something away.

use mtg_cards::effect::Effect;

use super::may::performed_name;
use crate::choose::Target;
use crate::resolve::{Cx, Run};

/// A sacrifice or a removal of counters binds its controller for a following `If` of `Performed` when it proposed at
/// least one event (a permanent sacrificed, a counter taken off) and nothing when it did nothing; every other effect, and
/// an effect that asks first, passes through unchanged.
pub(super) fn report(cx: &Cx<'_>, effect: &Effect, mut run: Run) -> Run {
    if !matches!(
        effect,
        Effect::Sacrifice { .. } | Effect::RemoveCounters { .. }
    ) {
        return run;
    }
    let name = performed_name(cx.resolving.position());
    // A delayed trigger carries the names its spell bound, and its first step has the position of that spell's first
    // step: a name that is already bound belongs to the spell, not to this step.
    if cx.scope.bindings.get(&name).is_some() {
        return run;
    }
    if let Run::Done { events, bound, .. } = &mut run {
        let did = if events.is_empty() {
            Vec::new()
        } else {
            vec![Target::Player(cx.scope.controller)]
        };
        bound.push((name, did));
    }
    run
}

#[cfg(test)]
mod tests {
    //! Goal: a sacrifice or a removal of counters binds its controller under the name an "if you do" reads when it took something and nothing when it did not, and every other effect binds no such name.
    use mtg_cards::value::CounterKind;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope, prim};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 controls the Source with two charge counters; the table has `seats` seats.
    fn game(seats: usize) -> GameState {
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        let mut game = setup(vec![Vec::new(); seats], battlefield);
        game.players[0].battlefield[0].counters = vec![(CounterKind::Charge, 2)];
        GameState::from_setup(&game).unwrap()
    }

    /// What the effect bound under the name of position 0, or `None` when it bound nothing of the kind or asked.
    fn performed(game: &GameState, json: &str) -> Option<bool> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let Run::Done { bound, .. } = prim::run(&cx, &effect).unwrap() else {
            return None;
        };
        bound
            .iter()
            .find(|(name, _)| *name == performed_name(&[0]))
            .map(|(_, who)| !who.is_empty())
    }

    #[rstest]
    #[case::counters_taken(r#"{"RemoveCounters":{"from":"This","counter":"charge"}}"#, Some(true))]
    #[case::all_counters_taken(
        r#"{"RemoveCounters":{"from":"This","counter":"charge","count":5}}"#,
        Some(true)
    )]
    #[case::no_counter_of_the_kind(
        r#"{"RemoveCounters":{"from":"This","counter":"+1/+1"}}"#,
        Some(false)
    )]
    #[case::a_count_of_nothing(
        r#"{"RemoveCounters":{"from":"This","counter":"charge","count":0}}"#,
        Some(false)
    )]
    #[case::nothing_to_sacrifice(r#"{"Sacrifice":{"filter":[{"Type":"Land"}]}}"#, Some(false))]
    #[case::the_card_sacrificed_itself(r#"{"Sacrifice":{"filter":["This"]}}"#, Some(true))]
    #[case::another_effect_binds_nothing(r#"{"GainLife":{"amount":1}}"#, None)]
    #[case::a_draw_binds_nothing(r#"{"Draw":{}}"#, None)]
    fn an_effect_that_takes_something_reports_whether_it_did(
        #[case] json: &str,
        #[case] expected: Option<bool>,
        #[values(2, 3, 4)] seats: usize,
    ) {
        assert_eq!(performed(&game(seats), json), expected);
    }
}
