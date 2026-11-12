//! The wording of effects that happen as a permanent enters (`ReplaceBy::AsEnters`): "As ~ enters, discard your hand".

use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Pred;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, CounterKind};

/// "As ~ enters, <the effects as a spell writes them>", without the closing period; empty when no writer reads them.
pub(super) fn texts(effects: &[Effect]) -> Vec<String> {
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: effects.to_vec(),
    };
    // The same steps as an enters trigger writes them ("When ~ enters, sacrifice all lands you control").
    let trigger = Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: effects.to_vec(),
    };
    let triggered = super::render::written_variants(&trigger)
        .into_iter()
        .filter_map(|text| text.strip_prefix("When ~ enters, ").map(str::to_owned));
    let mut written: Vec<String> = chosen_counter_text(effects).into_iter().collect();
    for text in super::render::written_variants(&spell)
        .into_iter()
        .chain(triggered)
    {
        if !written.contains(&text) {
            written.push(text);
        }
    }
    written
        .iter()
        .filter_map(|text| {
            let body = text.strip_suffix('.')?;
            let mut chars = body.chars();
            let first = chars.next()?;
            Some(format!(
                "As ~ enters, {}{}",
                first.to_lowercase(),
                chars.as_str()
            ))
        })
        .collect()
}

/// "As ~ enters, <steps>. ~ enters with a +1/+1 counter on it for each <thing> this way" for the steps whose count
/// the printed cards say; empty for any other shape.
pub(super) fn with_texts(effects: &[Effect], counter: &CounterKind, count: &Amount) -> Vec<String> {
    if *counter != CounterKind::PlusOnePlusOne {
        return Vec::new();
    }
    let removed = matches!(
        (effects, count),
        (
            [
                Effect::Let { bind, amount: Amount::AnyCounters(Ref::Each(tallied)) },
                Effect::RemoveAllCounters { from: Ref::Each(all), counter: None },
            ],
            Amount::Bound(read),
        ) if bind == read && tallied.is_empty() && all.is_empty()
    );
    if removed {
        return vec![
            "As ~ enters, remove all counters from all permanents. ~ enters with a +1/+1 counter on it for each \
             counter removed this way"
                .to_owned(),
        ];
    }
    revealed_text(effects, count)
        .into_iter()
        .chain(if_you_do_texts(effects, count))
        .collect()
}

/// "As ~ enters, you may return a nonland permanent you control to its owner's hand. If you do, ~ enters with a +1/+1
/// counter on it.": one object chosen inside a "may", and one counter for it.
fn if_you_do_texts(effects: &[Effect], count: &Amount) -> Vec<String> {
    let (
        [
            Effect::May {
                who: Ref::You,
                effects: inner,
            },
        ],
        Amount::CountOf {
            of: Ref::Bound(counted),
            filter,
        },
    ) = (effects, count)
    else {
        return Vec::new();
    };
    let chosen_once = inner.iter().any(|step| {
        matches!(step, Effect::Choose { bind, count, rest_as: None, .. } if bind == counted && count.is_one())
    });
    if !chosen_once || !filter.is_empty() {
        return Vec::new();
    }
    super::render::written_variants(&Ability::Spell {
        targets: Vec::new(),
        effects: inner.clone(),
    })
    .iter()
    .filter_map(|text| {
        let body = text.strip_suffix('.')?;
        let mut chars = body.chars();
        let first = chars.next()?;
        Some(format!(
            "As ~ enters, you may {}{}. If you do, ~ enters with a +1/+1 counter on it",
            first.to_lowercase(),
            chars.as_str()
        ))
    })
    .collect()
}

