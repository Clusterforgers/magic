//! l32-a wordings and their writers: "you may pay {1}{G} any number of times. When you pay this cost one or more times,
//! S." (`PayAnyTimes` and the reflexive trigger after it, whose "that many" is the number of payments), "This ability
//! still resolves if its target becomes illegal." (`StillResolves`) and "you may reselect which player or permanent
//! target attacking creature is attacking" (`Reattack`) in a trigger on entering during the declare attackers step.

use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use crate::trigger::Step;
use crate::value::{Amount, Condition};
use mtg_core::card::CardType;

/// A target count of the number of payments.
const UP_TO_MANY: &str = "up to that many ";
/// The count the twin of such a target is written with.
const UP_TO_TWO: &str = "up to two ";
/// The words before the cost of a payment any number of times.
const PAY: &str = "you may pay ";
/// The words after the cost, up to the reflexive trigger's steps.
const TIMES: &str = " any number of times. When you pay this cost one or more times, ";
/// The sentence of an ability that resolves with an illegal target.
const STILL: &str = "This ability still resolves if its target becomes illegal.";
/// The words of a trigger on entering during the declare attackers step, after the trigger's own words.
const DURING: &str = " during the declare attackers step, ";
/// The step of the reflexive head that a probe of the trigger's head writes ("When ~ enters, you gain 1 life.").
const PROBE: &str = "you gain 1 life.";

/// A trigger with these targets and steps and everything else of `ability`; `None` for any other ability.
fn with_steps(
    ability: &Ability,
    targets: Vec<TargetSpec>,
    effects: Vec<Effect>,
) -> Option<Ability> {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        ..
    } = ability
    else {
        return None;
    };
    Some(Ability::Triggered {
        when: when.clone(),
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets,
        effects,
    })
}

/// The words of the trigger before its steps, as the writers put them ("When ~ enters, "), read off a probe of the
/// same trigger that gains 1 life.
fn heads(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let probe = with_steps(
        ability,
        Vec::new(),
        vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(1),
        }],
    );
    probe
        .map(|probe| written(&probe))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|text| text.strip_suffix(PROBE).map(str::to_owned))
        .collect()
}

/// "When ~ enters, you may pay {1}{G} any number of times. When you pay this cost one or more times, S.": read from the
/// twin "When ~ enters, S." whose targets and steps become the reflexive trigger's.
pub(super) fn l32a_times_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let (head, rest) = text.split_once(TIMES)?;
    let at = head.rfind(PAY)?;
    let mana: crate::mana::ManaCostX = head[at + PAY.len()..].parse().ok()?;
    (!mana.has_x()).then_some(())?;
    // "up to that many target ..." is read as "up to two target ..." and its most is then the number of payments.
    let spoken = rest.matches(UP_TO_MANY).count();
    (spoken <= 1 && !rest.contains(UP_TO_TWO)).then_some(())?;
    let twin = read(&format!(
        "{}{}",
        &head[..at],
        rest.replacen(UP_TO_MANY, UP_TO_TWO, 1)
    ))?;
    let (mut targets, effects) = (twin.targets().to_vec(), twin.effects().to_vec());
    if spoken == 1 {
        let [spec] = targets.as_mut_slice() else {
            return None;
        };
        (spec.bounds == Bounds::up_to(Some(Amount::Fixed(2)))).then_some(())?;
        spec.bounds = Bounds::up_to(Some(Amount::Triggering));
    }
    with_steps(
        &twin,
        Vec::new(),
        vec![
            Effect::PayAnyTimes {
                who: Ref::You,
                cost: Cost::mana(mana),
            },
            Effect::Reflexive {
                targets,
                effects,
                condition: None,
            },
        ],
    )
}

