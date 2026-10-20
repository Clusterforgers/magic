//! Stage 5: a clause the compiler does not read becomes a `Manual` ability with a reason in plain words.

use super::ClauseKind;
use crate::ability::Ability;
use crate::card::Span;

/// Why a clause of this kind is left to the table.
pub const fn reason(kind: ClauseKind) -> &'static str {
    match kind {
        ClauseKind::Keyword => "a keyword line the compiler does not read yet",
        ClauseKind::Spell => "a spell effect the compiler does not read yet",
        ClauseKind::Triggered => "a triggered ability the compiler does not read yet",
        ClauseKind::Activated => "an activated ability the compiler does not read yet",
        ClauseKind::Loyalty => "a loyalty ability the compiler does not read yet",
        ClauseKind::Modal => "a modal spell the compiler does not read yet",
        ClauseKind::Static => "a static ability the compiler does not read yet",
    }
}

/// The ability that leaves the clause at `span` to the table.
pub fn manual(kind: ClauseKind, span: Span) -> Ability {
    Ability::Manual {
        reason: reason(kind).to_owned(),
        span,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every kind of clause is left to the table with a reason in plain words that names the kind.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(ClauseKind::Keyword, "keyword")]
    #[case(ClauseKind::Spell, "spell")]
    #[case(ClauseKind::Triggered, "triggered")]
    #[case(ClauseKind::Activated, "activated")]
    #[case(ClauseKind::Loyalty, "loyalty")]
    #[case(ClauseKind::Modal, "modal")]
    #[case(ClauseKind::Static, "static")]
    fn the_reason_names_the_kind(#[case] kind: ClauseKind, #[case] word: &str) {
        let reason = reason(kind);
        assert!(reason.contains(word), "{reason}");
        assert!(reason.ends_with("does not read yet"));
        let span = Span { start: 3, end: 9 };
        assert_eq!(
            manual(kind, span),
            Ability::Manual {
                reason: reason.to_owned(),
                span
            }
        );
    }
}
