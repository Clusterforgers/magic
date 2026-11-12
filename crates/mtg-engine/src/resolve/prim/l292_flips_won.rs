//! The flips a `FlipCoin` has won, kept under its `won_as` name as a `Let` keeps a number ("flip a coin until you lose a
//! flip. Put a +1/+1 counter on ~ for each flip you won"): each won flip adds one, so the steps after the flips read
//! how many were won.

use mtg_cards::value::Amount;

use crate::choose::Target;
use crate::resolve::{Cx, LET, ResolveError};

/// Adds to `bound` the kept number of flips won, one more than before, when the flip was won and the effect names one.
pub(super) fn l292_count_win(
    cx: &Cx<'_>,
    won_as: Option<&str>,
    won: bool,
    bound: &mut Vec<(String, Vec<Target>)>,
) -> Result<(), ResolveError> {
    let (Some(name), true) = (won_as, won) else {
        return Ok(());
    };
    let before = cx.scope.amount(&Amount::Bound(name.to_owned()))?;
    bound.push((
        format!("{LET}{}:{name}", before.saturating_add(1)),
        Vec::new(),
    ));
    Ok(())
}
