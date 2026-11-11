//! Wordings of shapes the renderers write without one of their parts: a replacement under a condition ("If a source
//! you control would deal noncombat damage to a permanent or player while there are four or more card types among
//! cards in your graveyard, ...", "As long as an opponent lost life this turn, each other Vampire you control enters
//! with ..."), a target picked at random ("target opponent chosen at random"), a spell that costs life for each target
//! ("This spell costs 3 life more to cast for each target.") and a target spell that targets only a single object of a
//! kind ("target spell that targets only a single creature"), and creatures of one of two colors tapped as a cost
//! ("Tap two untapped white and/or blue creatures you control"), and a spell its owner can't cast in their first turns
//! ("You can't cast ~ during your first, second, or third turns of the game."), and the card attacking a battle
//! ("Whenever this creature attacks a battle, ..."), and other creatures with a morph or megamorph ability as targets
//! ("any number of target creatures with morph abilities other than this creature"), and the ∞ ability of a stone,
//! which works while its permanent is harnessed ("∞ — At the beginning of your upkeep, ..."), and the permanents attached
//! to the controller ("destroy all Curses attached to you"), and a target player other than the one the trigger is about
//! ("choose another target player. ~ deals damage ... to the chosen player"), and a counter put on or taken from a
//! creature as it fought since the last upkeep ("put a +1/+1 counter on enchanted creature if it attacked or blocked
//! since your last upkeep. Otherwise, remove a +1/+1 counter from it.").

use crate::ability::{Ability, CostChange};
use crate::compile::read::conditional::condition_texts;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::compile::read::triggers::condition_text;
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Selector, TargetSpec, Targetable};
use crate::value::Amount;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for text in conditioned(ability)
        .into_iter()
        .chain(at_random(ability))
        .chain(life_per_target(ability))
        .chain(single_kind(ability))
        .chain(either_color_tapped(ability))
        .chain(not_in_early_turns(ability))
        .chain(attacks_a_battle(ability))
        .chain(with_morph(ability))
        .chain(infinity(ability))
        .chain(attached_to_you(ability))
        .chain(another_player(ability))
        .chain(fought_since_upkeep(ability))
    {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// A replacement under a condition, from the wordings of the replacement without it: "If ... would ... while
/// <condition>, ..." and "As long as <condition>, <the replacement>."
fn conditioned(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when,
        by,
        condition: Some(condition),
        from,
    } = ability
    else {
        return Vec::new();
    };
    let plain = Ability::Replacement {
        when: when.clone(),
        by: by.clone(),
        condition: None,
        from: *from,
    };
    let mut conditions = condition_texts(condition);
    if let Some(said) = condition_text(condition)
        && !conditions.contains(&said)
    {
        conditions.push(said);
    }
    let mut texts = Vec::new();
    for text in crate::compile::written_variants(&plain) {
        for said in &conditions {
            if text.starts_with("If ")
                && let Some((head, rest)) = text.split_once(", ")
                && head.contains(" would ")
            {
                texts.push(format!("{head} while {said}, {rest}"));
            } else if !text.starts_with("If ")
                && let Some(first) = text.chars().next()
            {
                let rest = &text[first.len_utf8()..];
                texts.push(format!("As long as {said}, {}{rest}", first.to_lowercase()));
            }
        }
    }
    texts
}

