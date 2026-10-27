//! A win at a number of counters: "When this artifact has twenty or more charge counters on it, you win the game."
//! No renderer writes a state trigger on the counters of the card.

use crate::ability::Ability;
use crate::compile::read::amounts::count_word;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// The name of a kind of counter as a card prints it; `None` for the kinds whose name is not one word.
fn c5_af_counter_name(counter: &CounterKind) -> Option<&str> {
    match counter {
        CounterKind::Charge => Some("charge"),
        CounterKind::Loyalty => Some("loyalty"),
        CounterKind::Poison => Some("poison"),
        CounterKind::Other(name) => Some(name),
        CounterKind::PlusOnePlusOne | CounterKind::MinusOneMinusOne => None,
    }
}

/// Adds the wording of a win when the card holds a number of counters of a kind.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::State {
                condition:
                    Condition::Compare {
                        left:
                            Amount::Counters {
                                on: Ref::This,
                                counter,
                            },
                        op: Cmp::Ge,
                        right: Amount::Fixed(number),
                    },
            },
        intervening_if: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [Effect::WinGame { who: Ref::You }] = effects.as_slice() else {
        return;
    };
    let (Some(name), Some(word), true) = (
        c5_af_counter_name(counter),
        u32::try_from(*number).ok().and_then(count_word),
        targets.is_empty(),
    ) else {
        return;
    };
    texts.push(format!(
        "When ~ has {word} or more {name} counters on it, you win the game."
    ));
}
