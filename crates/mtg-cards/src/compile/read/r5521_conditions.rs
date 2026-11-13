//! r55-21 wordings of two player conditions: "the Ring has tempted you two or more times this game" (`RingTempted`) and
//! "you've completed Tomb of Annihilation" (`CompletedNamedDungeon`, "you haven't completed ..." its negation).

use super::Reader;
use super::amounts::{count_word, number_word};
use crate::refs::Ref;
use crate::value::Condition;

/// Reads one of the conditions after "if".
pub fn r5521_condition(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(|r| {
        if r.r376_text("the Ring has tempted you") {
            let times = number_word(r.take_word()?)?;
            r.need("or more times this game")?;
            return (2..=4).contains(&times).then_some(Condition::RingTempted {
                who: Ref::You,
                times,
            });
        }
        let known = crate::w11d_dungeon::w11d_dungeons().ok()?;
        // The name is read with the words before it, so its capitals tokenize as they do inside a sentence.
        let (dungeon, negated) = known.iter().find_map(|d| {
            [false, true].into_iter().find_map(|negated| {
                let verb = if negated { "haven't" } else { "'ve" };
                let words = if negated {
                    format!("you {verb} completed {}", d.name)
                } else {
                    format!("you{verb} completed {}", d.name)
                };
                r.r376_text(&words).then_some((d, negated))
            })
        })?;
        let completed = Condition::CompletedNamedDungeon {
            who: Ref::You,
            dungeon: dungeon.name.clone(),
        };
        Some(if negated {
            Condition::Not(Box::new(completed))
        } else {
            completed
        })
    })
}

/// The condition `r5521_condition` reads, written back.
pub fn r5521_condition_text(condition: &Condition) -> Option<String> {
    match condition {
        Condition::RingTempted {
            who: Ref::You,
            times,
        } if (2..=4).contains(times) => Some(format!(
            "the Ring has tempted you {} or more times this game",
            count_word(*times)?
        )),
        Condition::CompletedNamedDungeon {
            who: Ref::You,
            dungeon,
        } => Some(format!("you've completed {dungeon}")),
        Condition::Not(inner) => match inner.as_ref() {
            Condition::CompletedNamedDungeon {
                who: Ref::You,
                dungeon,
            } => Some(format!("you haven't completed {dungeon}")),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the Ring-temptation and named-dungeon conditions read and write back, and their near misses (one temptation,
    //! another player, an unknown dungeon) do not.
    use super::*;
    use crate::compile::read::tokens::tokenize;
    use rstest::rstest;

    fn read(text: &str) -> Option<Condition> {
        let tokens = tokenize(text);
        let mut r = Reader::new(&tokens, false);
        let found = r5521_condition(&mut r)?;
        r.end().then_some(found)
    }

    #[rstest]
    #[case("the Ring has tempted you two or more times this game")]
    #[case("the Ring has tempted you four or more times this game")]
    #[case("you've completed Tomb of Annihilation")]
    #[case("you haven't completed Tomb of Annihilation")]
    #[case("you haven't completed Lost Mine of Phandelver")]
    fn r5521_conditions_read_back(#[case] text: &str) {
        let condition = read(text).unwrap();
        assert_eq!(r5521_condition_text(&condition).as_deref(), Some(text));
    }

    #[rstest]
    #[case("the Ring has tempted you five or more times this game")]
    #[case("the Ring has tempted you two or more times")]
    #[case("you've completed Tomb of Horrors")]
    #[case("you haven't completed a dungeon")]
    fn r5521_near_misses_do_not_read(#[case] text: &str) {
        assert_eq!(read(text), None);
    }

    #[rstest]
    #[case(Condition::RingTempted { who: Ref::Opponents, times: 2 })]
    #[case(Condition::RingTempted { who: Ref::You, times: 5 })]
    #[case(Condition::CompletedNamedDungeon { who: Ref::Opponents, dungeon: "Tomb of Annihilation".into() })]
    fn r5521_other_shapes_have_no_words(#[case] condition: Condition) {
        assert_eq!(r5521_condition_text(&condition), None);
    }
}
