//! `Effect::KeepTotalPower`: each player keeps any number of their permanents of a kind whose powers add up to a most
//! and sacrifices the rest (Slaughter the Strong, Destined Confrontation).

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::{characteristics, controller_of};
use crate::choose::CardsOptions;
use crate::choose::Choice;
use crate::decision::{Decision, DecisionKind};
use crate::events::{Event, ZoneCause};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// The power a permanent adds to the total: its power, a negative one included (it subtracts).
fn power_of(cx: &Cx<'_>, id: ObjectId) -> i64 {
    characteristics(cx.state(), id)
        .and_then(|chars| chars.power)
        .map_or(0, i64::from)
}

/// The permanents a player may add to `kept` so that the total can still end at `most` or less: one whose power, with
/// the kept ones and every negative power left, stays within it.
fn offered(cx: &Cx<'_>, own: &[ObjectId], kept: &[ObjectId], most: i64) -> Vec<ObjectId> {
    let left: Vec<ObjectId> = own
        .iter()
        .copied()
        .filter(|id| !kept.contains(id))
        .collect();
    let total: i64 = kept.iter().map(|id| power_of(cx, *id)).sum();
    let negatives: i64 = left.iter().map(|id| power_of(cx, *id).min(0)).sum();
    left.iter()
        .copied()
        .filter(|id| {
            let power = power_of(cx, *id);
            total + power + negatives - power.min(0) <= most
        })
        .collect()
}

