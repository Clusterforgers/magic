//! Goal: the sentence after the exile of a target that depends on the kind of the object exiled ("If it was a creature card, ...") reads into an exile that binds what it took and an `If` over the binding, writes back as the card words it, and a sentence that is not about the one target exiled, names no kind of card or says more than one condition stays with the table.
use super::*;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Condition;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::creature_card_counter_and_life(
    "{G}: Exile target card from a graveyard. If it was a creature card, put a +1/+1 counter on this creature and you gain 1 life."
)]
#[case::permanent_card_counter(
    "{W}: Exile target card from a graveyard. If it was a permanent card, put a +1/+1 counter on this permanent."
)]
#[case::creature_card_token(
    "{2}{W}{B}: Exile target card from a graveyard. If it was a creature card, create a 1/1 white Human Soldier creature token."
)]
fn the_kind_of_the_exiled_card_decides_what_follows(#[case] text: &str) {
    let Ability::Activated { effects, .. } = golden(text, ClauseKind::Activated, false) else {
        panic!("not an activated ability: {text}");
    };
    let [
        Effect::Exile {
            bind: Some(named), ..
        },
        Effect::If {
            condition:
                Condition::Matches {
                    subject: Ref::Bound(subject),
                    ..
                },
            otherwise,
            ..
        },
    ] = effects.as_slice()
    else {
        panic!("an exile that binds and an If over the binding: {effects:?}");
    };
    assert_eq!(named, subject);
    assert_eq!(otherwise.len(), 0);
}

#[rstest]
#[case::no_kind(
    "{G}: Exile target card from a graveyard. If it was tapped, put a +1/+1 counter on this creature."
)]
#[case::a_second_condition(
    "{G}: Exile target card from a graveyard. If it was a creature card, put a +1/+1 counter on this creature. If it was a land card, you gain 1 life."
)]
#[case::an_unread_consequence(
    "{G}: Exile target card from a graveyard. If it was a creature card, frobnicate."
)]
#[case::no_exile(
    "{G}: Draw a card. If it was a creature card, put a +1/+1 counter on this creature."
)]
fn only_the_exile_of_a_target_card_has_a_kind_to_ask(#[case] text: &str) {
    assert!(activated(text).is_none(), "{text}");
}
