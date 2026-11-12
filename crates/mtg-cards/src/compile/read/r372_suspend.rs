//! r37-2 wordings of a card that gains suspend in exile (`Effect::GainSuspend`) and their writers: "exile it with three
//! time counters on it and it gains suspend", "exile cards from the top of your library until you exile a nonland card.
//! Put three time counters on it. If it doesn't have suspend, it gains suspend." and the sentence "It gains suspend." or
//! "If it doesn't have suspend, it gains suspend." after an exile that named the card.

use mtg_core::card::CardType;

use super::library::{top_cards, top_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::{EXILED, Effect, FOUND};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, CounterKind, StatKind};

/// The words before the found card of an exile until a nonland card.
const UNTIL: &str = "exile cards from the top of your library until you exile a nonland card";

/// The name of each card a pass over the exiled cards is for.
const EACH: &str = "card";
/// The name of the card chosen from a hand.
const CHOSEN: &str = "chosen";
/// The sentences after an exile of the top cards that suspend each nonland one.
const EACH_NONLAND: &str = "For each nonland card exiled this way, put a number of time counters on that card equal to \
                            its mana value. If it doesn't have suspend, it gains suspend";
/// The exile of a card from a hand with time counters by its mana value, and the suspend it gains.
const FROM_HAND: &str = "exile a card from your hand with a number of time counters on it equal to its mana value. If \
                         it doesn't have suspend, it gains suspend";

/// A time counter for each mana value of `of`.
fn by_mana_value(of: &str) -> Effect {
    Effect::PutCounters {
        on: Ref::Bound(of.to_owned()),
        counter: CounterKind::Other("time".to_owned()),
        count: Amount::Stat {
            of: Ref::Bound(of.to_owned()),
            stat: StatKind::ManaValue,
        },
    }
}

/// "If it doesn't have suspend, it gains suspend." of `of`.
fn gains_unless_has(of: &str) -> Effect {
    Effect::GainSuspend {
        what: Ref::Bound(of.to_owned()),
        unless_has: true,
    }
}

/// The nonland cards among those exiled.
fn exiled_nonland() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::Is(Ref::Bound(EXILED.to_owned())),
        Pred::Not(Box::new(Pred::Type(CardType::Land))),
    ]))
}

/// The cards of the controller's hand.
fn own_hand() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(Ref::You),
    ]))
}

/// The steps of "Exile the top five cards of your library. For each nonland card exiled this way, put a number of time
/// counters on that card equal to its mana value. If it doesn't have suspend, it gains suspend."
fn top_suspended(count: Amount) -> Vec<Effect> {
    vec![
        Effect::ExileTop {
            who: Ref::You,
            count,
            bind: EXILED.to_owned(),
            sealed: false,
        },
        Effect::RepeatEach {
            over: exiled_nonland(),
            bind: EACH.to_owned(),
            effects: vec![by_mana_value(EACH), gains_unless_has(EACH)],
        },
    ]
}

/// The steps of "you may exile a card from your hand with a number of time counters on it equal to its mana value. If it
/// doesn't have suspend, it gains suspend."
fn from_hand_suspended() -> Vec<Effect> {
    vec![Effect::May {
        who: Ref::You,
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: own_hand(),
                count: Bounds::default(),
                bind: CHOSEN.to_owned(),
                rest_as: None,
            },
            Effect::Exile {
                what: Ref::Bound(CHOSEN.to_owned()),
                bind: Some(EXILED.to_owned()),
                duration: None,
            },
            by_mana_value(EXILED),
            gains_unless_has(EXILED),
        ],
    }]
}

/// Reads the words and stops of `text` ahead, its words in any case.
fn words_of(r: &mut Reader<'_>, text: &str) -> Option<()> {
    for piece in text.split(' ') {
        let word = piece.trim_end_matches([',', '.']);
        r.word(&word.to_lowercase()).then_some(())?;
        for mark in piece[word.len()..].chars() {
            r.punct(mark).then_some(())?;
        }
    }
    Some(())
}

