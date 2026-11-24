//! Random actions of a player ("whenever you discard one or more cards at random, select one or more targets at
//! random, choose one or more modes at random, flip one or more coins, or roll one or more dice"): each instruction that
//! does one of these is one action, however many cards, targets, modes, coins or dice it takes. A roll or a flip is the
//! dice or coins of the player in one batch (as "roll one or more dice" counts a roll), a discard at random the cards a
//! random pick took from the hand that the batch then discards, and an announcement one action for its random modes and
//! one for its random targets. Also the helper of a cast with assist: the other player who spent mana in its batch.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::events::Event;

/// The players for whom `event`, after `earlier` in its batch, is a random action, once for each action it begins.
pub(super) fn r120_random_actions(earlier: &[Event], event: &Event) -> Vec<PlayerId> {
    // A step of a resolution ends one instruction.
    let from = earlier
        .iter()
        .rposition(|seen| matches!(seen, Event::StepDone { .. }))
        .map_or(0, |at| at + 1);
    let earlier = &earlier[from..];
    match event {
        Event::DieRolled { player, .. } => (!earlier
            .iter()
            .any(|seen| matches!(seen, Event::DieRolled { player: by, .. } if by == player)))
        .then_some(*player)
        .into_iter()
        .collect(),
        Event::CoinFlipped { player, .. } => (!earlier
            .iter()
            .any(|seen| matches!(seen, Event::CoinFlipped { player: by, .. } if by == player)))
        .then_some(*player)
        .into_iter()
        .collect(),
        Event::CardDiscarded { player, object } => {
            let picked = |card| {
                earlier.iter().any(
                    |seen| matches!(seen, Event::ChosenAtRandom { cards } if cards.contains(card)),
                )
            };
            let first = !earlier.iter().any(|seen| {
                matches!(seen, Event::CardDiscarded { player: by, object: card }
                    if by == player && picked(card))
            });
            (picked(object) && first)
                .then_some(*player)
                .into_iter()
                .collect()
        }
        Event::SpellCast {
            player, choices, ..
        } => announced(*player, choices),
        Event::AbilityPutOnStack(put) => announced(put.controller, &put.choices),
        _ => Vec::new(),
    }
}

/// The player other than `caster` who spent mana in the batch of a cast, before it: the helper of a spell with assist,
/// whose payment the cast carries (CR 702.132a).
pub(super) fn r120_helper(earlier: &[Event], caster: PlayerId) -> Option<PlayerId> {
    earlier.iter().find_map(|seen| match seen {
        Event::ManaSpent { player, mana, .. } if *player != caster && !mana.is_empty() => {
            Some(*player)
        }
        _ => None,
    })
}

/// At the first damage a source deals in its batch, the source and the total damage each recipient took from it in the
/// batch ("if at least one permanent or player was dealt exactly 2 of that damage"); none at its later damage.
pub(super) fn r120_source_damage(
    earlier: &[Event],
    event: &Event,
    later: &[Event],
) -> Option<(ObjectId, Vec<u32>)> {
    let Event::DamageDealt { source, .. } = event else {
        return None;
    };
    // A step of a resolution ends one instruction, and so one damage event of the source.
    let step = |seen: &Event| matches!(seen, Event::StepDone { .. });
    let earlier = &earlier[earlier.iter().rposition(step).map_or(0, |at| at + 1)..];
    let later = &later[..later.iter().position(step).unwrap_or(later.len())];
    let dealt = |seen: &Event| match seen {
        Event::DamageDealt {
            source: by,
            target,
            amount,
            ..
        } if by == source => Some((*target, *amount)),
        _ => None,
    };
    if earlier.iter().any(|seen| dealt(seen).is_some()) {
        return None;
    }
    let mut totals: Vec<(crate::combat::DamageTarget, u32)> = Vec::new();
    for (target, amount) in std::iter::once(event).chain(later).filter_map(dealt) {
        match totals.iter_mut().find(|(hit, _)| *hit == target) {
            Some((_, total)) => *total = total.saturating_add(amount),
            None => totals.push((target, amount)),
        }
    }
    Some((
        *source,
        totals.into_iter().map(|(_, total)| total).collect(),
    ))
}

