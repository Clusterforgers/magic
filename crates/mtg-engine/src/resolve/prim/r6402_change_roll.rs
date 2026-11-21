//! "After you roll a die, you may pay 1 life. If you do, increase or decrease the result by 1." (`Rule::R6402ChangeRoll`,
//! CR 706.2): once a player has rolled, each rule that lets them change a result asks them, in timestamp order, whether
//! to pay its cost and which die of the roll to change by how much; the changed number is the result, the number the die
//! showed its natural result.
//! A rule that lets them exchange a result with a number its permanent stored (`Rule::R113ExchangeRoll`, "If you would
//! roll one or more six-sided dice, instead roll them and you may exchange one result with this creature's base power or
//! base toughness") is asked first, as it replaces the roll.

use mtg_cards::modify::R6402RollPay;
use mtg_core::ids::PlayerId;

use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::R6402RollChanger;
use crate::state::{GameState, Tally};

/// The roll once every rule that could change it has been asked: the results in the order rolled, the events of the
/// changes (the costs paid and `R6402DieChanged`) and how many answers of the step's journal the questions used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct R6402Changed {
    /// The results, in the order rolled.
    pub(crate) results: Vec<u32>,
    /// The costs paid and the changes, in the order made.
    pub(crate) events: Vec<Event>,
    /// The answers the questions used, from the start of the journal.
    pub(crate) asked: usize,
}

impl R6402Changed {
    /// The roll events, each die with its result and, when changed, its natural result.
    pub(crate) fn rolled(&self, player: PlayerId, sides: u32, natural: &[u32]) -> Vec<Event> {
        self.results
            .iter()
            .zip(natural)
            .map(|(result, shown)| Event::DieRolled {
                player,
                sides,
                result: *result,
                natural: (result != shown).then_some(*shown),
            })
            .collect()
    }
}

/// The words of the cost of a change.
fn r6402_pay_text(pay: &R6402RollPay) -> String {
    match pay {
        R6402RollPay::Life(life) => format!("Pay {life} life"),
        R6402RollPay::RemoveCounter(counter) => format!("Remove a {} counter", counter.name()),
    }
}

/// Whether the player can pay the cost now, `spent` life having gone to the changes asked before (CR 119.4, 119.8).
fn r6402_payable(
    state: &GameState,
    player: PlayerId,
    changer: &R6402RollChanger,
    spent: u32,
) -> bool {
    match &changer.pay {
        R6402RollPay::Life(life) => {
            !crate::rules::life_total_cant_change(state, player)
                && state.player(player).is_some_and(|seat| {
                    i64::from(seat.life()) - i64::from(spent) >= i64::from(*life)
                })
        }
        R6402RollPay::RemoveCounter(counter) => state
            .object(changer.source.id)
            .and_then(|source| source.permanent())
            .is_some_and(|permanent| permanent.counters_of(counter) >= 1),
    }
}

/// The changes a rule offers on the results: none first, then each die up and down by `by` (never below 0).
fn r6402_offers(results: &[u32], by: u32) -> Vec<Option<(usize, u32)>> {
    let mut offers = vec![None];
    for (die, result) in results.iter().enumerate() {
        offers.push(Some((die, result.saturating_add(by))));
        if *result >= by {
            offers.push(Some((die, result - by)));
        }
    }
    offers
}

/// The options of a rule's question, in the order of `r6402_offers`.
fn r6402_options(results: &[u32], offers: &[Option<(usize, u32)>], pay: &str) -> Vec<ChoiceOption> {
    offers
        .iter()
        .map(|offer| {
            ChoiceOption::Label(match offer {
                None if results.len() == 1 => "Keep the result".to_owned(),
                None => "Keep the results".to_owned(),
                Some((die, to)) if results.len() == 1 => {
                    format!("{pay}: the {} becomes {to}", results[*die])
                }
                Some((die, to)) => {
                    format!("{pay}: die {} ({}) becomes {to}", die + 1, results[*die])
                }
            })
        })
        .collect()
}

