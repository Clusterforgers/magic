//! Questions asked at every roll of dice (CR 706): a rule that adds a die whose roll its controller ignores
//! (`Rule::R706IgnoreChosen`, CR 706.6, Bamboozling Beeble) is applied as the dice are rolled, and a rule that lets the
//! roller pay to reroll dice of the roll (`Rule::R706Reroll`, CR 706.2b, Monitor Monitor) right after, before any change
//! by an amount. Both ask with `DecisionKind::R706ChooseDice`; the answer is kept in the journal as one
//! `Choice::Option` whose bits are the dice chosen.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::choose::Choice;
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::resolve::prim::r6402_change_roll::R6402Changed;
use crate::resolve::{ResolveError, Run};
use crate::rng::GameRng;
use crate::state::{GameState, Tally};

/// The most dice one question offers: the answer is a bit set of them.
const R706_MOST: usize = 63;

/// What the dice chosen are for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum R706DicePurpose {
    /// The roller pays the cost and rerolls the dice chosen; none chosen pays nothing.
    Reroll(#[cfg_attr(feature = "ts", ts(type = "string"))] ManaCost),
    /// The die chosen is ignored: its roll never happened.
    Ignore,
}

/// A question about the dice of a roll just made: which to reroll, or which roll to ignore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct R706DiceOptions {
    /// The permanent whose rule asks, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source: Option<ObjectId>,
    /// Who rolled.
    pub roller: PlayerId,
    /// How many faces the dice have.
    pub sides: u32,
    /// The results of the dice, in the order rolled.
    pub results: Vec<u32>,
    /// The fewest dice the answer names.
    pub min: usize,
    /// The most dice the answer names.
    pub max: usize,
    /// What the dice chosen are for.
    pub purpose: R706DicePurpose,
    /// The question in words.
    pub text: String,
}

impl R706DiceOptions {
    /// The answer that does the least: no reroll, or the first roll ignored.
    pub fn passive(&self) -> Vec<usize> {
        (0..self.min).collect()
    }

    /// The answer a player who wants high results gives: for a reroll the dice below the middle face, for an ignored
    /// roll the lowest when the chooser is `friendly` to the roller and else the highest.
    pub fn favoured(&self, friendly: bool) -> Vec<usize> {
        match self.purpose {
            R706DicePurpose::Reroll(_) => {
                let middle = (self.sides + 1) / 2;
                let low: Vec<usize> = (0..self.results.len())
                    .filter(|die| self.results[*die] < middle)
                    .collect();
                low.into_iter().take(self.max).collect()
            }
            R706DicePurpose::Ignore => {
                let by = |die: &usize| self.results[*die];
                let pick = if friendly {
                    (0..self.results.len()).min_by_key(by)
                } else {
                    (0..self.results.len()).max_by_key(by)
                };
                pick.into_iter().collect()
            }
        }
    }
}

/// The bit set of the dice chosen.
fn r706_mask(dice: &[usize]) -> usize {
    dice.iter().fold(0, |mask, die| mask | (1 << die))
}

/// The dice of a bit set, in order, when it names only dice of `count`.
fn r706_unmask(mask: usize, count: usize) -> Option<Vec<usize>> {
    (mask >> count == 0).then(|| (0..count).filter(|die| mask & (1 << die) != 0).collect())
}

/// Checks the dice chosen against the question: distinct dice of the roll, as many as it allows.
fn r706_check(options: &R706DiceOptions, dice: &[usize]) -> Result<(), IllegalAnswer> {
    let mut seen = dice.to_vec();
    seen.sort_unstable();
    seen.dedup();
    if seen.len() != dice.len()
        || dice.iter().any(|die| *die >= options.results.len())
        || dice.len() < options.min
        || dice.len() > options.max
    {
        return Err(IllegalAnswer::NotOffered("dice"));
    }
    Ok(())
}

/// The answer to a question about dice: logged for the resolution or the roll to visit that asked
/// (`Event::ChoiceMade`), or kept by the announcement whose cost rolled.
pub(crate) fn r706_answer(
    player: PlayerId,
    options: &R706DiceOptions,
    dice: &[usize],
    announcing: bool,
) -> Result<Outcome, EngineError> {
    r706_check(options, dice)?;
    let mask = r706_mask(dice);
    if announcing {
        return Ok(Outcome::Way(mask));
    }
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Option(mask),
    }]))
}

