//! Counters of any kind: "Remove a counter from a permanent you control" as a cost and "Remove up to three counters
//! from target permanent" as an effect, where no kind is named.

use super::amounts::{count_word, number_word};
use super::counters::{kind, text};
use super::tokens::Reader;
use crate::value::{Amount, CounterKind};

/// "a counter", "three counters", "X counters", "a +1/+1 counter": how many counters of what kind, `None` for a
/// counter of any kind.
pub(super) fn w2f_counters(r: &mut Reader<'_>) -> Option<(Option<CounterKind>, Amount)> {
    let (count, number) = if r.x() {
        (Amount::X, None)
    } else {
        let number = number_word(r.take_word()?)?;
        (Amount::Fixed(i32::try_from(number).ok()?), Some(number))
    };
    let counter = r.attempt(kind);
    r.word(if number == Some(1) {
        "counter"
    } else {
        "counters"
    })
    .then_some(())?;
    Some((counter, count))
}

/// The words of [`w2f_counters`]: "a counter", "three counters", or those of a named kind.
pub(super) fn w2f_text(counter: Option<&CounterKind>, count: &Amount) -> Option<String> {
    if let Some(counter) = counter {
        return text(counter, count);
    }
    Some(match count {
        Amount::X => "X counters".to_owned(),
        Amount::Fixed(1) => "a counter".to_owned(),
        Amount::Fixed(number) => format!("{} counters", count_word(u32::try_from(*number).ok()?)?),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a count of counters reads with or without a kind and renders back to the same words; counts the cards never
    //! print (zero, the triggering amount without a kind) are not read or rendered.
    use rstest::rstest;

    use super::*;
    use crate::compile::read::tokens::tokenize;

    fn reader_of<T>(words: &str, read: impl Fn(&mut Reader<'_>) -> Option<T>) -> Option<T> {
        let tokens = tokenize(words);
        let mut reader = Reader::new(&tokens, true);
        read(&mut reader).filter(|_| reader.end())
    }

    #[rstest]
    #[case::a_counter("a counter", None, Amount::Fixed(1))]
    #[case::three_counters("three counters", None, Amount::Fixed(3))]
    #[case::a_named_kind("a +1/+1 counter", Some(CounterKind::PlusOnePlusOne), Amount::Fixed(1))]
    #[case::two_charge("two charge counters", Some(CounterKind::Charge), Amount::Fixed(2))]
    fn counts_read_and_render_back(
        #[case] words: &str,
        #[case] counter: Option<CounterKind>,
        #[case] count: Amount,
    ) {
        let read = reader_of(words, w2f_counters);
        assert_eq!(read, Some((counter.clone(), count.clone())));
        assert_eq!(w2f_text(counter.as_ref(), &count).as_deref(), Some(words));
    }

    #[rstest]
    #[case::number_disagrees("a counters")]
    #[case::plural_disagrees("three counter")]
    #[case::no_count("counters")]
    fn near_misses_are_not_read(#[case] words: &str) {
        assert_eq!(reader_of(words, w2f_counters), None);
    }

    #[rstest]
    #[case::zero(Amount::Fixed(0))]
    #[case::triggering(Amount::Triggering)]
    fn counts_never_printed_are_not_rendered(#[case] count: Amount) {
        assert_eq!(w2f_text(None, &count), None);
    }
}