/// One action for modes chosen at random and one for targets selected at random.
fn announced(controller: PlayerId, choices: &crate::choose::Choices) -> Vec<PlayerId> {
    [choices.random_modes, choices.random_targets > 0]
        .into_iter()
        .filter(|random| *random)
        .map(|_| controller)
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a roll, a flip and a random discard of a player are one action each however many dice, coins or cards
    //! they take in their batch, another player's are theirs, a discard the player chose is none, and an announcement
    //! is one action for its random modes and one for its random targets; the helper of a cast is the other player who
    //! spent mana in its batch.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::r120_random_actions;
    use crate::choose::Choices;
    use crate::events::Event;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn die(player: PlayerId) -> Event {
        Event::DieRolled {
            player,
            sides: 6,
            result: 3,
            natural: None,
        }
    }

    fn coin(player: PlayerId) -> Event {
        Event::CoinFlipped {
            player,
            won: true,
            call: crate::choose::CoinSide::default(),
            side: crate::choose::CoinSide::default(),
            coins: Vec::new(),
        }
    }

    fn discard(player: PlayerId, card: u64) -> Event {
        Event::CardDiscarded {
            player,
            object: ObjectId(card),
        }
    }

    fn picked(cards: &[u64]) -> Event {
        Event::ChosenAtRandom {
            cards: cards.iter().copied().map(ObjectId).collect(),
        }
    }

    /// The actions each event of the batch begins, in order.
    fn actions(batch: &[Event]) -> Vec<PlayerId> {
        (0..batch.len())
            .flat_map(|at| r120_random_actions(&batch[..at], &batch[at]))
            .collect()
    }

    #[rstest]
    #[case::one_die(vec![die(ME)], vec![ME])]
    #[case::three_dice(vec![die(ME), die(ME), die(ME)], vec![ME])]
    #[case::dice_of_two_players(vec![die(ME), die(FOE), die(ME)], vec![ME, FOE])]
    #[case::one_coin(vec![coin(ME)], vec![ME])]
    #[case::two_coins(vec![coin(ME), coin(ME)], vec![ME])]
    #[case::a_die_and_a_coin(vec![die(ME), coin(ME)], vec![ME, ME])]
    #[case::a_random_discard(vec![picked(&[5, 6]), discard(ME, 5), discard(ME, 6)], vec![ME])]
    #[case::a_chosen_discard(vec![discard(ME, 5)], vec![])]
    #[case::each_player_at_random(
        vec![picked(&[5]), picked(&[7]), discard(ME, 5), discard(FOE, 7)],
        vec![ME, FOE]
    )]
    #[case::a_random_pick_not_discarded(vec![picked(&[5]), discard(ME, 6)], vec![])]
    fn r120_a_batch_is_one_action_of_each_kind(
        #[case] batch: Vec<Event>,
        #[case] expected: Vec<PlayerId>,
    ) {
        assert_eq!(actions(&batch), expected);
    }

    fn spent(player: PlayerId, mana: usize) -> Event {
        Event::ManaSpent {
            player,
            mana: vec![mtg_core::mana::ManaType::Colorless; mana],
            restricted: Vec::new(),
        }
    }

    #[rstest]
    #[case::helped(vec![spent(FOE, 2), spent(ME, 1)], Some(FOE))]
    #[case::paid_alone(vec![spent(ME, 3)], None)]
    #[case::nothing_paid(vec![], None)]
    #[case::nothing_spent_by_them(vec![spent(FOE, 0)], None)]
    fn r120_the_helper_is_who_else_spent_mana_in_the_cast(
        #[case] earlier: Vec<Event>,
        #[case] expected: Option<PlayerId>,
    ) {
        assert_eq!(super::r120_helper(&earlier, ME), expected);
    }

    fn hit(source: u64, player: PlayerId, amount: u32) -> Event {
        Event::DamageDealt {
            source: ObjectId(source),
            target: crate::combat::DamageTarget::Player(player),
            amount,
            deathtouch: false,
            form: crate::combat::DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        }
    }

    #[rstest]
    #[case::one_hit(vec![hit(1, FOE, 2)], vec![Some(vec![2])])]
    #[case::two_recipients(vec![hit(1, FOE, 2), hit(1, ME, 3)], vec![Some(vec![2, 3]), None])]
    #[case::one_recipient_twice(vec![hit(1, FOE, 1), hit(1, FOE, 1)], vec![Some(vec![2]), None])]
    #[case::two_sources(vec![hit(1, FOE, 2), hit(2, FOE, 2)], vec![Some(vec![2]), Some(vec![2])])]
    fn r120_a_source_is_one_event_with_each_recipients_total(
        #[case] batch: Vec<Event>,
        #[case] expected: Vec<Option<Vec<u32>>>,
    ) {
        let found: Vec<Option<Vec<u32>>> = (0..batch.len())
            .map(|at| {
                super::r120_source_damage(&batch[..at], &batch[at], &batch[at + 1..])
                    .map(|(_, totals)| totals)
            })
            .collect();
        assert_eq!(found, expected);
    }

    #[rstest]
    #[case::nothing_random(false, 0, 0)]
    #[case::random_modes(true, 0, 1)]
    #[case::random_targets(false, 2, 1)]
    #[case::both(true, 1, 2)]
    fn r120_an_announcement_is_an_action_per_random_choice(
        #[case] random_modes: bool,
        #[case] random_targets: u8,
        #[case] expected: usize,
    ) {
        let choices = Choices {
            random_modes,
            random_targets,
            ..Choices::default()
        };
        let cast = Event::SpellCast {
            player: FOE,
            object: ObjectId(9),
            choices,
        };
        assert_eq!(actions(&[cast]), vec![FOE; expected]);
    }
}
