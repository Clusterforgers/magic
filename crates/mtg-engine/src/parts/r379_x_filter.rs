//! A part of a cost whose filter reads the X announced ("Exile a red card with mana value X from your hand"): the X is
//! announced before the part is picked, and the filter takes it as a number (CR 601.2b).

use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::Amount;

use super::{Part, Pick};

/// Whether some predicate of `filter` compares to the X announced.
pub(crate) fn reads_x(filter: &Selector) -> bool {
    filter.0.iter().any(pred_reads_x)
}

fn pred_reads_x(pred: &Pred) -> bool {
    match pred {
        Pred::Compare { to, .. } => *to == Amount::X,
        Pred::Not(inner) => pred_reads_x(inner),
        Pred::AnyOf(preds) => preds.iter().any(pred_reads_x),
        _ => false,
    }
}

/// `filter` with the X it compares to fixed at `x`.
fn fixed(filter: &Selector, x: u32) -> Selector {
    Selector(filter.0.iter().map(|pred| fixed_pred(pred, x)).collect())
}

fn fixed_pred(pred: &Pred, x: u32) -> Pred {
    match pred {
        Pred::Compare { stat, op, to } if *to == Amount::X => Pred::Compare {
            stat: *stat,
            op: *op,
            to: Amount::Fixed(i32::try_from(x).unwrap_or(i32::MAX)),
        },
        Pred::Not(inner) => Pred::Not(Box::new(fixed_pred(inner, x))),
        Pred::AnyOf(preds) => Pred::AnyOf(preds.iter().map(|p| fixed_pred(p, x)).collect()),
        other => other.clone(),
    }
}

impl Part {
    /// Whether the part needs the X announced before it is picked: it counts X, or its filter compares to X.
    pub(crate) fn reads_x(&self) -> bool {
        self.is_counted() || self.r379_filter_reads_x()
    }

    /// Whether the part picks objects by a filter that compares to X.
    pub(crate) fn r379_filter_reads_x(&self) -> bool {
        match self {
            Part::Pick(pick) => reads_x(&pick.filter),
            Part::X(inner) => inner.r379_filter_reads_x(),
            _ => false,
        }
    }

    /// The part with the X of its filter fixed at `x`; a part whose filter reads no X stays as it is.
    pub(super) fn r379_fix_filter(self, x: u32) -> Part {
        match self {
            Part::Pick(pick) if reads_x(&pick.filter) => Part::Pick(Pick {
                filter: fixed(&pick.filter, x),
                ..pick
            }),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a cost filter that compares to X takes the announced X, and one that does not stays unchanged.
    use rstest::rstest;

    use super::*;
    use crate::parts::Kind;

    fn pick(filter: &str) -> Part {
        Part::Pick(Pick {
            kind: Kind::ExileHand,
            filter: serde_json::from_str(filter).unwrap(),
            count: 1,
        })
    }

    #[rstest]
    #[case::mana_value_x(
        r#"[{"Color":"Red"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]"#,
        true,
        r#"[{"Color":"Red"},{"Compare":{"stat":"ManaValue","op":"Eq","to":3}}]"#
    )]
    #[case::under_not(
        r#"[{"Not":{"Compare":{"stat":"Power","op":"Le","to":"X"}}}]"#,
        true,
        r#"[{"Not":{"Compare":{"stat":"Power","op":"Le","to":3}}}]"#
    )]
    #[case::near_miss_a_fixed_number(
        r#"[{"Compare":{"stat":"ManaValue","op":"Eq","to":2}}]"#,
        false,
        r#"[{"Compare":{"stat":"ManaValue","op":"Eq","to":2}}]"#
    )]
    #[case::near_miss_no_compare(r#"[{"Color":"Blue"}]"#, false, r#"[{"Color":"Blue"}]"#)]
    fn a_filter_takes_the_announced_x(
        #[case] filter: &str,
        #[case] reads: bool,
        #[case] at_three: &str,
    ) {
        let part = pick(filter);
        assert_eq!(part.reads_x(), reads);
        assert_eq!(part.r379_filter_reads_x(), reads);
        assert_eq!(part.at(3), pick(at_three));
    }
}
