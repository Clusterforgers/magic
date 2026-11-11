//! X named in a sentence of its own after the spell's last sentence: "~ deals X damage to target creature and you gain X
//! life. X is the number of creatures you control." reads as the same text with ", where X is ..." closing the sentence
//! before; the writers give that wording back (`where_x`).

/// The text with a closing "X is ...." sentence joined to the sentence before it as ", where X is ....".
pub(super) fn rmt0_joined(text: &str) -> Option<String> {
    let (head, named) = text.rsplit_once(". X is ")?;
    (named.ends_with('.') && !named.contains(". ") && !head.is_empty()).then_some(())?;
    Some(format!("{head}, where X is {named}"))
}

#[cfg(test)]
mod tests {
    //! Goal: only a last sentence "X is ..." joins the sentence before it.
    use super::rmt0_joined;
    use rstest::rstest;

    #[rstest]
    #[case::joined(
        "Draw X cards. X is the number of creatures you control.",
        Some("Draw X cards, where X is the number of creatures you control.")
    )]
    #[case::not_last(
        "Draw X cards. X is the number of creatures you control. Discard a card.",
        None
    )]
    #[case::none("Draw X cards, where X is the number of creatures you control.", None)]
    fn a_last_x_sentence_joins_the_one_before(#[case] text: &str, #[case] expected: Option<&str>) {
        assert_eq!(rmt0_joined(text).as_deref(), expected);
    }
}
