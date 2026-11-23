//! Payments of the r117-04 pack: a payment sized by its target, and every sentence after a payment's "When you do".
//!
//! "You may pay {X}. If you do, return another target Zombie creature card with mana value X ...": the target is chosen
//! as the ability is put on the stack, before X is announced as the payment is made (CR 601.2c, 603.3d), so the
//! only payment that does anything is the target's own mana value. The ability pays {1} once for each point of it.

use crate::ability::Ability;
use crate::cost::Cost;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, TargetSpec, Targetable};
use crate::value::{Amount, Cmp, StatKind};

/// The payment of `{1}` times the stat of the one target taken since `from` that compares its stat with the X paid,
/// with that comparison taken out of the target; `None` unless the cost is `{X}` alone.
pub(super) fn r117_04_sized(
    targets: &mut [TargetSpec],
    from: usize,
    cost: &Cost,
) -> Option<(Cost, Amount)> {
    mana_is(cost, "{X}").then_some(())?;
    let mut found = None;
    for (at, target) in targets.iter_mut().enumerate().skip(from) {
        let Targetable::Object(selector) = &mut target.what else {
            continue;
        };
        let Some(spot) = selector.0.iter().position(|pred| stat_of_x(pred).is_some()) else {
            continue;
        };
        found.is_none().then_some(())?;
        let stat = stat_of_x(&selector.0[spot])?;
        selector.0.remove(spot);
        found = Some((at, stat));
    }
    let (at, stat) = found?;
    let one = Cost::mana("{1}".parse().ok()?);
    Some((
        one,
        Amount::Stat {
            of: Ref::Target(u8::try_from(at).ok()?),
            stat,
        },
    ))
}

/// The stat a predicate compares with an equal X.
fn stat_of_x(pred: &Pred) -> Option<StatKind> {
    match pred {
        Pred::Compare {
            stat,
            op: Cmp::Eq,
            to: Amount::X,
        } => Some(*stat),
        _ => None,
    }
}

/// Whether the cost is exactly this mana.
fn mana_is(cost: &Cost, mana: &str) -> bool {
    serde_json::to_value(cost).is_ok_and(|value| value == serde_json::json!(mana))
}

/// Adds the wordings of a triggered ability that pays `{1}` once for each point of its target's stat: those of the
/// ability as the words say it, `{X}` paid and the target with that stat X.
pub(super) fn r117_04_sized_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Triggered {
        targets, effects, ..
    } = ability
    else {
        return;
    };
    let Some(at) = effects.iter().position(|effect| {
        matches!(effect, Effect::Pay { times: Amount::Stat { of: Ref::Target(_), .. }, cost, .. } if mana_is(cost, "{1}"))
    }) else {
        return;
    };
    let mut worded = effects.clone();
    let Some(Effect::Pay { cost, times, .. }) = worded.get_mut(at) else {
        return;
    };
    let Amount::Stat {
        of: Ref::Target(index),
        stat,
    } = times.clone()
    else {
        return;
    };
    let Some(x) = "{X}".parse().ok().map(Cost::mana) else {
        return;
    };
    *cost = x;
    *times = Amount::one();
    let Some(TargetSpec {
        what: Targetable::Object(selector),
        ..
    }) = targets.get(usize::from(index))
    else {
        return;
    };
    // The comparison may stand anywhere among the target's words.
    for spot in 0..=selector.0.len() {
        let mut with_x = targets.clone();
        if let Some(TargetSpec {
            what: Targetable::Object(held),
            ..
        }) = with_x.get_mut(usize::from(index))
        {
            held.0.insert(
                spot,
                Pred::Compare {
                    stat,
                    op: Cmp::Eq,
                    to: Amount::X,
                },
            );
        }
        let mut twin = ability.clone();
        if let Ability::Triggered {
            targets, effects, ..
        } = &mut twin
        {
            *targets = with_x;
            effects.clone_from(&worded);
        }
        for text in written(&twin) {
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: "you may pay {X}. If you do, ... target ... with mana value X" reads as {1} paid once for each point of the
    //! target's mana value and writes back as the same text; another cost or a fixed mana value is not that text.
    #![allow(clippy::unwrap_used)]
    use crate::ability::Ability;
    use crate::compile::ClauseKind;
    use crate::compile::read::clause;
    use crate::compile::read::render::written_variants;
    use rstest::rstest;

    const TEXT: &str = "When ~ dies, you may pay {X}. If you do, return another target Zombie creature card with mana value X from your graveyard to the battlefield.";
    const JSON: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Subtype":"Zombie"},"Other",{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Pay":{"cost":"{1}","times":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"paid":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}]}}"#;

    #[test]
    fn r117_04_a_payment_sized_by_its_target_reads_and_writes_back() {
        let read = clause(TEXT, ClauseKind::Triggered, false).unwrap();
        assert_eq!(
            serde_json::to_value(&read).unwrap(),
            serde_json::from_str::<serde_json::Value>(JSON).unwrap()
        );
        let texts = written_variants(&read);
        assert!(texts.iter().any(|t| t == TEXT), "{texts:#?}");
    }

    #[rstest]
    #[case::two_each(&JSON.replace(r#""cost":"{1}""#, r#""cost":"{2}""#))]
    #[case::power(&JSON.replace(r#""stat":"ManaValue""#, r#""stat":"Power""#))]
    fn r117_04_another_payment_is_not_the_text(#[case] json: &str) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let texts = written_variants(&ability);
        assert!(
            !texts.iter().any(|t| t.contains("with mana value X")),
            "{texts:#?}"
        );
    }

    /// Every sentence after a payment's "When you do" is the reflexive trigger (CR 603.12), so "it" there names its
    /// target; a sentence the reader cannot read leaves the whole ability unread rather than outside the trigger.
    #[rstest]
    #[case::two_sentences(
        "Whenever you attack, you may discard a card. When you do, put a +1/+1 counter on target attacking creature. It gains trample until end of turn.",
        true
    )]
    #[case::an_unread_sentence(
        "Whenever you attack, you may discard a card. When you do, put a +1/+1 counter on target attacking creature. It gains the colors of your choice forever.",
        false
    )]
    fn r117_04_a_reflexive_trigger_keeps_every_sentence(#[case] text: &str, #[case] reads: bool) {
        let read = clause(text, ClauseKind::Triggered, false);
        assert_eq!(read.is_some(), reads, "{text}");
        if let Some(Ability::Triggered { effects, .. }) = read {
            let json = serde_json::to_string(&effects).unwrap();
            assert!(
                json.starts_with(r#"[{"Pay""#) && json.contains("Trample"),
                "{json}"
            );
            assert_eq!(effects.len(), 1, "{json}");
        }
    }
}
