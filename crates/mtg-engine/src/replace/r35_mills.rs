//! Mills that several replacements of one batch make each read the library as the batch began ("prevent that damage
//! and each opponent mills that many cards" over each attacker's damage): a card an earlier mill of the batch took
//! is not milled again, the next card from the top is (CR 701.17a).

use std::collections::BTreeSet;

use mtg_core::ids::ObjectId;

use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// Gives each mill of `events` a card no earlier move out of that library in the batch took: the topmost card left,
/// or no mill when the library has none left.
pub(super) fn r35_fresh_mills(state: &GameState, events: &mut Vec<Event>) {
    let mut taken: BTreeSet<ObjectId> = BTreeSet::new();
    let mut kept = Vec::with_capacity(events.len());
    for mut event in events.drain(..) {
        match &mut event {
            Event::ZoneChanged {
                object,
                from: Zone::Library(owner),
                cause: ZoneCause::Milled,
                ..
            } => {
                if taken.contains(object) {
                    let next = state
                        .zone(Zone::Library(*owner))
                        .iter()
                        .rev()
                        .find(|card| !taken.contains(card))
                        .copied();
                    let Some(card) = next else { continue };
                    *object = card;
                }
                taken.insert(*object);
            }
            Event::ZoneChanged {
                object,
                from: Zone::Library(_),
                ..
            }
            | Event::CardDrawn { object, .. } => {
                taken.insert(*object);
            }
            _ => {}
        }
        kept.push(event);
    }
    *events = kept;
}

#[cfg(test)]
mod tests {
    //! Goal: the mills that the replacements of one damage batch make take distinct cards from the top down, and stop
    //! when the library runs out.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use crate::combat::{DamageForm, DamageTarget};
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::state;
    use crate::replace::{Processed, process};
    use crate::state::GameState;
    use crate::zones::Zone;

    const MILLS: &str = r#"{"Replacement":{"when":{"Damage":{"to":"Opponents","source":[{"ControlledBy":"You"}]}},"by":{"PreventAnd":[{"Mill":{"who":"Opponents","count":"Triggering"}}]}}}"#;

    fn game() -> GameState {
        let rule: Ability = serde_json::from_str(MILLS).unwrap();
        let mindskinner = CardDefinition::new("Mindskinner")
            .with_type(CardType::Creature)
            .with_rules(rule);
        state(
            vec![vec![], vec![]],
            vec![vec![Arc::new(mindskinner)], vec![]],
        )
    }

    fn hit(source: ObjectId, amount: u32) -> Event {
        Event::DamageDealt {
            source,
            target: DamageTarget::Player(PlayerId(1)),
            amount,
            deathtouch: false,
            form: DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        }
    }

    #[rstest]
    #[case::two_hits_within_the_library(&[2, 2], 4)]
    #[case::three_hits_within_the_library(&[1, 2, 1], 4)]
    #[case::hits_past_the_library(&[3, 4], 5)]
    fn each_mill_of_a_batch_takes_the_next_card(#[case] hits: &[u32], #[case] milled: usize) {
        let game = game();
        let source = game.zone(Zone::Battlefield)[0];
        let batch = hits.iter().map(|amount| hit(source, *amount)).collect();
        let Processed::Events(events) = process(&game, batch, &[]).unwrap() else {
            panic!("nothing is asked");
        };
        let mills: Vec<ObjectId> = events
            .iter()
            .filter_map(|event| match event {
                Event::ZoneChanged {
                    object,
                    cause: ZoneCause::Milled,
                    ..
                } => Some(*object),
                _ => None,
            })
            .collect();
        let top: Vec<ObjectId> = game
            .zone(Zone::Library(PlayerId(1)))
            .iter()
            .rev()
            .take(milled)
            .copied()
            .collect();
        assert_eq!(mills, top, "{events:?}");
        let mut folded = game.clone();
        for event in &events {
            crate::fold::apply_event(&mut folded, event).unwrap();
        }
    }
}
