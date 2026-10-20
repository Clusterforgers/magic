//! `Effect::Modal`: the modes were chosen as the spell or ability was put on the stack (CR 700.2).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// Does nothing itself: the modes were chosen when the spell or ability was announced (`choose/mode.rs`), so the
/// step enters the first mode chosen. The modes chosen run in the order they are printed, a mode chosen twice twice,
/// and the interpreter moves on from each to the next (`resolve/tree.rs`).
pub(super) fn run(_cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Modal { .. } = effect else {
        return Err(wrong_effect("Modal"));
    };
    Ok(Run::Done {
        events: Vec::new(),
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Enter(0),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a modal effect does nothing but enter the first mode chosen, whatever was chosen, and refuses another effect by name.
    use mtg_core::ids::PlayerId;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::zones::Zone;

    fn modal() -> Effect {
        serde_json::from_str(
            r#"{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":1}}]}]}}"#,
        )
        .unwrap()
    }

    fn run_modal(effect: &Effect) -> Result<Run, ResolveError> {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    #[test]
    fn it_enters_the_first_mode_chosen_and_changes_nothing() {
        assert_eq!(
            run_modal(&modal()).unwrap(),
            Run::Done {
                events: Vec::new(),
                bound: Vec::new(),
                snapshots: Vec::new(),
                next: Next::Enter(0)
            }
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let draw: Effect = serde_json::from_str(r#"{"Draw":{}}"#).unwrap();
        assert_eq!(
            run_modal(&draw).unwrap_err(),
            ResolveError::Invalid("the Modal primitive was given another effect".into())
        );
    }
}