/// A target chosen at random, from the wordings of the ability with an ordinary target: "target opponent chosen at
/// random".
fn at_random(ability: &Ability) -> Vec<String> {
    let (Ability::Triggered { targets, .. }
    | Ability::Activated { targets, .. }
    | Ability::Spell { targets, .. }) = ability
    else {
        return Vec::new();
    };
    let mut plain = Vec::new();
    let mut nouns = Vec::new();
    for spec in targets {
        match &spec.what {
            Targetable::AtRandom(inner) if spec.bounds.is_one() => {
                let noun = match inner.as_ref() {
                    Targetable::Opponent => "opponent",
                    Targetable::Player => "player",
                    _ => return Vec::new(),
                };
                nouns.push(noun);
                plain.push(TargetSpec {
                    what: inner.as_ref().clone(),
                    ..spec.clone()
                });
            }
            _ => plain.push(spec.clone()),
        }
    }
    // One random target only: two would leave which wording belongs to which unsaid.
    let [noun] = nouns.as_slice() else {
        return Vec::new();
    };
    let Some(plain) = with_targets(ability, plain) else {
        return Vec::new();
    };
    let said = format!("target {noun} ");
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter(|text| text.matches(&said).count() == 1)
        .map(|text| text.replacen(&said, &format!("target {noun} chosen at random "), 1))
        .collect()
}

/// The ability with other target specifications.
fn with_targets(ability: &Ability, specs: Vec<TargetSpec>) -> Option<Ability> {
    let mut ability = ability.clone();
    match &mut ability {
        Ability::Triggered { targets, .. }
        | Ability::Activated { targets, .. }
        | Ability::Spell { targets, .. } => *targets = specs,
        _ => return None,
    }
    Some(ability)
}

/// "This spell costs 3 life more to cast for each target.": an increase of life alone, once for each target of the
/// spell's first target specification.
fn life_per_target(ability: &Ability) -> Option<String> {
    let Ability::CostModifier {
        applies_to,
        change: CostChange::Increase,
        mana,
        per: Amount::TargetCount(0),
        condition: None,
        activating: None,
        targeting: None,
        least: 0,
        life,
    } = ability
    else {
        return None;
    };
    (*applies_to == Selector::this()
        && *life > 0
        && mana.x() == 0
        && mana.rest().symbols().is_empty())
    .then(|| format!("This spell costs {life} life more to cast for each target."))
}

/// "target spell that targets only a single creature", from the wordings of the ability with "target spell with a
/// single target": a spell with one target, which the selector picks.
fn single_kind(ability: &Ability) -> Vec<String> {
    let (Ability::Triggered { targets, .. }
    | Ability::Activated { targets, .. }
    | Ability::Spell { targets, .. }) = ability
    else {
        return Vec::new();
    };
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    let Targetable::Object(selector) = &spec.what else {
        return Vec::new();
    };
    let [Pred::Spell, Pred::SingleTarget, Pred::Targeting(aim)] = selector.0.as_slice() else {
        return Vec::new();
    };
    let Some(noun) = phrase_text(aim, Form::Target) else {
        return Vec::new();
    };
    let plain = TargetSpec {
        what: Targetable::Object(Selector(vec![Pred::Spell, Pred::SingleTarget])),
        ..spec.clone()
    };
    let Some(plain) = with_targets(ability, vec![plain]) else {
        return Vec::new();
    };
    let said = "target spell with a single target";
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter(|text| text.matches(said).count() == 1)
        .map(|text| {
            text.replacen(
                said,
                &format!("target spell that targets only a single {noun}"),
                1,
            )
        })
        .collect()
}

/// A cost that taps creatures of either of two colors, from the wordings of the cost with the first color alone:
/// "Tap two untapped white and/or blue creatures you control".
fn either_color_tapped(ability: &Ability) -> Vec<String> {
    let Ability::Activated { cost, .. } = ability else {
        return Vec::new();
    };
    let mut plain = cost.0.clone();
    let mut colors = None;
    for part in &mut plain {
        let CostItem::TapPermanents { filter, .. } = part else {
            continue;
        };
        for pred in &mut filter.0 {
            if let Pred::AnyOf(either) = pred
                && let [Pred::Color(first), Pred::Color(second)] = either.as_slice()
                && colors.is_none()
            {
                colors = Some((*first, *second));
                *pred = Pred::Color(*first);
            }
        }
    }
    let Some((first, second)) = colors else {
        return Vec::new();
    };
    let mut rebuilt = ability.clone();
    if let Ability::Activated { cost, .. } = &mut rebuilt {
        *cost = Cost(plain);
    }
    let (first, second) = (
        format!("{first:?}").to_lowercase(),
        format!("{second:?}").to_lowercase(),
    );
    let said = format!(" {first} creatures");
    crate::compile::written_variants(&rebuilt)
        .into_iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(": ")?;
            (head.starts_with("Tap ") && head.matches(&said).count() == 1).then(|| {
                let head = head.replacen(&said, &format!(" {first} and/or {second} creatures"), 1);
                format!("{head}: {rest}")
            })
        })
        .collect()
}