/// "As ~ enters, you may reveal any number of other artifact cards from your hand. ~ enters with a +1/+1 counter on it
/// for each card revealed this way."
fn revealed_text(effects: &[Effect], count: &Amount) -> Option<String> {
    let (
        [
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(among),
                count: bounds,
                bind,
                rest_as: None,
            },
            Effect::Reveal {
                what: Ref::Bound(shown),
            },
        ],
        Amount::CountOf {
            of: Ref::Bound(counted),
            filter,
        },
    ) = (effects, count)
    else {
        return None;
    };
    let any_number = bounds.min == Amount::Fixed(0) && bounds.max.is_none();
    if !any_number || shown != bind || counted != bind || !filter.is_empty() {
        return None;
    }
    let in_hand = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    if !in_hand.iter().all(|pred| among.0.contains(pred)) {
        return None;
    }
    let kind = Selector(
        among
            .0
            .iter()
            .filter(|pred| !in_hand.contains(pred))
            .cloned()
            .collect(),
    );
    let cards = card_plural(&kind)?;
    Some(format!(
        "As ~ enters, you may reveal any number of {cards} from your hand. ~ enters with a +1/+1 counter on it for \
         each card revealed this way"
    ))
}

/// "other artifact cards", "Dragon cards": the plural of the kind of card, from its type, subtype and "other".
fn card_plural(kind: &Selector) -> Option<String> {
    let mut other = false;
    let mut words: Vec<String> = Vec::new();
    for pred in &kind.0 {
        match pred {
            Pred::Other => other = true,
            Pred::Type(card_type) => words.push(format!("{card_type:?}").to_lowercase()),
            Pred::Subtype(subtype) => words.push(subtype.0.clone()),
            _ => return None,
        }
    }
    if words.len() != 1 {
        return None;
    }
    let other = if other { "other " } else { "" };
    Some(format!("{other}{} cards", words[0]))
}

