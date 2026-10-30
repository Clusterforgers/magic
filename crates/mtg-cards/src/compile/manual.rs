//! Stage 5: a clause the compiler does not read becomes a `Manual` ability whose reason tells the player what to do.

use super::ClauseKind;
use crate::ability::Ability;
use crate::card::Span;

/// What the player does with a clause of this kind, as the deck preview and the by-hand list read it.
pub const fn reason(kind: ClauseKind) -> &'static str {
    match kind {
        ClauseKind::Keyword => "you apply this keyword by hand",
        ClauseKind::Spell => "you resolve this spell effect by hand",
        ClauseKind::Triggered => "you apply this triggered ability by hand",
        ClauseKind::Activated => "you apply this activated ability by hand",
        ClauseKind::Loyalty => "you apply this loyalty ability by hand",
        ClauseKind::Modal => "you resolve this modal spell by hand",
        ClauseKind::Static => "you apply this static ability by hand",
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
    //! Goal: every kind of clause is left to the table with a reason that names the kind and tells the player to play it by hand.
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
        assert!(
            reason.starts_with("you ") && reason.ends_with("by hand"),
            "{reason}"
        );
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