/// "Exile the top five cards of your library. For each nonland card exiled this way, put a number of time counters on
/// that card equal to its mana value. If it doesn't have suspend, it gains suspend."
pub fn r372_top_suspended(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("exile")?;
    let count = top_cards(r)?;
    r.need("of your library")?;
    r.punct('.').then_some(())?;
    words_of(r, EACH_NONLAND)?;
    Some(top_suspended(count))
}

/// "You may exile a card from your hand with a number of time counters on it equal to its mana value. If it doesn't have
/// suspend, it gains suspend."
pub fn r372_from_hand(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may")?;
    words_of(r, FROM_HAND)?;
    Some(from_hand_suspended())
}

/// The words of a trigger on damage that takes time counters off a suspended card.
const SPED_UP: &str = "choose a suspended card you own and remove that many time counters from it";

/// The steps of "choose a suspended card you own and remove that many time counters from it".
fn sped_up() -> Vec<Effect> {
    vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(Selector(vec![
                Pred::Suspended,
                Pred::OwnedBy(Ref::You),
                Pred::In(ZoneKind::Exile),
            ])),
            count: Bounds::default(),
            bind: EACH.to_owned(),
            rest_as: None,
        },
        Effect::RemoveCounters {
            from: Ref::Bound(EACH.to_owned()),
            counter: Some(CounterKind::Other("time".to_owned())),
            count: Amount::Triggering,
            up_to: false,
        },
    ]
}

/// "Choose a suspended card you own and remove that many time counters from it." in a trigger whose event has an
/// amount.
pub fn r372_sped_up(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.triggered.then_some(())?;
    words_of(r, SPED_UP)?;
    Some(sped_up())
}

/// The name of the countered spell as the card it becomes.
const COUNTERED: &str = "countered";
/// The words of Delay between the counter and its counters.
const DELAY_EXILE: &str = "If the spell is countered this way, exile it with";
/// The words of Delay after its counters.
const DELAY_REST: &str = "on it instead of putting it into its owner's graveyard. If it doesn't have suspend, it gains suspend";

/// The countered card while it is in exile.
fn countered_in_exile() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::Is(Ref::Bound(COUNTERED.to_owned())),
    ]))
}

/// The steps of "Counter target spell. If the spell is countered this way, exile it with three time counters on it instead
/// of putting it into its owner's graveyard. If it doesn't have suspend, it gains suspend.": the shield sends the spell
/// countered to exile, and the card it becomes there gets the counters and suspend.
fn delay(counter: CounterKind, count: Amount) -> Option<Vec<Effect>> {
    let shield: Effect = serde_json::from_value(serde_json::json!({"Shield": {
        "when": {"Countered": {"what": [{"Is": {"Target": 0}}, "Spell"]}},
        "by": {"MoveTo": "Exile"},
        "duration": "UntilEndOfTurn"
    }}))
    .ok()?;
    Some(vec![
        shield,
        Effect::Counter {
            what: Ref::Target(0),
            bind: Some(COUNTERED.to_owned()),
        },
        Effect::PutCounters {
            on: countered_in_exile(),
            counter,
            count,
        },
        Effect::GainSuspend {
            what: Ref::Bound(COUNTERED.to_owned()),
            unless_has: true,
        },
    ])
}

/// "Counter target spell. If the spell is countered this way, exile it with three time counters on it instead of putting
/// it into its owner's graveyard. If it doesn't have suspend, it gains suspend."
pub fn r372_delay(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("counter")?;
    let what = super::selectors::target(r)?;
    let spell = r.targets.first().is_some_and(|spec| {
        matches!(&spec.what, crate::refs::Targetable::Object(selector) if selector.0.first() == Some(&Pred::Spell))
    });
    (spell && what == Ref::Target(0)).then_some(())?;
    r.punct('.').then_some(())?;
    words_of(r, DELAY_EXILE)?;
    let (counter, number) = super::counters::counters(r)?;
    words_of(r, DELAY_REST)?;
    delay(counter, Amount::Fixed(i32::try_from(number).ok()?))
}