/// "put a phylactery counter on an artifact you control.": one permanent the controller chooses among those the
/// selector names gets a counter.
fn chosen_counter_text(effects: &[Effect]) -> Option<String> {
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(among),
            count,
            bind,
            rest_as: None,
        },
        Effect::PutCounters {
            on: Ref::Bound(on),
            counter,
            count: Amount::Fixed(1),
        },
    ] = effects
    else {
        return None;
    };
    if on != bind || !count.is_one() {
        return None;
    }
    let kind = super::counters::text(counter, &Amount::one())?;
    let what = phrase_text(among, Form::Subject)?;
    Some(format!(
        "put {kind} on {} {what}.",
        super::triggers::article(&what)
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: effects as a permanent enters read back as the spell's or the enters trigger's words after "As ~ enters,"
    //! and a near miss (another amount, another kind) is not written as the printed clause.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::lose_life(
        r#"[{"LoseLife":{"amount":{"Life":"You"}}}]"#,
        "As ~ enters, you lose life equal to your life total",
        true
    )]
    #[case::lose_two(
        r#"[{"LoseLife":{"amount":2}}]"#,
        "As ~ enters, you lose life equal to your life total",
        false
    )]
    #[case::discard_hand(
        r#"[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]"#,
        "As ~ enters, discard your hand",
        true
    )]
    #[case::discard_one(r#"[{"Discard":{}}]"#, "As ~ enters, discard your hand", false)]
    #[case::sacrifice_lands(r#"[{"Sacrifice":{"filter":[{"Type":"Land"},{"ControlledBy":"You"}],"count":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]"#, "As ~ enters, sacrifice all lands you control", true)]
    #[case::sacrifice_two_lands(
        r#"[{"Sacrifice":{"filter":[{"Type":"Land"},{"ControlledBy":"You"}],"count":2}}]"#,
        "As ~ enters, sacrifice all lands you control",
        false
    )]
    #[case::sacrifice_creatures(r#"[{"Sacrifice":{"filter":[{"Type":"Creature"},{"ControlledBy":"You"}],"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]"#, "As ~ enters, sacrifice all lands you control", false)]
    #[case::counter_on_an_artifact(r#"[{"Choose":{"from":{"Each":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"as":"chosen"}},{"PutCounters":{"on":{"Bound":"chosen"},"counter":{"Other":"phylactery"}}}]"#, "As ~ enters, put a phylactery counter on an artifact you control", true)]
    #[case::two_counters(r#"[{"Choose":{"from":{"Each":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"as":"chosen"}},{"PutCounters":{"on":{"Bound":"chosen"},"counter":{"Other":"phylactery"},"count":2}}]"#, "As ~ enters, put a phylactery counter on an artifact you control", false)]
    #[case::face_down(
        r#"[{"TurnFaceDown":{"what":{"Each":[{"Type":"Creature"},"Other",{"Not":"Token"}]}}}]"#,
        "As ~ enters, turn all other nontoken creatures face down",
        true
    )]
    fn the_effects_read_after_as_it_enters(
        #[case] json: &str,
        #[case] printed: &str,
        #[case] written: bool,
    ) {
        let effects: Vec<Effect> = serde_json::from_str(json).expect("effects");
        let texts = texts(&effects);
        assert_eq!(texts.iter().any(|t| t == printed), written, "{texts:?}");
    }

    const THIEF: &str = r#"[{"Let":{"as":"removed","amount":{"AnyCounters":{"Each":[]}}}},{"RemoveAllCounters":{"from":{"Each":[]}}}]"#;
    const THIEF_CREATURES: &str = r#"[{"Let":{"as":"removed","amount":{"AnyCounters":{"Each":[{"Type":"Creature"}]}}}},{"RemoveAllCounters":{"from":{"Each":[{"Type":"Creature"}]}}}]"#;
    const THRESHER: &str = r#"[{"Choose":{"from":{"Each":[{"Type":"Artifact"},"Other",{"In":"Hand"},{"OwnedBy":"You"}]},"count":{"min":0,"max":null},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}}]"#;
    const THRESHER_ONE: &str = r#"[{"Choose":{"from":{"Each":[{"Type":"Artifact"},"Other",{"In":"Hand"},{"OwnedBy":"You"}]},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}}]"#;
    const SPHINX: &str = r#"[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Not":{"Type":"Land"}},{"ControlledBy":"You"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}]"#;
    const RETURNED: &str = "As ~ enters, you may return a nonland permanent you control to its owner's hand. If you do, ~ enters with a +1/+1 counter on it";
    const REMOVED: &str = "As ~ enters, remove all counters from all permanents. ~ enters with a +1/+1 counter on it for each counter removed this way";
    const REVEALED: &str = "As ~ enters, you may reveal any number of other artifact cards from your hand. ~ enters with a +1/+1 counter on it for each card revealed this way";

    #[rstest]
    #[case::removed(THIEF, r#"{"Bound":"removed"}"#, REMOVED, true)]
    #[case::removed_from_creatures(THIEF_CREATURES, r#"{"Bound":"removed"}"#, REMOVED, false)]
    #[case::removed_but_fixed(THIEF, "2", REMOVED, false)]
    #[case::returned(
        SPHINX,
        r#"{"CountOf":{"of":{"Bound":"returned"},"filter":[]}}"#,
        RETURNED,
        true
    )]
    #[case::returned_but_two(SPHINX, "2", RETURNED, false)]
    #[case::revealed(
        THRESHER,
        r#"{"CountOf":{"of":{"Bound":"revealed"},"filter":[]}}"#,
        REVEALED,
        true
    )]
    #[case::revealed_one(
        THRESHER_ONE,
        r#"{"CountOf":{"of":{"Bound":"revealed"},"filter":[]}}"#,
        REVEALED,
        false
    )]
    fn the_count_reads_after_the_steps(
        #[case] json: &str,
        #[case] count: &str,
        #[case] printed: &str,
        #[case] written: bool,
    ) {
        let effects: Vec<Effect> = serde_json::from_str(json).expect("effects");
        let count: Amount = serde_json::from_str(count).expect("count");
        let texts = with_texts(&effects, &CounterKind::PlusOnePlusOne, &count);
        assert_eq!(texts.iter().any(|t| t == printed), written, "{texts:?}");
    }
}
