//! `Effect::BecomeDayNight`: it becomes day or night (CR 726.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The designation becomes `to`; when it already is that nothing is logged, so a change that was not one triggers nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BecomeDayNight { to } = effect else {
        return Err(wrong_effect("BecomeDayNight"));
    };
    Ok(Run::events(
        (cx.state().day_night() != Some(*to))
            .then_some(Event::DayNightChanged { to: *to })
            .into_iter()
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: becoming day or night logs one `DayNightChanged` unless the game already is that, from neither as well as from the other, and refuses another effect; the fold sets the designation.
    use mtg_cards::value::DayNight;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    fn game(now: Option<DayNight>) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        if let Some(now) = now {
            apply_event(&mut game, &Event::DayNightChanged { to: now }).unwrap();
        }
        game
    }

    fn run_on(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn events(run: Run) -> Vec<Event> {
        let Run::Done { events, next, .. } = run else {
            panic!("becoming day or night asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
    }

    #[rstest]
    #[case::day_from_neither(None, "Day", true)]
    #[case::night_from_neither(None, "Night", true)]
    #[case::day_from_night(Some(DayNight::Night), "Day", true)]
    #[case::night_from_day(Some(DayNight::Day), "Night", true)]
    #[case::day_when_it_is_day(Some(DayNight::Day), "Day", false)]
    #[case::night_when_it_is_night(Some(DayNight::Night), "Night", false)]
    fn the_designation_changes_unless_it_already_is_that(
        #[case] now: Option<DayNight>,
        #[case] to: &str,
        #[case] logged: bool,
    ) {
        let mut game = game(now);
        let json = format!(r#"{{"BecomeDayNight":{{"to":"{to}"}}}}"#);
        let done = events(run_on(&game, &json).unwrap());
        let wanted: DayNight = serde_json::from_str(&format!("\"{to}\"")).unwrap();
        assert_eq!(done.len(), usize::from(logged));
        for event in &done {
            assert_eq!(event, &Event::DayNightChanged { to: wanted });
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.day_night(), if logged { Some(wanted) } else { now });
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let refused = run_on(&game(None), r#"{"Draw":{}}"#);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
