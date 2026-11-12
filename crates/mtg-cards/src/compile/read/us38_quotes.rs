//! A quoted ability inside another quoted ability: cards write the inner one in single quotes with its period inside
//! ("gains "{2}, {T}: Create a ... token with 'This token gets +1/+1 for each artifact you control.'"").

/// Adds, for each wording that quotes an ability with a quotation inside it, the wording with the inner quotes single.
pub(super) fn us38_nested_wordings(texts: &mut Vec<String>) {
    let nested: Vec<String> = texts.iter().filter_map(|text| us38_nested(text)).collect();
    for text in nested {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The wording with the quotes inside its outermost quotes single; `None` when nothing is quoted inside a quote.
fn us38_nested(text: &str) -> Option<String> {
    let open = text.find('"')?;
    let close = text.rfind('"')?;
    let inner = text.get(open + 1..close)?;
    (inner.matches('"').count() >= 2).then_some(())?;
    // The inner quote's period goes inside it: `control.".` is `control.'`.
    let inner = match inner.strip_suffix("\".") {
        Some(head) if head.ends_with('.') => format!("{head}\""),
        _ => inner.to_owned(),
    };
    Some(format!(
        "{}\"{}\"{}",
        text.get(..open)?,
        inner.replace('"', "'"),
        text.get(close + 1..)?
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: a wording with a quotation inside a quotation gains the wording with single inner quotes, and a wording with one level of quotes gains nothing.
    use rstest::rstest;

    use super::us38_nested;

    #[rstest]
    #[case::token(
        "~ gains \"{T}: Create a token with \"~ gets +1/+1.\".\".",
        Some("~ gains \"{T}: Create a token with '~ gets +1/+1.'\".")
    )]
    #[case::inner_period_outside(
        "~ has \"{T}: Target land gains \"{T}: Add {C}\".\"",
        Some("~ has \"{T}: Target land gains '{T}: Add {C}'.\"")
    )]
    #[case::flat("~ gains \"{T}: Add {C}.\"", None)]
    #[case::unquoted("Draw a card.", None)]
    fn nested_quotes_turn_single(#[case] text: &str, #[case] expected: Option<&str>) {
        assert_eq!(us38_nested(text).as_deref(), expected);
    }
}