/// Each player in turn order from the active player is asked for one permanent to keep at a time among those that keep
/// the total within the most (`offered`), and may stop, with no card, once the kept total is within it; a player with
/// nothing to offer is not asked. Then every permanent of the kind not kept goes to its owner's graveyard as sacrificed,
/// all together (one that can't be sacrificed stays).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::KeepTotalPower { what, most } = effect else {
        return Err(wrong_effect("KeepTotalPower"));
    };
    let state = cx.state();
    let most = cx.scope.amount(most)?;
    let matching = cx.scope.matching(what)?;
    let players = state.turn_order_from(state.active_or_next());
    let mut answers = cx.journal().iter();
    let mut doomed: Vec<(PlayerId, ObjectId)> = Vec::new();
    for player in players {
        let own: Vec<ObjectId> = matching
            .iter()
            .map(|permanent| permanent.id)
            .filter(|id| {
                state.object(*id).is_some_and(|object| {
                    object.zone() == Zone::Battlefield && controller_of(state, *id) == Some(player)
                })
            })
            .collect();
        let mut kept: Vec<ObjectId> = Vec::new();
        loop {
            let from = offered(cx, &own, &kept, most);
            if from.is_empty() {
                break;
            }
            let total: i64 = kept.iter().map(|id| power_of(cx, *id)).sum();
            let required = u32::from(total > most);
            let options = CardsOptions {
                source: cx.scope.source.id,
                from: from.clone(),
                min: required,
                max: 1,
                text: crate::wording::r4704_keep_power(what, most, total),
                can_cancel: false,
                one_per_name: false,
            };
            match answers.next() {
                None => {
                    return Ok(Run::Ask(Decision {
                        player,
                        kind: DecisionKind::ChooseCards(options),
                    }));
                }
                Some((by, Choice::Cards(cards)))
                    if *by == player && crate::choose::check_cards(&options, cards).is_ok() =>
                {
                    match cards.first() {
                        Some(card) => kept.push(*card),
                        None => break,
                    }
                }
                Some(other) => {
                    return Err(ResolveError::Invalid(format!(
                        "{other:?} is no answer to the choice of what {player:?} keeps"
                    )));
                }
            }
        }
        doomed.extend(
            own.iter()
                .copied()
                .filter(|id| !kept.contains(id))
                .map(|id| (player, id)),
        );
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of what to keep that was not asked".into(),
        ));
    }
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for (player, object) in doomed {
        let Some(owner) = state.object(object).map(crate::object::GameObject::owner) else {
            continue;
        };
        if !crate::prohibit::may_sacrifice(state, player, object) {
            continue;
        }
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
    //! Goal: a choice kept within a total of power is asked one permanent at a time among those that leave the total
    //! reachable, a negative power subtracting (a 5/5 kept beside a -2/1 within 4), must go on while the kept total is
    //! over the most and may stop once within it, is not asked of a player with nothing that fits, and sacrifices every
    //! other permanent of the kind of every player together; at two to four seats.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(seats: usize) -> GameState {
        let mut battlefields = vec![vec![
            creature("Source", 0, 1),
            creature("Big", 5, 5),
            creature("Shrunk", -2, 1),
            creature("Mid", 3, 3),
        ]];
        battlefields.resize_with(seats, || vec![creature("Huge", 9, 9)]);
        state(vec![vec![]; seats], battlefields)
    }

    fn named(game: &GameState, name: &str) -> ObjectId {
        game.zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| game.object(*id).unwrap().card().name == name)
            .unwrap()
    }

    fn step(game: &GameState, journal: Vec<(PlayerId, Choice)>) -> Run {
        let source = game.object(named(game, "Source")).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(
            r#"{"KeepTotalPower":{"what":[{"Type":"Creature"},{"Not":"This"}],"most":4}}"#,
        )
        .unwrap();
        run(&cx, &effect).unwrap()
    }

    fn asked(run: &Run) -> (Vec<ObjectId>, u32) {
        let Run::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        }) = run
        else {
            panic!("a choice is asked: {run:?}");
        };
        assert_eq!(*player, ME);
        assert_eq!(options.max, 1);
        let mut from = options.from.clone();
        from.sort_unstable();
        (from, options.min)
    }

    #[rstest]
    fn a_negative_power_lets_a_big_creature_be_kept(#[values(2, 3, 4)] seats: usize) {
        let game = game(seats);
        let (big, shrunk, mid) = (
            named(&game, "Big"),
            named(&game, "Shrunk"),
            named(&game, "Mid"),
        );
        let mut all = vec![big, shrunk, mid];
        all.sort_unstable();
        assert_eq!(asked(&step(&game, vec![])), (all, 0));
        let mut journal = vec![(ME, Choice::Cards(vec![big]))];
        assert_eq!(
            asked(&step(&game, journal.clone())),
            (vec![shrunk], 1),
            "over the most only the negative power is offered, and one must be taken"
        );
        journal.push((ME, Choice::Cards(vec![shrunk])));
        let Run::Done { events, .. } = step(&game, journal) else {
            panic!("nothing else fits and nobody else has anything that fits");
        };
        let mut gone: Vec<ObjectId> = events
            .iter()
            .filter_map(|event| match event {
                Event::ZoneChanged {
                    object,
                    cause: ZoneCause::Sacrificed,
                    ..
                } => Some(*object),
                _ => None,
            })
            .collect();
        gone.sort_unstable();
        let mut wanted = vec![mid];
        wanted.extend(
            game.zone(Zone::Battlefield)
                .iter()
                .copied()
                .filter(|id| game.object(*id).unwrap().card().name == "Huge"),
        );
        wanted.sort_unstable();
        assert_eq!(
            gone, wanted,
            "the 3/3 and every other seat's 9/9 go together"
        );
    }

    #[rstest]
    fn a_player_may_stop_once_within_the_most(#[values(2, 3, 4)] seats: usize) {
        let game = game(seats);
        let mid = named(&game, "Mid");
        let journal = vec![(ME, Choice::Cards(vec![mid])), (ME, Choice::Cards(vec![]))];
        let Run::Done { events, .. } = step(&game, journal) else {
            panic!("stopping ends the choice");
        };
        let kept_mid = !events
            .iter()
            .any(|event| matches!(event, Event::ZoneChanged { object, .. } if *object == mid));
        assert!(kept_mid);
        assert_eq!(
            events.len(),
            2 + (seats - 1),
            "the 5/5, the -2/1 and each 9/9 are sacrificed"
        );
    }
}
