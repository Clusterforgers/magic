//! Votes on named choices (CR 701.38): "Starting with you, each player votes for grace or condemnation. If grace gets more
//! votes, S. If condemnation gets more votes or the vote is tied, S." is a `Vote` and an `If` on the sizes of the two
//! names; "For each evidence vote, S. For each bribery vote, S." is a `RepeatEach` over each name; "Each player secretly
//! votes for truth or consequences, then those votes are revealed." is the vote cast face down.

use super::conditional::body;
use super::tokens::Reader;
use crate::effect::{Effect, VoteFor};
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition};

/// The name the voter of a pass is bound to in the effects "for each A vote" does.
pub const VOTER: &str = "voter";

/// A choice of a vote: one lowercase word ("grace", "time"), the name its votes are bound to.
fn choice(r: &mut Reader<'_>) -> Option<String> {
    let word = r.take_word()?;
    word.chars()
        .all(char::is_alphabetic)
        .then(|| word.to_owned())
}

/// The vote and what the sentences after it do with the votes, when they are one of the two shapes.
pub fn named(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let ordered = r.words("starting with you");
    if ordered {
        r.punct(',').then_some(())?;
    }
    r.need("each player")?;
    let secret = r.word("secretly");
    // A vote cast in the open starts with the controller; a secret one is cast together.
    (ordered != secret).then_some(())?;
    r.need("votes for")?;
    let first = choice(r)?;
    r.need("or")?;
    let second = choice(r)?;
    (first != second).then_some(())?;
    if secret {
        r.punct(',').then_some(())?;
        r.need("then those votes are revealed")?;
    }
    let mut effects = vec![Effect::Vote {
        options: VoteFor::Named(vec![first.clone(), second.clone()]),
        secret,
        bind: None,
    }];
    if let Some(outcome) = r.attempt(|r| outcome(r, &first, &second)) {
        effects.push(outcome);
    } else if let Some(each) = r.attempt(|r| each_vote(r, &first, &second)) {
        effects.extend(each);
    }
    Some(effects)
}

/// The condition "A gets more votes": more votes for the first name than for the second.
pub fn more_votes(first: &str, second: &str) -> Condition {
    Condition::Compare {
        left: Amount::Players(Ref::Bound(first.to_owned())),
        op: Cmp::Gt,
        right: Amount::Players(Ref::Bound(second.to_owned())),
    }
}

/// ". If A gets more votes, S. If B gets more votes or the vote is tied, S".
fn outcome(r: &mut Reader<'_>, first: &str, second: &str) -> Option<Effect> {
    r.punct('.').then_some(())?;
    r.need("if")?;
    r.word(first).then_some(())?;
    r.need("gets more votes")?;
    r.punct(',').then_some(())?;
    let then = body(r)?;
    r.punct('.').then_some(())?;
    r.need("if")?;
    r.word(second).then_some(())?;
    r.need("gets more votes or the vote is tied")?;
    r.punct(',').then_some(())?;
    let otherwise = body(r)?;
    Some(Effect::If {
        condition: more_votes(first, second),
        then,
        otherwise,
    })
}

/// ". For each A vote, S. For each B vote, S".
fn each_vote(r: &mut Reader<'_>, first: &str, second: &str) -> Option<Vec<Effect>> {
    let mut passes = Vec::new();
    for name in [first, second] {
        r.punct('.').then_some(())?;
        r.need("for each")?;
        r.word(name).then_some(())?;
        r.need("vote")?;
        r.punct(',').then_some(())?;
        passes.push(Effect::RepeatEach {
            over: Ref::Bound(name.to_owned()),
            bind: VOTER.to_owned(),
            effects: body(r)?,
        });
    }
    Some(passes)
}