/// The events of paying the cost of a change.
fn r6402_paid(player: PlayerId, changer: &R6402RollChanger) -> Event {
    match &changer.pay {
        R6402RollPay::Life(life) => Event::LifePaid {
            player,
            amount: *life,
        },
        R6402RollPay::RemoveCounter(counter) => Event::CountersChanged(CountersChanged {
            on: Target::Object(changer.source),
            counter: counter.clone(),
            delta: -1,
        }),
    }
}

/// Asks `player`, who rolled `rolled`, each rule that lets them change a result and that they can use now: one that
/// works once each turn and was used is passed over, and so is one whose cost they cannot pay. The answers are the first
/// of the step's journal; a question not yet answered is asked.
pub(crate) fn r6402_change(
    cx: &Cx<'_>,
    player: PlayerId,
    sides: u32,
    rolled: &[u32],
) -> Result<Result<R6402Changed, Run>, ResolveError> {
    let state = cx.state();
    let mut changed = R6402Changed {
        results: rolled.to_vec(),
        events: Vec::new(),
        asked: 0,
    };
    if rolled.is_empty() {
        return Ok(Ok(changed));
    }
    // The roll is replaced by one that lets them exchange (CR 614.1a), so the exchanges come before any change made
    // after it.
    if let Err(ask) = r113_exchange(cx, player, sides, &mut changed)? {
        return Ok(Err(ask));
    }
    let mut spent = 0;
    for changer in crate::rules::r6402_roll_changers(state, player) {
        if changer.once && state.object_tallied(changer.source, None, Tally::R6402Changed) > 0 {
            continue;
        }
        if !r6402_payable(state, player, &changer, spent) {
            continue;
        }
        let offers = r6402_offers(&changed.results, changer.by);
        let options = r6402_options(&changed.results, &offers, &r6402_pay_text(&changer.pay));
        let index = match cx.journal().get(changed.asked) {
            None => {
                return Ok(Err(Run::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        source: changer.source.id,
                        options,
                        text: format!(
                            "After you roll: increase or decrease a result by {}?",
                            changer.by
                        ),
                        can_cancel: false,
                    }),
                })));
            }
            Some((who, Choice::Option(index))) if *who == player && *index < offers.len() => *index,
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no answer to the change of a die's result of {player:?}"
                )));
            }
        };
        changed.asked += 1;
        let Some(Some((die, to))) = offers.get(index).copied() else {
            continue;
        };
        if let R6402RollPay::Life(life) = changer.pay {
            spent += life;
        }
        changed.events.push(r6402_paid(player, &changer));
        changed.events.push(Event::R6402DieChanged {
            player,
            source: changer.source,
            from: changed.results[die],
            to,
        });
        changed.results[die] = to;
    }
    Ok(Ok(changed))
}

/// The exchanges a rule offers: none first, then each die with each stored number.
fn r113_offers(results: &[u32], stored: &[i64]) -> Vec<Option<(usize, usize)>> {
    let mut offers = vec![None];
    for die in 0..results.len() {
        for place in 0..stored.len() {
            offers.push(Some((die, place)));
        }
    }
    offers
}

/// The options of an exchange question, in the order of `r113_offers`.
fn r113_options(
    results: &[u32],
    stored: &[i64],
    offers: &[Option<(usize, usize)>],
) -> Vec<ChoiceOption> {
    offers
        .iter()
        .map(|offer| {
            ChoiceOption::Label(match offer {
                None => "Exchange nothing".to_owned(),
                Some((die, place)) => format!(
                    "Exchange die {} ({}) with stored number {} ({})",
                    die + 1,
                    results[*die],
                    place + 1,
                    stored[*place]
                ),
            })
        })
        .collect()
}