/// The answer the journal holds for the next question of the roll, `None` when it is not given yet.
fn r706_read(
    journal: &[(PlayerId, Choice)],
    changed: &R6402Changed,
    options: &R706DiceOptions,
) -> Result<Option<Vec<usize>>, ResolveError> {
    match journal.get(changed.asked) {
        None => Ok(None),
        Some((_, Choice::Option(mask))) => r706_unmask(*mask, options.results.len())
            .filter(|dice| r706_check(options, dice).is_ok())
            .map(Some)
            .ok_or_else(|| ResolveError::Invalid(format!("{mask} names no dice of {options:?}"))),
        Some(other) => Err(ResolveError::Invalid(format!(
            "{other:?} is no answer to a question about dice"
        ))),
    }
}

/// The next die of the roll, from the substream after the ones the roll has drawn so far.
fn r706_draw(state: &GameState, sides: u32, changed: &mut R6402Changed) -> u32 {
    let below = GameRng::substream(state.seed(), state.rng_index() + changed.drawn)
        .below(u64::from(sides.max(1)));
    changed.drawn += 1;
    u32::try_from(below).unwrap_or(0) + 1
}

/// "Instead they roll that many dice plus one and you choose one of those rolls to ignore" (CR 706.6): for each rule
/// change in force on the roller's rolls, one more die, and its controller chooses the roll ignored; the change ends.
/// A chooser who has left the game leaves the choice to the roller; dice that all rolled the same ask nothing.
pub(crate) fn r706_ignore(
    (state, journal): (&GameState, &[(PlayerId, Choice)]),
    player: PlayerId,
    sides: u32,
    changed: &mut R6402Changed,
) -> Result<Result<(), Run>, ResolveError> {
    for (effect, chooser, source) in crate::rules::r706_ignorers(state, player) {
        if changed.results.len() >= R706_MOST {
            break;
        }
        let die = r706_draw(state, sides, changed);
        changed.results.push(die);
        changed.natural.push(die);
        let chooser = if state.is_alive(chooser) {
            chooser
        } else {
            player
        };
        let options = R706DiceOptions {
            source: source.map(|source| source.id),
            roller: player,
            sides,
            results: changed.results.clone(),
            min: 1,
            max: 1,
            purpose: R706DicePurpose::Ignore,
            text: "Choose one of the rolls to ignore.".to_owned(),
        };
        let at = if changed.results.iter().all(|result| *result == die) {
            0
        } else {
            match r706_read(journal, changed, &options)? {
                None => {
                    return Ok(Err(Run::Ask(Decision {
                        player: chooser,
                        kind: DecisionKind::R706ChooseDice(options),
                    })));
                }
                Some(dice) => {
                    changed.asked += 1;
                    dice.first().copied().unwrap_or(0)
                }
            }
        };
        let result = changed.results.remove(at);
        changed.natural.remove(at);
        changed.events.push(Event::R706DieIgnored {
            roller: player,
            chooser,
            effect,
            result,
        });
    }
    Ok(Ok(()))
}

/// The events of paying `cost` for the rerolls of `source`, the mana chosen automatically; `None` when it can't be paid.
fn r706_payment(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    cost: &ManaCost,
) -> Option<Vec<Event>> {
    let spend = crate::spend::Spend::for_ability(state, source, false);
    let options = crate::cast::payment_decision(
        state,
        player,
        source,
        cost.clone(),
        crate::priority::mana_options(state, player),
        crate::help::Offered::default(),
        spend,
        false,
        0,
    )?;
    crate::cast::auto_pay_events(state, player, &options).ok()
}

