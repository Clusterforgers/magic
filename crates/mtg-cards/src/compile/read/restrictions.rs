//! The sentence that limits when an ability is activated, read after its effects: "Activate only as a sorcery.",
//! "Activate only during your upkeep.", "Activate only during your turn, before attackers are declared.", "Activate only
//! if you control a Swamp." and the parts joined by "and only" in any order ("Activate only during your turn and only if
//! there are at least three Zombie creature cards in your graveyard."). The parts become a timing, a limit and one
//! condition: the conditions read in order, an `All` when there are several. Written back by `activations.rs`.

use super::tokens::Reader;
use super::{as_long_as, statics};
use crate::ability::{Limit, Timing};
use crate::trigger::Step;
use crate::value::Condition;

/// What the sentence says, in the order of the parts.
pub(super) type Restrictions = (Timing, Option<Limit>, Option<Condition>);

/// A moment of the turn after "during", as the conditions that hold in it.
pub(super) fn moments() -> Vec<(&'static str, Vec<Condition>)> {
    vec![
        ("your turn", vec![Condition::YourTurn]),
        (
            "your upkeep",
            vec![Condition::YourTurn, Condition::Step(Step::Upkeep)],
        ),
        (
            "an opponent's turn",
            vec![Condition::Not(Box::new(Condition::YourTurn))],
        ),
        (
            "an opponent's upkeep",
            vec![
                Condition::Not(Box::new(Condition::YourTurn)),
                Condition::Step(Step::Upkeep),
            ],
        ),
        ("any upkeep step", vec![Condition::Step(Step::Upkeep)]),
        ("combat", vec![Condition::InCombat]),
        (
            "the declare attackers step",
            vec![Condition::Step(Step::DeclareAttackers)],
        ),
        (
            "the declare blockers step",
            vec![Condition::Step(Step::DeclareBlockers)],
        ),
        (
            "the end of combat step",
            vec![Condition::Step(Step::EndCombat)],
        ),
    ]
}

/// The words before the moment of a turn that "before attackers are declared" follows after a comma.
const BEFORE_ATTACKERS: &str = "before attackers are declared";

/// A part after "only": "as a sorcery", "once each turn", "during ...", "before attackers are declared" or "if ...".
fn part(
    r: &mut Reader<'_>,
    timing: &mut Timing,
    limit: &mut Option<Limit>,
    held: &mut Vec<Condition>,
) -> Option<()> {
    if *timing == Timing::Instant && r.words("as a sorcery") {
        *timing = Timing::Sorcery;
    } else if limit.is_none() && r.words("once each turn") {
        *limit = Some(Limit::OncePerTurn);
    } else if r.words(BEFORE_ATTACKERS) {
        held.push(Condition::BeforeAttackers);
    } else if r.word("during") {
        let (_, found) = moments()
            .into_iter()
            .find(|(words, _)| r.attempt(|r| r.words(words).then_some(())).is_some())?;
        held.extend(found);
        // "during your turn, before attackers are declared"
        if r.attempt(|r| (r.punct(',') && r.words(BEFORE_ATTACKERS)).then_some(()))
            .is_some()
        {
            held.push(Condition::BeforeAttackers);
        }
    } else if r.word("if") {
        let condition = r
            .attempt(statics::control)
            .or_else(|| r.attempt(|r| as_long_as::body(r, Some(&crate::refs::Ref::This))))?;
        held.push(condition);
    } else {
        return None;
    }
    Some(())
}

/// The restrictions after the effects of an activated ability: nothing, or "Activate only ..." and its parts.
pub(super) fn read(r: &mut Reader<'_>) -> Option<Restrictions> {
    if !r.words("activate only") {
        return Some((Timing::Instant, None, None));
    }
    let (mut timing, mut limit, mut held) = (Timing::Instant, None, Vec::new());
    loop {
        part(r, &mut timing, &mut limit, &mut held)?;
        if !r.words("and only") {
            break;
        }
    }
    let condition = match held.len() {
        0 => None,
        1 => held.pop(),
        _ => Some(Condition::All(held)),
    };
    r.sentence_end().then_some((timing, limit, condition))
}