/// The steps of "copy it, then exile the spell you cast with four time counters on it" in a cast trigger.
fn copy_then_exile(counter: CounterKind, count: Amount) -> Vec<Effect> {
    vec![
        Effect::CopySpell {
            what: Ref::Triggering,
            who: Ref::You,
            count: Amount::one(),
            new_targets: false,
            except: Vec::new(),
            each_target: None,
        },
        Effect::Exile {
            what: Ref::Triggering,
            bind: Some(EXILED.to_owned()),
            duration: None,
        },
        Effect::PutCounters {
            on: Ref::Bound(EXILED.to_owned()),
            counter,
            count,
        },
    ]
}

/// "copy it, then exile the spell you cast with four time counters on it": the spell of the cast trigger is copied, then
/// exiled with the counters; the sentence about suspend follows.
pub fn r372_copy_then_exile(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.cast_spell.then_some(())?;
    r.need("copy it")?;
    r.punct(',').then_some(())?;
    r.need("then exile the spell you cast with")?;
    let (counter, number) = super::counters::counters(r)?;
    r.need("on it")?;
    Some(copy_then_exile(
        counter,
        Amount::Fixed(i32::try_from(number).ok()?),
    ))
}

/// The step that gives the card suspend, as the writer says it of `object` ("it", "the exiled card").
pub fn gain_text(object: &str, unless_has: bool) -> String {
    if unless_has {
        format!("if {object} doesn't have suspend, it gains suspend")
    } else {
        format!("{object} gains suspend")
    }
}

/// The card the last step put into exile and named, which "it" of the next sentence is.
fn exiled_by(effects: &[Effect]) -> Option<Ref> {
    match effects.last()? {
        Effect::Exile {
            bind: Some(name), ..
        } => Some(Ref::Bound(name.clone())),
        Effect::ExileUntil { bind, .. } => Some(Ref::Bound(bind.clone())),
        Effect::PutCounters {
            on: on @ Ref::Bound(_),
            counter: CounterKind::Other(kind),
            ..
        } if kind == "time" => Some(on.clone()),
        _ => None,
    }
}

/// "It gains suspend." or "If it doesn't have suspend, it gains suspend." (also "Then if the exiled card doesn't have
/// suspend, ..."): the card the step before exiled gains suspend.
pub fn r372_after(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let what = exiled_by(effects)?;
    let unless_has = if r.word("it") {
        false
    } else {
        r.word("then");
        r.need("if")?;
        (r.word("it") || r.words("the exiled card")).then_some(())?;
        r.need("doesn't have suspend")?;
        r.punct(',').then_some(())?;
        r.need("it")?;
        true
    };
    r.need("gains suspend")?;
    r.sentence_end().then_some(())?;
    effects.push(Effect::GainSuspend { what, unless_has });
    Some(())
}

/// "When ~ dies, exile it with three time counters on it and it gains suspend.": the card in the graveyard is exiled
/// with the counters and gains suspend.
pub fn r372_exile_gains(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("exile it with")?;
    r.died.then_some(())?;
    let (counter, number) = super::counters::counters(r)?;
    r.need("on it and it gains suspend")?;
    Some(vec![
        Effect::Exile {
            what: super::effects::dead_card(),
            bind: Some(EXILED.to_owned()),
            duration: None,
        },
        Effect::PutCounters {
            on: Ref::Bound(EXILED.to_owned()),
            counter,
            count: Amount::Fixed(i32::try_from(number).ok()?),
        },
        Effect::GainSuspend {
            what: Ref::Bound(EXILED.to_owned()),
            unless_has: false,
        },
    ])
}

