//! "Put up to three lore counters on it": a number from none to the count chosen as the step resolves (`ChooseValue`
//! of a `Number`), then that many counters put (`PutCounters` of `ChosenAs` it), read and written as one sentence.

use crate::effect::{Effect, ValueKind};
use crate::refs::Ref;
use crate::value::{Amount, CounterKind};

/// The name the chosen number is kept under.
pub(super) const R121_UP_TO: &str = "up_to";

/// The two steps of "put up to N <kind> counters on <place>".
pub(super) fn r121_up_to_steps(on: Ref, counter: CounterKind, most: Amount) -> Vec<Effect> {
    vec![
        Effect::ChooseValue {
            who: Ref::You,
            kind: ValueKind::Number(Amount::Fixed(0), most),
            bind: R121_UP_TO.to_owned(),
        },
        Effect::PutCounters {
            on,
            counter,
            count: Amount::ChosenAs(R121_UP_TO.to_owned()),
        },
    ]
}

/// The put of the most counters that a choice from none and the put of the number chosen stand for: written as that put
/// with "up to" before its count.
pub(super) fn r121_up_to_put(choose: &Effect, put: &Effect) -> Option<Effect> {
    let (
        Effect::ChooseValue {
            who: Ref::You,
            kind: ValueKind::Number(Amount::Fixed(0), most @ Amount::Fixed(n)),
            bind,
        },
        Effect::PutCounters {
            on,
            counter,
            count: Amount::ChosenAs(chosen),
        },
    ) = (choose, put)
    else {
        return None;
    };
    (bind == R121_UP_TO && chosen == R121_UP_TO && *n >= 1).then(|| Effect::PutCounters {
        on: on.clone(),
        counter: counter.clone(),
        count: most.clone(),
    })
}

/// The sentence of the put of the most counters with "up to" before the count: "put up to three lore counters on it".
pub(super) fn r121_up_to_text(text: &str) -> Option<String> {
    ["put ", "Put "].iter().find_map(|verb| {
        text.strip_prefix(verb)
            .map(|rest| format!("{verb}up to {rest}"))
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the two steps of "put up to N counters" are written as the put of N with "up to" before the count, and a
    //! choice from another least number, of another name or with a count of the board is not.
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn r121_the_steps_stand_for_the_put_of_the_most() {
        let [choose, put] = <[Effect; 2]>::try_from(r121_up_to_steps(
            Ref::This,
            CounterKind::Other("lore".to_owned()),
            Amount::Fixed(3),
        ))
        .unwrap();
        let most = r121_up_to_put(&choose, &put).unwrap();
        assert!(matches!(
            most,
            Effect::PutCounters {
                count: Amount::Fixed(3),
                ..
            }
        ));
        let Effect::ChooseValue { who, bind, .. } = choose.clone() else {
            panic!("a choice");
        };
        let from_one = Effect::ChooseValue {
            who: who.clone(),
            kind: ValueKind::Number(Amount::Fixed(1), Amount::Fixed(3)),
            bind: bind.clone(),
        };
        assert_eq!(r121_up_to_put(&from_one, &put), None);
        let named = Effect::ChooseValue {
            who: who.clone(),
            kind: ValueKind::Number(Amount::Fixed(0), Amount::Fixed(3)),
            bind: "n".to_owned(),
        };
        assert_eq!(r121_up_to_put(&named, &put), None);
        let counted = Effect::ChooseValue {
            who,
            kind: ValueKind::Number(Amount::Fixed(0), Amount::X),
            bind,
        };
        assert_eq!(r121_up_to_put(&counted, &put), None);
    }

    #[test]
    fn r121_up_to_goes_before_the_count() {
        assert_eq!(
            r121_up_to_text("put three lore counters on it").as_deref(),
            Some("put up to three lore counters on it")
        );
        assert_eq!(
            r121_up_to_text("Put two +1/+1 counters on target creature").as_deref(),
            Some("Put up to two +1/+1 counters on target creature")
        );
        assert_eq!(r121_up_to_text("you get two poison counters"), None);
    }
}
