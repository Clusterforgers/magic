//! A filter that reads the X a spell announces ("Exile a red card with mana value X from your hand"): whether it does,
//! and the filter once X is known.

use crate::refs::{Pred, Selector};
use crate::value::Amount;

/// Whether a predicate compares a characteristic with X, directly or inside `Not` or `AnyOf`.
fn reads_x(pred: &Pred) -> bool {
    match pred {
        Pred::Compare { to, .. } => *to == Amount::X,
        Pred::Not(inner) => reads_x(inner),
        Pred::AnyOf(any) => any.iter().any(reads_x),
        _ => false,
    }
}

/// The predicate with X read as `x`.
fn with_x(pred: &Pred, x: i32) -> Pred {
    match pred {
        Pred::Compare {
            stat,
            op,
            to: Amount::X,
        } => Pred::Compare {
            stat: *stat,
            op: *op,
            to: Amount::Fixed(x),
        },
        Pred::Not(inner) => Pred::Not(Box::new(with_x(inner, x))),
        Pred::AnyOf(any) => Pred::AnyOf(any.iter().map(|inner| with_x(inner, x)).collect()),
        other => other.clone(),
    }
}

impl Selector {
    /// Whether the selector compares a characteristic with X ("a red card with mana value X").
    pub fn r301_reads_x(&self) -> bool {
        self.0.iter().any(reads_x)
    }

    /// The selector with X read as `x`.
    #[must_use]
    pub fn r301_with_x(&self, x: i32) -> Selector {
        Selector(self.0.iter().map(|pred| with_x(pred, x)).collect())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a filter reads X exactly when a comparison in it, nested or not, is with X, and fixing X changes only those comparisons.
    use rstest::rstest;

    use crate::refs::Selector;

    fn sel(json: &str) -> Selector {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::mana_value_x(
        r#"[{"Color":"Red"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]"#,
        true
    )]
    #[case::inside_not(r#"[{"Not":{"Compare":{"stat":"Power","op":"Le","to":"X"}}}]"#, true)]
    #[case::inside_any_of(
        r#"[{"AnyOf":[{"Type":"Land"},{"Compare":{"stat":"ManaValue","op":"Ge","to":"X"}}]}]"#,
        true
    )]
    #[case::a_number(r#"[{"Compare":{"stat":"ManaValue","op":"Eq","to":3}}]"#, false)]
    #[case::no_comparison(r#"[{"Color":"Red"}]"#, false)]
    fn r301_a_filter_reads_x(#[case] json: &str, #[case] reads: bool) {
        let filter = sel(json);
        assert_eq!(filter.r301_reads_x(), reads);
        assert!(!filter.r301_with_x(4).r301_reads_x());
        if !reads {
            assert_eq!(filter.r301_with_x(4), filter);
        }
    }

    #[test]
    fn r301_fixing_x_puts_the_number_in_its_place() {
        let filter =
            sel(r#"[{"Color":"Red"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]"#);
        assert_eq!(
            filter.r301_with_x(3),
            sel(r#"[{"Color":"Red"},{"Compare":{"stat":"ManaValue","op":"Eq","to":3}}]"#)
        );
    }
}