/// "Exile cards from the top of your library until you exile a nonland card. Put three time counters on it.": the found
/// card gets the counters; the sentence about suspend follows.
pub fn r372_until_counters(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(UNTIL)?;
    r.punct('.').then_some(())?;
    r.need("put")?;
    let (counter, number) = super::counters::counters(r)?;
    r.need("on it")?;
    Some(vec![
        Effect::ExileUntil {
            who: Ref::You,
            filter: Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))]),
            bind: FOUND.to_owned(),
            exiled_as: EXILED.to_owned(),
        },
        Effect::PutCounters {
            on: Ref::Bound(FOUND.to_owned()),
            counter,
            count: Amount::Fixed(i32::try_from(number).ok()?),
        },
    ])
}

/// The sentences of the steps at the start of `effects` that these wordings write, and how many steps they cover.
pub fn text(effects: &[Effect]) -> Option<(String, usize)> {
    match effects {
        [
            Effect::Exile {
                what,
                bind: Some(exiled),
                duration: None,
            },
            Effect::PutCounters {
                on: Ref::Bound(on),
                counter,
                count,
            },
            Effect::GainSuspend {
                what: Ref::Bound(gains),
                unless_has: false,
            },
            ..,
        ] if *what == super::effects::dead_card()
            && [exiled, on, gains].iter().all(|name| *name == EXILED) =>
        {
            let counters = super::counters::text(counter, count)?;
            Some((
                format!("exile it with {counters} on it and it gains suspend"),
                3,
            ))
        }
        [
            Effect::ExileUntil {
                who: Ref::You,
                filter: Selector(preds),
                bind,
                exiled_as,
            },
            Effect::PutCounters {
                on: Ref::Bound(on),
                counter,
                count,
            },
            ..,
        ] if bind == FOUND
            && exiled_as == EXILED
            && on == FOUND
            && preds.as_slice() == [Pred::Not(Box::new(Pred::Type(CardType::Land)))] =>
        {
            let counters = super::counters::text(counter, count)?;
            Some((format!("{UNTIL}. Put {counters} on it"), 2))
        }
        [Effect::ExileTop { count, .. }, second, ..]
            if effects.get(..2) == Some(top_suspended(count.clone()).as_slice())
                && matches!(second, Effect::RepeatEach { .. }) =>
        {
            Some((
                format!("exile {} of your library. {EACH_NONLAND}", top_text(count)?),
                2,
            ))
        }
        [Effect::Choose { .. }, Effect::RemoveCounters { .. }, ..]
            if effects.get(..2) == Some(sped_up().as_slice()) =>
        {
            Some((SPED_UP.to_owned(), 2))
        }
        [first @ Effect::May { .. }, ..] if [first.clone()] == *from_hand_suspended() => {
            Some((format!("you may {FROM_HAND}"), 1))
        }
        [
            Effect::GainSuspend {
                what: Ref::Bound(_),
                unless_has,
            },
            ..,
        ] => Some((gain_text("it", *unless_has), 1)),
        _ => None,
    }
}

/// The words of the cast trigger of a card cast from anywhere but exile, after "When you cast ~".
const NOT_FROM_EXILE: &str = " from anywhere other than exile,";

/// The heads a cast trigger of the card itself is written with.
const CAST_ITSELF: [&str; 2] = ["When you cast ~", "When you cast this spell"];

/// The spell cast is the card itself, cast from anywhere but exile.
fn not_from_exile() -> Selector {
    Selector(vec![
        Pred::This,
        Pred::Not(Box::new(Pred::WasCast {
            by: None,
            from: Some(ZoneKind::Exile),
        })),
    ])
}

