//! Mill replacement (CR 701.17, 614.1a): "If an opponent would mill one or more cards, they mill twice that many cards
//! instead." The cards one effect mills from a player's library are one event, the run of their `ZoneChanged` moves
//! with `ZoneCause::Milled`; `Multiply` and `Add` change how many cards that run takes from the top of the library.
//! The moves of the cards stay events of their own, so a replacement of a card's move (Rest in Peace) applies to
//! each card the changed mill takes.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Action, Candidate, Item, Origin, Pass, Slot, moving};
use crate::choose::{Choice, ReplacementCandidate, ReplacementOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::resolve::Scope;
use crate::zones::Zone;

/// The player whose card the slot mills, when it holds a move of a milled card from that player's library to their
/// graveyard.
fn milled_by(slot: &Slot) -> Option<PlayerId> {
    match &slot.item {
        Item::One(Event::ZoneChanged {
            from: Zone::Library(player),
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Milled,
            ..
        }) if player == owner => Some(*player),
        _ => None,
    }
}

impl Pass<'_> {
    /// The mill that begins at `at`: the player and the end (exclusive) of the run of slots that mill their cards.
    fn e19_mill_run(&self, at: usize) -> Option<(PlayerId, usize)> {
        let player = milled_by(self.slots.get(at)?)?;
        // A slot inside a run was changed with the run from its first slot.
        if at > 0 && milled_by(&self.slots[at - 1]) == Some(player) {
            return None;
        }
        let end = (at..self.slots.len())
            .find(|&slot| milled_by(&self.slots[slot]) != Some(player))
            .unwrap_or(self.slots.len());
        Some((player, end))
    }

    /// The mill replacements that apply to the mill of `player` at `at` and have not been applied to it.
    fn e19_mill_candidates(
        &self,
        at: usize,
        player: PlayerId,
    ) -> Result<Vec<Candidate>, EngineError> {
        let mut found: Vec<Candidate> = Vec::new();
        for source in &self.sources {
            let Ability::Replacement {
                when: ReplaceWhen::Mill { who },
                by: by @ (ReplaceBy::Multiply(_) | ReplaceBy::Add(_)),
                condition,
                ..
            } = &*source.ability
            else {
                continue;
            };
            let origin = if source.copied {
                Origin::Copied(source.address.ability)
            } else {
                Origin::Ability(source.address)
            };
            if self.slots[at].applied.contains(&origin) || found.iter().any(|c| c.origin == origin)
            {
                continue;
            }
            let scope = Scope::announcing(self.state, source.controller, source.object);
            if !scope.players(who)?.contains(&player) {
                continue;
            }
            if let Some(condition) = condition
                && !scope.holds(condition)?
            {
                continue;
            }
            found.push(Candidate {
                origin,
                object: source.object,
                controller: source.controller,
                action: Action::Replace(by.clone()),
                is_self: false,
            });
        }
        found.sort_by_key(|candidate| candidate.origin);
        Ok(found)
    }

    /// Applies the mill replacements to the mill that begins at `at`, one at a time, the milling player ordering two or
    /// more (CR 616.1); the question when one is needed.
    pub(super) fn e19_mill(&mut self, at: usize) -> Result<Option<Decision>, EngineError> {
        loop {
            let Some((player, end)) = self.e19_mill_run(at) else {
                return Ok(None);
            };
            let candidates = self.e19_mill_candidates(at, player)?;
            let chosen = match candidates.as_slice() {
                [] => return Ok(None),
                [only] => only.clone(),
                _ => match self.answer() {
                    None => {
                        return Ok(Some(Decision {
                            player,
                            kind: DecisionKind::ChooseReplacement(ReplacementOptions {
                                candidates: candidates
                                    .iter()
                                    .map(|c| ReplacementCandidate {
                                        source: c.object.id,
                                        ability: c.origin.ability_index(),
                                        shield: c.origin.shield(),
                                        text: self.words(c),
                                    })
                                    .collect(),
                                text: format!("Mill {} card(s).", end - at),
                            }),
                        }));
                    }
                    Some((who, Choice::Replacement(index)))
                        if *who == player && *index < candidates.len() =>
                    {
                        candidates[*index].clone()
                    }
                    Some(other) => {
                        return Err(Self::invalid(format!(
                            "{other:?} is no answer to the order of the mill replacements of {player:?}"
                        )));
                    }
                },
            };
            self.e19_scale_mill(at, (player, end), &chosen);
        }
    }

    /// The mill of `player` from `at` to `end` takes as many cards as `chosen` makes of it, from the top of the library
    /// below the cards it already takes; a library that runs out gives what it has.
    fn e19_scale_mill(&mut self, at: usize, (player, end): (PlayerId, usize), chosen: &Candidate) {
        let milled = u32::try_from(end - at).unwrap_or(u32::MAX);
        let wanted = match &chosen.action {
            Action::Replace(ReplaceBy::Multiply(times)) => milled.saturating_mul(*times),
            Action::Replace(ReplaceBy::Add(extra)) => milled.saturating_add(*extra),
            _ => milled,
        };
        let mut applied = self.slots[at].applied.clone();
        applied.push(chosen.origin);
        let library = Zone::Library(player);
        let taken: Vec<ObjectId> = self
            .slots
            .iter()
            .filter_map(|slot| match &slot.item {
                Item::One(event) => moving(self.state, event)
                    .filter(|(_, from, _)| *from == library)
                    .map(|(object, ..)| object),
                Item::Tokens(_) => None,
            })
            .collect();
        let extra = usize::try_from(wanted.saturating_sub(milled)).unwrap_or(usize::MAX);
        let more: Vec<ObjectId> = self
            .library_at(at, player)
            .into_iter()
            .filter(|card| !taken.contains(card))
            .take(extra)
            .collect();
        for slot in &mut self.slots[at..end] {
            slot.applied.clone_from(&applied);
        }
        for (index, object) in more.into_iter().enumerate() {
            self.slots.insert(
                end + index,
                Slot {
                    item: Item::One(Event::ZoneChanged {
                        object,
                        from: library,
                        to: Zone::Graveyard(player),
                        cause: ZoneCause::Milled,
                    }),
                    applied: applied.clone(),
                    frozen: false,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a mill of an opponent is one event that "twice that many" and "that many plus four" change as a whole, over
    //! any count, in the order the milling player chooses when both apply, never past the end of the library and never
    //! for a mill the effect does not name; each card the changed mill takes is still a move a card's replacement sees.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, setup};
    use crate::library::top_cards;
    use crate::state::GameState;
    use crate::zones::Zone;

    const TWICE: &str =
        r#"{"Replacement":{"when":{"Mill":{"who":"Opponents"}},"by":{"Multiply":2}}}"#;
    const PLUS_FOUR: &str =
        r#"{"Replacement":{"when":{"Mill":{"who":"Opponents"}},"by":{"Add":4}}}"#;
    const EXILE: &str = r#"{"Replacement":{"when":{"ZoneChange":{"what":[],"to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#;

    /// Seat 0 controls a permanent for each of `rules`; seat 1's library holds `library` cards, seat 0's five.
    fn game(rules: &[&str], library: usize) -> GameState {
        let permanents = rules
            .iter()
            .enumerate()
            .map(|(index, rules)| {
                let rule: Ability = serde_json::from_str(rules).unwrap();
                Arc::new(
                    CardDefinition::new(format!("Source {index}"))
                        .with_type(CardType::Enchantment)
                        .with_rules(rule),
                )
            })
            .collect();
        let mut setup = setup(vec![vec![], vec![]], vec![permanents, vec![]]);
        setup.players[1].library = (0..library)
            .map(|n| creature(&format!("Card {n}"), 1, 1))
            .collect();
        GameState::from_setup(&setup).unwrap()
    }

    /// The moves of the top `count` cards of `player`'s library to their graveyard, as `Effect::Mill` proposes them.
    fn mill(game: &GameState, player: PlayerId, count: usize) -> Vec<Event> {
        top_cards(game, player, count)
            .into_iter()
            .map(|object| Event::ZoneChanged {
                object,
                from: Zone::Library(player),
                to: Zone::Graveyard(player),
                cause: ZoneCause::Milled,
            })
            .collect()
    }

    /// Where each card of the processed batch goes; the cards must be the top cards of seat 1's library in order.
    fn milled(game: &GameState, answers: &[usize], batch: Vec<Event>) -> Vec<Zone> {
        let journal: Vec<(PlayerId, Choice)> = answers
            .iter()
            .map(|index| (PlayerId(1), Choice::Replacement(*index)))
            .collect();
        let Processed::Events(events) = process(game, batch, &journal).unwrap() else {
            panic!("every question is answered");
        };
        let top = top_cards(game, PlayerId(1), usize::MAX);
        events
            .iter()
            .enumerate()
            .map(|(index, event)| match event {
                Event::ZoneChanged {
                    object,
                    from: Zone::Library(_),
                    to,
                    cause: ZoneCause::Milled,
                } => {
                    assert_eq!(*object, top[index], "the cards come from the top in order");
                    *to
                }
                other => panic!("only mills are proposed, not {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::twice_one(&[TWICE], 1, 2)]
    #[case::twice_three(&[TWICE], 3, 6)]
    #[case::plus_four_one(&[PLUS_FOUR], 1, 5)]
    #[case::plus_four_three(&[PLUS_FOUR], 3, 7)]
    #[case::twice_twice(&[TWICE, TWICE], 2, 8)]
    #[case::short_library(&[TWICE], 7, 12)]
    #[case::plus_runs_out(&[PLUS_FOUR], 10, 12)]
    fn an_opponents_mill_takes_the_changed_count(
        #[case] rules: &[&str],
        #[case] count: usize,
        #[case] expected: usize,
    ) {
        let game = game(rules, 12);
        let batch = mill(&game, PlayerId(1), count);
        let answers = if rules.len() > 1 { vec![0] } else { vec![] };
        let zones = milled(&game, &answers, batch);
        assert_eq!(zones, vec![Zone::Graveyard(PlayerId(1)); expected]);
    }

    #[rstest]
    #[case::twice_first(0, 2 * 3 + 4)]
    #[case::plus_first(1, (3 + 4) * 2)]
    fn the_milling_player_orders_two_changes(#[case] first: usize, #[case] expected: usize) {
        let game = game(&[TWICE, PLUS_FOUR], 20);
        let batch = mill(&game, PlayerId(1), 3);
        let Processed::Ask { decision, .. } = process(&game, batch.clone(), &[]).unwrap() else {
            panic!("two effects apply");
        };
        assert_eq!(decision.player, PlayerId(1));
        let DecisionKind::ChooseReplacement(options) = decision.kind else {
            panic!("the order is asked");
        };
        assert_eq!(options.candidates.len(), 2);
        assert_eq!(milled(&game, &[first], batch).len(), expected);
    }

    #[rstest]
    #[case::own_mill(&[TWICE], PlayerId(0))]
    #[case::no_effect(&[], PlayerId(1))]
    #[case::other_event(&[EXILE], PlayerId(1))]
    fn a_mill_the_effect_does_not_name_keeps_its_count(
        #[case] rules: &[&str],
        #[case] player: PlayerId,
    ) {
        let game = game(rules, 12);
        let batch = mill(&game, player, 2);
        let Processed::Events(events) = process(&game, batch, &[]).unwrap() else {
            panic!("nothing is asked");
        };
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn each_card_of_a_changed_mill_is_still_a_move() {
        let game = game(&[TWICE, EXILE], 12);
        let batch = mill(&game, PlayerId(1), 2);
        assert_eq!(milled(&game, &[], batch), vec![Zone::Exile; 4]);
    }
}