/// Asks `player` of each rule that lets them exchange a result of a roll of dice with `sides` faces with a number its
/// permanent stored (`Rule::R113ExchangeRoll`) which exchange to make; the die takes the stored number and the memory the
/// result. The answers continue the step's journal after those `changed` used.
fn r113_exchange(
    cx: &Cx<'_>,
    player: PlayerId,
    sides: u32,
    changed: &mut R6402Changed,
) -> Result<Result<(), Run>, ResolveError> {
    let state = cx.state();
    for exchanger in crate::rules::r113_roll_exchangers(state, player, sides) {
        let slot = crate::memory::r112_lasting(&exchanger.slot);
        let mut stored = crate::memory::recall_numbers(state, exchanger.source, &slot);
        // An earlier exchange of this roll changed the memory too.
        for event in &changed.events {
            if let Event::Remembered(change) = event
                && change.source == exchanger.source
                && change.slot == slot
            {
                stored = change
                    .values
                    .iter()
                    .filter_map(|held| match held {
                        crate::memory::Held::Number(number) => Some(*number),
                        _ => None,
                    })
                    .collect();
            }
        }
        if stored.is_empty() {
            continue;
        }
        let offers = r113_offers(&changed.results, &stored);
        let index = match cx.journal().get(changed.asked) {
            None => {
                return Ok(Err(Run::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        source: exchanger.source.id,
                        options: r113_options(&changed.results, &stored, &offers),
                        text: "You may exchange one result with a stored number".to_owned(),
                        can_cancel: false,
                    }),
                })));
            }
            Some((who, Choice::Option(index))) if *who == player && *index < offers.len() => *index,
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no answer to the exchange of a die's result of {player:?}"
                )));
            }
        };
        changed.asked += 1;
        let Some(Some((die, place))) = offers.get(index).copied() else {
            continue;
        };
        let result = changed.results[die];
        let to = u32::try_from(stored[place].max(0)).unwrap_or(0);
        stored[place] = i64::from(result);
        changed.events.push(Event::R6402DieChanged {
            player,
            source: exchanger.source,
            from: result,
            to,
        });
        changed
            .events
            .push(Event::Remembered(Box::new(crate::memory::Remembered {
                source: exchanger.source,
                slot,
                objects: Vec::new(),
                values: stored
                    .into_iter()
                    .map(crate::memory::Held::Number)
                    .collect(),
                replace: true,
            })));
        changed.results[die] = to;
    }
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    //! Goal: the offers of a change are none, then each die up and down by the amount, never below 0, worded with the cost and the die; a cost is payable with enough life and with a counter of the kind on the permanent.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::one_die(&[4], 1, vec![None, Some((0, 5)), Some((0, 3))])]
    #[case::a_one_goes_to_zero(&[1], 1, vec![None, Some((0, 2)), Some((0, 0))])]
    #[case::not_below_zero(&[1], 2, vec![None, Some((0, 3))])]
    #[case::two_dice(&[2, 6], 1, vec![None, Some((0, 3)), Some((0, 1)), Some((1, 7)), Some((1, 5))])]
    fn r6402_offers_go_up_and_down_by_the_amount(
        #[case] results: &[u32],
        #[case] by: u32,
        #[case] expected: Vec<Option<(usize, u32)>>,
    ) {
        assert_eq!(r6402_offers(results, by), expected);
    }

    #[rstest]
    #[case::one_die(&[4], &["Keep the result", "Pay 1 life: the 4 becomes 5", "Pay 1 life: the 4 becomes 3"])]
    #[case::two_dice(&[2, 6], &["Keep the results", "Pay 1 life: die 1 (2) becomes 3", "Pay 1 life: die 1 (2) becomes 1", "Pay 1 life: die 2 (6) becomes 7", "Pay 1 life: die 2 (6) becomes 5"])]
    fn r6402_options_name_the_cost_and_the_die(#[case] results: &[u32], #[case] expected: &[&str]) {
        let offers = r6402_offers(results, 1);
        let options = r6402_options(results, &offers, &r6402_pay_text(&R6402RollPay::Life(1)));
        let labels: Vec<String> = options
            .into_iter()
            .map(|option| match option {
                ChoiceOption::Label(label) => label,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(labels, expected);
    }
}