/// "You can't cast ~ during your first, second, or third turns of the game.": the spell is cast only outside the
/// caster's first turns.
fn not_in_early_turns(ability: &Ability) -> Option<String> {
    let Ability::CastOnly {
        condition: crate::value::Condition::Not(early),
    } = ability
    else {
        return None;
    };
    let turns = crate::compile::read::rmt8_conditions::early_turns_text(early, true)?;
    Some(format!("You can't cast ~ during {turns}."))
}

/// "Whenever ~ attacks a battle, ...": from the wordings of the trigger on the card's attack.
fn attacks_a_battle(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when: crate::trigger::TriggerPattern::Attacks { what },
        ..
    } = ability
    else {
        return Vec::new();
    };
    if what.0 != [Pred::This, Pred::AttackingBattle] {
        return Vec::new();
    }
    let mut plain = ability.clone();
    if let Ability::Triggered {
        when: crate::trigger::TriggerPattern::Attacks { what },
        ..
    } = &mut plain
    {
        *what = Selector::this();
    }
    let said = "Whenever ~ attacks, ";
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter_map(|text| {
            let rest = text.strip_prefix(said)?;
            Some(format!("Whenever ~ attacks a battle, {rest}"))
        })
        .collect()
}

/// The morph abilities (CR 702.37b: megamorph is a variant of morph).
fn morph_abilities() -> Pred {
    Pred::AnyOf(vec![
        Pred::HasKeywordNamed("Morph".to_owned()),
        Pred::HasKeywordNamed("Megamorph".to_owned()),
    ])
}

/// "any number of target creatures with morph abilities other than ~": from the wordings of the ability whose targets
/// are any number of other creatures.
fn with_morph(ability: &Ability) -> Vec<String> {
    let (Ability::Triggered { targets, .. }
    | Ability::Activated { targets, .. }
    | Ability::Spell { targets, .. }) = ability
    else {
        return Vec::new();
    };
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    let Targetable::Object(selector) = &spec.what else {
        return Vec::new();
    };
    let creature = Pred::Type(mtg_core::card::CardType::Creature);
    if selector.0 != [creature.clone(), morph_abilities(), Pred::Other] {
        return Vec::new();
    }
    let plain = TargetSpec {
        what: Targetable::Object(Selector(vec![creature, Pred::Other])),
        ..spec.clone()
    };
    let Some(plain) = with_targets(ability, vec![plain]) else {
        return Vec::new();
    };
    let said = "any number of other target creatures";
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter(|text| text.matches(said).count() == 1)
        .map(|text| {
            text.replacen(
                said,
                "any number of target creatures with morph abilities other than ~",
                1,
            )
        })
        .collect()
}

