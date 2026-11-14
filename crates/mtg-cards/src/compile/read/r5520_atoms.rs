//! Selector atoms of card pack r55-20: "that don't have a name" after the noun (a face-down creature has none, CR
//! 708.2a) and "foretold" before it (read in `w21a_atoms`).

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::Pred;

/// "that has a name", "that have a name", "that doesn't have a name", "that don't have a name".
pub(super) fn r5520_name_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        r.need("that")?;
        let has = if r.word("has") || r.word("have") {
            true
        } else {
            (r.words("doesn't have") || r.words("don't have")).then_some(())?;
            false
        };
        r.need("a name")?;
        Some(if has {
            Pred::HasName
        } else {
            Pred::Not(Box::new(Pred::HasName))
        })
    })
}

/// "with foretell": a card that has the keyword (CR 702.143), whatever its cost; the shared writer says it back.
pub(super) fn r5520_keyword_named(r: &mut Reader<'_>) -> Option<Pred> {
    r.words("with foretell")
        .then(|| Pred::HasKeywordNamed("Foretell".to_owned()))
}

/// The words `r5520_name_suffix` reads, in the number of the phrase.
pub(super) fn r5520_name_suffix_text(has: bool, form: Form) -> String {
    match (has, form.plural()) {
        (true, true) => "that have a name",
        (true, false) => "that has a name",
        (false, true) => "that don't have a name",
        (false, false) => "that doesn't have a name",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    //! Goal: the name atom reads its four wordings and none of their near misses, and writes back in the phrase's number.
    use super::*;
    use crate::compile::read::tokens::tokenize;
    use rstest::rstest;

    fn read(text: &str) -> Option<Pred> {
        let tokens = tokenize(text);
        r5520_name_suffix(&mut Reader::new(&tokens, false))
    }

    #[rstest]
    #[case("that has a name", Some(Pred::HasName))]
    #[case("that have a name", Some(Pred::HasName))]
    #[case("that doesn't have a name", Some(Pred::Not(Box::new(Pred::HasName))))]
    #[case("that don't have a name", Some(Pred::Not(Box::new(Pred::HasName))))]
    #[case("that has the name", None)]
    #[case("that don't have names", None)]
    #[case("that have a name sticker", Some(Pred::HasName))]
    #[case("with a name", None)]
    fn the_name_atom_reads_its_wordings(#[case] text: &str, #[case] expected: Option<Pred>) {
        assert_eq!(read(text), expected);
    }

    #[rstest]
    #[case("with foretell", true)]
    #[case("with foretold", false)]
    #[case("without foretell", false)]
    fn with_foretell_is_the_named_keyword(#[case] text: &str, #[case] read: bool) {
        let tokens = tokenize(text);
        let found = r5520_keyword_named(&mut Reader::new(&tokens, false));
        assert_eq!(found.is_some(), read);
    }

    #[rstest]
    #[case(true, Form::All, "that have a name")]
    #[case(true, Form::Each, "that has a name")]
    #[case(false, Form::All, "that don't have a name")]
    #[case(false, Form::Target, "that doesn't have a name")]
    fn the_name_atom_writes_in_number(#[case] has: bool, #[case] form: Form, #[case] text: &str) {
        assert_eq!(r5520_name_suffix_text(has, form), text);
        assert!(read(text).is_some());
    }
}
