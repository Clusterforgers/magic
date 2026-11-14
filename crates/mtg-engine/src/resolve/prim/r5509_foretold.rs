//! `Effect::BecomeForetold`: the named objects are exiled face down and become foretold ("exile it face down. It becomes
//! foretold."); their owners may cast them on a later turn for their foretell costs. This is not foretelling (CR
//! 702.143b), so nothing that waits for a foretelling triggers.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each object the effect names that is still where it was named moves to exile as a foretold card: face down, seen by
/// its owner. An ability, the resolving spell and an object already in exile are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BecomeForetold { what } = effect else {
        return Err(wrong_effect("BecomeForetold"));
    };
    let mut events = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let resolving_spell = object.zone() == Zone::Stack && object.id() == cx.resolving.object();
        if object.kind() == ObjectKind::Ability || resolving_spell || object.zone() == Zone::Exile {
            continue;
        }
        events.push(Event::ZoneChanged {
            object: object.id(),
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Foretold,
        });
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the named permanent is exiled face down as a foretold card on this turn, at two to four seats, and an object
    //! already in exile is passed over.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};

    #[rstest]
    #[case::two_seats(2)]
    #[case::three_seats(3)]
    #[case::four_seats(4)]
    fn r5509_the_permanent_is_exiled_foretold(#[case] seats: usize) {
        let mut battlefields = vec![vec![creature("Soldier", 1, 1)]];
        battlefields.resize(seats, vec![]);
        let game = state(vec![vec![]; seats], battlefields);
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(r#"{"BecomeForetold":{"what":"This"}}"#).unwrap();
        let Run::Done { events, .. } = run(&cx, &effect).unwrap() else {
            panic!("asks nothing");
        };
        assert_eq!(
            events,
            vec![Event::ZoneChanged {
                object: source.id(),
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::Foretold,
            }]
        );
    }
}