/// "∞ — <trigger>": a triggered ability that works only while its permanent is harnessed, from the wordings of the
/// trigger without that condition.
fn infinity(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = ability
    else {
        return Vec::new();
    };
    let harnessed = crate::value::Condition::Matches {
        subject: crate::refs::Ref::This,
        selector: Selector(vec![Pred::Harnessed]),
    };
    if *condition != harnessed {
        return Vec::new();
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    crate::compile::written_variants(&plain)
        .into_iter()
        .map(|text| format!("\u{221e} \u{2014} {text}"))
        .collect()
}

/// "destroy all Curses attached to you": from the wordings of the ability that destroys the group without the player
/// they are attached to.
fn attached_to_you(ability: &Ability) -> Vec<String> {
    let (Ability::Triggered { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Spell { effects, .. }) = ability
    else {
        return Vec::new();
    };
    let [
        crate::effect::Effect::Destroy {
            what: crate::refs::Ref::Each(group),
            ..
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let Some((Pred::AttachedTo(crate::refs::Ref::You), rest)) = group.0.split_last() else {
        return Vec::new();
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Spell { effects, .. } = &mut plain
        && let [crate::effect::Effect::Destroy { what, .. }] = effects.as_mut_slice()
    {
        *what = crate::refs::Ref::Each(Selector(rest.to_vec()));
    }
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter_map(|text| {
            let head = text.strip_suffix('.')?;
            head.contains("destroy all ")
                .then(|| format!("{head} attached to you."))
        })
        .collect()
}

/// The players other than the one the trigger is about: "another target player".
fn others() -> Targetable {
    use crate::refs::Ref;
    Targetable::PlayerAmong(Ref::PlayersWhere {
        who: Box::new(Ref::EachPlayer),
        test: Box::new(crate::value::Condition::Not(Box::new(
            crate::value::Condition::Among(Ref::TriggeringPlayer),
        ))),
    })
}

/// "Whenever ..., choose another target player. ~ deals damage ... to the chosen player.": from the wordings of the
/// ability with a target player.
fn another_player(ability: &Ability) -> Vec<String> {
    let Ability::Triggered { targets, .. } = ability else {
        return Vec::new();
    };
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    if spec.what != others() || !spec.bounds.is_one() {
        return Vec::new();
    }
    let plain = TargetSpec {
        what: Targetable::Player,
        ..spec.clone()
    };
    let Some(plain) = with_targets(ability, vec![plain]) else {
        return Vec::new();
    };
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter(|text| text.matches("target player").count() == 1)
        .filter_map(|text| {
            let (head, body) = text.split_once(", ")?;
            body.starts_with('~').then(|| {
                let body = body.replacen("target player", "the chosen player", 1);
                format!("{head}, choose another target player. {body}")
            })
        })
        .collect()
}

/// "<head>, put a +1/+1 counter on <it> if it attacked or blocked since your last upkeep. Otherwise, remove a +1/+1
/// counter from it.": from the wordings of the trigger that puts the counter alone.
fn fought_since_upkeep(ability: &Ability) -> Vec<String> {
    use crate::effect::Effect;
    use crate::value::Condition;
    let Ability::Triggered { effects, .. } = ability else {
        return Vec::new();
    };
    let [
        Effect::If {
            condition: Condition::Matches { subject, selector },
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let [
        Pred::FoughtSinceYourLastUpkeep {
            attacked,
            blocked,
            was_blocked,
        },
    ] = selector.0.as_slice()
    else {
        return Vec::new();
    };
    let deeds = match (attacked, blocked, was_blocked) {
        (true, true, false) => "attacked or blocked",
        (false, true, true) => "has blocked or been blocked",
        _ => return Vec::new(),
    };
    let [Effect::PutCounters { on, counter, count }] = then.as_slice() else {
        return Vec::new();
    };
    let [
        Effect::RemoveCounters {
            from,
            counter: Some(taken),
            count: one,
            up_to: false,
        },
    ] = otherwise.as_slice()
    else {
        return Vec::new();
    };
    let plus = crate::value::CounterKind::PlusOnePlusOne;
    if on != subject
        || from != subject
        || *counter != plus
        || *taken != plus
        || !count.is_one()
        || !one.is_one()
    {
        return Vec::new();
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut plain {
        effects.clone_from(then);
    }
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter_map(|text| {
            let head = text.strip_suffix('.')?;
            Some(format!(
                "{head} if it {deeds} since your last upkeep. Otherwise, remove a +1/+1 counter from it."
            ))
        })
        .collect()
}