/// The trigger `ability` with the spell its event names replaced by `what`, when it is a cast of the card by its controller.
fn cast_twin(ability: &Ability, what: Selector) -> Option<Ability> {
    let Ability::Triggered {
        when: TriggerPattern::SpellCast { by: Ref::You, .. },
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    Some(Ability::Triggered {
        when: TriggerPattern::SpellCast { what, by: Ref::You },
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: effects.clone(),
    })
}

/// "When you cast this spell from anywhere other than exile, S.": read from the twin "When you cast this spell, S.", whose
/// spell becomes the card cast from anywhere but exile.
pub(super) fn r372_not_from_exile_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let body = text.get(crate::compile::classify::ability_word_len(text).unwrap_or(0)..)?;
    let (head, rest) = CAST_ITSELF.iter().find_map(|head| {
        body.strip_prefix(head)?
            .strip_prefix(NOT_FROM_EXILE)
            .map(|rest| (*head, rest))
    })?;
    let twin = read(&format!("{head},{rest}"))?;
    let Ability::Triggered {
        when: TriggerPattern::SpellCast { what, by: Ref::You },
        ..
    } = &twin
    else {
        return None;
    };
    (what.0 == [Pred::This]).then_some(())?;
    cast_twin(&twin, not_from_exile())
}

/// The tail of a cast trigger that copies the spell and suspends it, after its counters.
const COPIED_TAIL: &str = "on it. If it doesn't have suspend, it gains suspend.";
/// The step of the probe whose wording gives a trigger's head.
const PROBE: &str = "you gain 1 life.";

/// The wordings of the whole abilities these steps make, each from a twin the other writers word: a cast trigger of the
/// card cast from anywhere but exile (the twin cast from anywhere, with the words put in after the head), Delay's spell
/// (the twin that only counters its target) and a cast trigger that copies the spell, then exiles and suspends it (the
/// head of a twin that gains 1 life). `None` for any other ability.
pub(super) fn r372_variants(
    ability: &Ability,
    variants: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    match ability {
        Ability::Triggered {
            when: TriggerPattern::SpellCast { what, by: Ref::You },
            ..
        } if *what == not_from_exile() => {
            let twin = cast_twin(ability, Selector(vec![Pred::This]))?;
            Some(
                variants(&twin)
                    .into_iter()
                    .filter_map(|text| {
                        CAST_ITSELF.iter().find_map(|head| {
                            let rest = text.strip_prefix(head)?.strip_prefix(',')?;
                            Some(format!("{head}{NOT_FROM_EXILE}{rest}"))
                        })
                    })
                    .collect(),
            )
        }
        Ability::Spell { targets, effects } => {
            let [_, _, Effect::PutCounters { counter, count, .. }, _] = effects.as_slice() else {
                return None;
            };
            (delay(counter.clone(), count.clone())?.as_slice() == effects.as_slice())
                .then_some(())?;
            let counters = super::counters::text(counter, count)?;
            let twin = Ability::Spell {
                targets: targets.clone(),
                effects: vec![Effect::Counter {
                    what: Ref::Target(0),
                    bind: None,
                }],
            };
            Some(
                variants(&twin)
                    .into_iter()
                    .filter_map(|text| {
                        let head = text.strip_suffix('.')?;
                        Some(format!("{head}. {DELAY_EXILE} {counters} {DELAY_REST}."))
                    })
                    .collect(),
            )
        }
        Ability::Triggered {
            when,
            intervening_if,
            limit,
            from,
            effects,
            ..
        } => {
            let [_, _, Effect::PutCounters { counter, count, .. }, last] = effects.as_slice()
            else {
                return None;
            };
            let mut whole = copy_then_exile(counter.clone(), count.clone());
            whole.push(gains_unless_has(EXILED));
            (whole.as_slice() == effects.as_slice() && *last == gains_unless_has(EXILED))
                .then_some(())?;
            let counters = super::counters::text(counter, count)?;
            let probe = Ability::Triggered {
                when: when.clone(),
                intervening_if: intervening_if.clone(),
                limit: *limit,
                from: *from,
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(1),
                }],
            };
            Some(
                variants(&probe)
                    .into_iter()
                    .filter_map(|text| {
                        let head = text.strip_suffix(PROBE)?;
                        Some(format!(
                            "{head}copy it, then exile the spell you cast with {counters} {COPIED_TAIL}"
                        ))
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}