/// The wordings of a trigger that pays a mana cost any number of times and then triggers: the twin that does the
/// reflexive trigger's steps itself, with the payment and the link put in after the trigger's words.
pub(super) fn l32a_times_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let (
        Ability::Triggered { targets, .. },
        [
            Effect::PayAnyTimes {
                who: Ref::You,
                cost,
            },
            Effect::Reflexive {
                targets: own,
                effects: steps,
                condition: None,
            },
        ],
    ) = (ability, ability.effects())
    else {
        return;
    };
    let [CostItem::Mana(mana)] = cost.0.as_slice() else {
        return;
    };
    if !targets.is_empty() || mana.has_x() {
        return;
    }
    let per_payment = Bounds::up_to(Some(Amount::Triggering));
    let two = Bounds::up_to(Some(Amount::Fixed(2)));
    let mut own = own.clone();
    let counted = own.iter().filter(|spec| spec.bounds == per_payment).count();
    if counted > 1 || own.iter().any(|spec| spec.bounds == two) {
        return;
    }
    for spec in &mut own {
        if spec.bounds == per_payment {
            spec.bounds = two.clone();
        }
    }
    let Some(twin) = with_steps(ability, own, steps.clone()) else {
        return;
    };
    let heads = heads(ability, written);
    for text in written(&twin) {
        if counted == 1 && text.matches(UP_TO_TWO).count() != 1 {
            continue;
        }
        let text = text.replacen(UP_TO_TWO, UP_TO_MANY, counted);
        for head in &heads {
            let Some(rest) = text.strip_prefix(head.as_str()) else {
                continue;
            };
            let paid = format!("{head}{PAY}{mana}{TIMES}{rest}");
            if !texts.contains(&paid) {
                texts.push(paid);
            }
        }
    }
}

/// "S. This ability still resolves if its target becomes illegal.": read from S, a trigger with a target.
pub(super) fn l32a_still_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let head = text.strip_suffix(STILL)?.trim_end();
    let twin = read(head)?;
    (!twin.targets().is_empty()).then_some(())?;
    let mut effects = twin.effects().to_vec();
    effects.push(Effect::StillResolves);
    with_steps(&twin, twin.targets().to_vec(), effects)
}

/// The wordings of a trigger with a target whose last step says it still resolves: those of the trigger without it,
/// with the sentence after them.
pub(super) fn l32a_still_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((Effect::StillResolves, steps)) = ability.effects().split_last() else {
        return;
    };
    if ability.targets().is_empty()
        || steps
            .iter()
            .any(|step| matches!(step, Effect::StillResolves))
    {
        return;
    }
    let Some(twin) = with_steps(ability, ability.targets().to_vec(), steps.to_vec()) else {
        return;
    };
    for text in written(&twin) {
        let still = format!("{} {STILL}", text.trim_end_matches('.').to_owned() + ".");
        if !texts.contains(&still) {
            texts.push(still);
        }
    }
}

/// An attacking creature, the object of a reselection.
fn attacking_creature() -> Selector {
    Selector(vec![Pred::Type(CardType::Creature), Pred::Attacking])
}

/// What a reselection may choose, in words.
fn chosen(players: bool) -> &'static str {
    if players {
        "player"
    } else {
        "player or permanent"
    }
}

/// The condition of a trigger on entering during the declare attackers step, checked only as it triggers.
fn during_attackers() -> Condition {
    Condition::AtTrigger(Box::new(Condition::Step(Step::DeclareAttackers)))
}

/// "you may reselect which player or permanent target attacking creature is attacking" and "for each attacking
/// creature, you may reselect which player or permanent that creature is attacking", with the targets the first takes.
fn reattack_steps(text: &str) -> Option<(Vec<TargetSpec>, Effect)> {
    let text = text.strip_suffix('.')?;
    if let Some(rest) = text.strip_prefix("for each attacking creature, you may reselect which ") {
        let players = chosen_of(rest.strip_suffix(" that creature is attacking")?)?;
        return Some((
            Vec::new(),
            Effect::Reattack {
                what: Ref::Each(attacking_creature()),
                players,
            },
        ));
    }
    let rest = text.strip_prefix("you may reselect which ")?;
    let players = chosen_of(rest.strip_suffix(" target attacking creature is attacking")?)?;
    Some((
        vec![TargetSpec {
            what: Targetable::Object(attacking_creature()),
            bounds: Bounds::default(),
            divide: None,
        }],
        Effect::Reattack {
            what: Ref::Target(0),
            players,
        },
    ))
}

/// Whether the words name players alone.
fn chosen_of(words: &str) -> Option<bool> {
    match words {
        "player" => Some(true),
        "player or permanent" => Some(false),
        _ => None,
    }
}