/// "Once each turn, you may pay {1} to reroll one or more dice you rolled" (CR 706.2b): each rule the roller can use now
/// asks which dice to reroll, none declining; the dice chosen get new results, and the costs of the rerolls chosen are
/// paid together, the mana chosen automatically, before the rerolls are logged. With `mana` false (a roll made while a
/// cost is paid, whose own payment is still open) rules whose cost has mana are passed over.
pub(crate) fn r706_reroll(
    (state, journal): (&GameState, &[(PlayerId, Choice)]),
    player: PlayerId,
    sides: u32,
    changed: &mut R6402Changed,
    mana: bool,
) -> Result<Result<(), Run>, ResolveError> {
    let mut owed: Vec<crate::events::Event> = Vec::new();
    let mut total = ManaCost::default();
    let mut first: Option<(ObjectId, usize)> = None;
    for reroller in crate::rules::r706_rerollers(state, player) {
        if changed.results.len() > R706_MOST
            || (reroller.once
                && state.object_tallied(reroller.source, None, Tally::R706Rerolled) > 0)
            || (!mana && !reroller.cost.symbols().is_empty())
        {
            continue;
        }
        let with = ManaCost::from_symbols(
            total
                .symbols()
                .iter()
                .chain(reroller.cost.symbols())
                .copied(),
        );
        if r706_payment(state, player, reroller.source.id, &with).is_none() {
            continue;
        }
        let options = R706DiceOptions {
            source: Some(reroller.source.id),
            roller: player,
            sides,
            results: changed.results.clone(),
            min: 0,
            max: changed.results.len(),
            purpose: R706DicePurpose::Reroll(reroller.cost.clone()),
            text: format!(
                "Pay {} to reroll the dice you choose, or choose none.",
                reroller.cost
            ),
        };
        let Some(dice) = r706_read(journal, changed, &options)? else {
            return Ok(Err(Run::Ask(Decision {
                player,
                kind: DecisionKind::R706ChooseDice(options),
            })));
        };
        changed.asked += 1;
        if dice.is_empty() {
            continue;
        }
        total = with;
        first.get_or_insert((reroller.source.id, changed.events.len()));
        let from = dice.iter().map(|die| changed.results[*die]).collect();
        for die in dice {
            let result = r706_draw(state, sides, changed);
            changed.results[die] = result;
            changed.natural[die] = result;
        }
        owed.push(Event::R706DiceRerolled {
            player,
            source: reroller.source,
            from,
        });
    }
    if let Some((source, at)) = first {
        let paid = if total.symbols().is_empty() {
            Vec::new()
        } else {
            r706_payment(state, player, source, &total).ok_or_else(|| {
                ResolveError::Invalid(format!("{player:?} can't pay {total} to reroll"))
            })?
        };
        let tail = changed.events.split_off(at);
        changed.events.extend(paid);
        changed.events.extend(owed);
        changed.events.extend(tail);
    }
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    //! Goal: the dice chosen travel through the journal as a bit set and back for every subset of every roll size the
    //! question offers, and an answer that names a die twice, a die past the roll, or too few or too many is refused.
    use rstest::rstest;

    use super::*;

    fn options(count: usize, min: usize, max: usize) -> R706DiceOptions {
        R706DiceOptions {
            source: None,
            roller: PlayerId(0),
            sides: 6,
            results: (1..=u32::try_from(count).unwrap()).collect(),
            min,
            max,
            purpose: R706DicePurpose::Ignore,
            text: String::new(),
        }
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(3)]
    #[case(5)]
    fn r706_every_subset_round_trips(#[case] count: usize) {
        for mask in 0..(1_usize << count) {
            let dice = r706_unmask(mask, count).unwrap();
            assert_eq!(r706_mask(&dice), mask);
            assert!(r706_check(&options(count, 0, count), &dice).is_ok());
        }
        assert_eq!(r706_unmask(1 << count, count), None);
    }

    #[rstest]
    #[case::twice(vec![0, 0], 0, 3)]
    #[case::past_the_roll(vec![3], 0, 3)]
    #[case::too_few(vec![], 1, 1)]
    #[case::too_many(vec![0, 1], 1, 1)]
    fn r706_a_wrong_answer_is_refused(
        #[case] dice: Vec<usize>,
        #[case] min: usize,
        #[case] max: usize,
    ) {
        assert!(r706_check(&options(3, min, max), &dice).is_err());
        assert!(r706_answer(PlayerId(0), &options(3, min, max), &dice, false).is_err());
    }
}