/// "When ~ enters during the declare attackers step, you may reselect ...": read from the trigger's own words, taken from
/// a twin that gains 1 life.
pub(super) fn l32a_reattack_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let (head, body) = text.split_once(DURING)?;
    let (targets, step) = reattack_steps(body)?;
    let twin = read(&format!("{head}, {PROBE}"))?;
    let Ability::Triggered {
        intervening_if: None,
        ..
    } = &twin
    else {
        return None;
    };
    let Ability::Triggered {
        when, limit, from, ..
    } = twin
    else {
        return None;
    };
    Some(Ability::Triggered {
        when,
        intervening_if: Some(during_attackers()),
        limit,
        from,
        targets,
        effects: vec![step],
    })
}

/// The wording of a trigger on entering during the declare attackers step that reselects what attacking creatures attack.
pub(super) fn l32a_reattack_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Triggered {
        intervening_if: Some(condition),
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [Effect::Reattack { what, players }] = effects.as_slice() else {
        return;
    };
    if *condition != during_attackers() {
        return;
    }
    let body = match (what, targets.as_slice()) {
        (Ref::Target(0), [spec])
            if spec.what == Targetable::Object(attacking_creature())
                && spec.bounds == Bounds::default()
                && spec.divide.is_none() =>
        {
            format!(
                "you may reselect which {} target attacking creature is attacking.",
                chosen(*players)
            )
        }
        (Ref::Each(selector), []) if *selector == attacking_creature() => format!(
            "for each attacking creature, you may reselect which {} that creature is attacking.",
            chosen(*players)
        ),
        _ => return,
    };
    let Some(Ability::Triggered {
        when, limit, from, ..
    }) = Some(ability.clone())
    else {
        return;
    };
    let plain = Ability::Triggered {
        when,
        intervening_if: None,
        limit,
        from,
        targets: Vec::new(),
        effects: Vec::new(),
    };
    for head in heads(&plain, written) {
        let Some(head) = head.strip_suffix(", ") else {
            continue;
        };
        let text = format!("{head}{DURING}{body}");
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The sentence after an exchange of the source and a target, when either did not happen.
const UNEXCHANGED: &str = ". If you don't or can't make an exchange, sacrifice ";

/// The step a twin uses to word the target alone ("exile up to one target creature an opponent controls").
fn exile_target() -> Option<Effect> {
    serde_json::from_str(r#"{"Exile":{"what":{"Target":0}}}"#).ok()
}

/// The steps of an exchange of the source and its one target, with the sacrifice of the source when there was none.
fn exchanged_steps() -> Option<Vec<Effect>> {
    serde_json::from_str(
        r#"[{"ExchangeControl":{"what":["This",{"Target":0}],"duration":"Forever"}},{"If":{"condition":{"Not":"Performed"},"then":[{"Sacrifice":{"filter":["This"]}}]}}]"#,
    )
    .ok()
}

/// "When ~ enters, exchange control of ~ and up to one target creature an opponent controls. If you don't or can't make
/// an exchange, sacrifice ~.": the target read from the twin that exiles it.
pub(super) fn l32a_exchange_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let (head, rest) = text.split_once("exchange control of ")?;
    let (spec, _) = ["~", "this creature"].iter().find_map(|name| {
        let spec = rest.strip_prefix(*name)?.strip_prefix(" and ")?;
        let spec = spec.strip_suffix(&format!("{UNEXCHANGED}{name}."))?;
        Some((spec, *name))
    })?;
    let twin = read(&format!("{head}exile {spec}."))?;
    let (Some(exiled), [spec]) = (exile_target(), twin.targets()) else {
        return None;
    };
    (twin.effects() == [exiled].as_slice()).then_some(())?;
    with_steps(&twin, vec![spec.clone()], exchanged_steps()?)
}

/// The wordings of a trigger that exchanges the source and its one target and sacrifices the source when it could not.
pub(super) fn l32a_exchange_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let (Some(steps), Some(exiled), [_]) = (exchanged_steps(), exile_target(), ability.targets())
    else {
        return;
    };
    if ability.effects() != steps.as_slice() {
        return;
    }
    let Some(twin) = with_steps(ability, ability.targets().to_vec(), vec![exiled]) else {
        return;
    };
    for head in heads(ability, written) {
        for text in written(&twin) {
            let Some(spec) = text
                .strip_prefix(&format!("{head}exile "))
                .and_then(|spec| spec.strip_suffix('.'))
            else {
                continue;
            };
            let exchanged = format!("{head}exchange control of ~ and {spec}{UNEXCHANGED}~.");
            if !texts.contains(&exchanged) {
                texts.push(exchanged);
            }
        }
    }
}
