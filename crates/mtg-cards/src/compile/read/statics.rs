//! Statics: the abilities of a permanent that need no event. A permanent enters tapped (also
//! unless something holds), with counters, after a choice or after a payment of life; a spell
//! can't be countered; a creature can't block, or attacks each combat; the top card of a library
//! is played revealed; a creature, the creature an Aura or Equipment is attached to, or a group of
//! creatures gets a bonus or a keyword; spells cost more or less. Read from the wording and written back the same way.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype};

use super::abilities;
use super::amounts::{count_word, counted_stats, for_each, for_each_text, number_word};
use super::as_long_as;
use super::aura_forms;
use super::copies;
use super::cost_conditions;
use super::counters;
use super::defining;
use super::effects::{granted, power_toughness};
use super::evasion;
use super::lands;
use super::life_gain;
use super::prevention;
use super::render::{self, capitalize, counts_stats, is_a_type, stats_text};
use super::replacements;
use super::selectors::{
    self, Form, all, color, color_name, each, granted_text, keyword, phrase, phrase_text, subtype,
};
use super::static_parts;
use super::tokens::Reader;
use super::triggers::article;
use super::where_x;
use crate::ability::{Ability, CostChange};
use crate::keyword::Keyword;
use crate::mana::ManaCostX;
use crate::modify::{GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};
use crate::value::{Amount, CastWay, Cmp, Condition};

type Template = fn(&mut Reader<'_>) -> Option<Ability>;

/// The static shapes, tried in this order.
const TEMPLATES: [Template; 24] = [
    entering,
    entering_tapped_if,
    group_enters_tapped,
    life_gain::replacement,
    no_life_loss,
    copies::enters_as_copy,
    as_entering,
    super::day_night::starts_the_day,
    rule,
    super::player_keywords::ability,
    lands::from_a_zone,
    lands::cast_from_the_top,
    cannot,
    cost_modifier,
    super::spell_keywords::ability,
    super::card_grants::ability,
    modifies,
    defining::ability,
    prevention::static_rule,
    super::costs::additional,
    super::attack_tax::static_rule,
    evasion::cannot_block,
    evasion::block_only,
    free_cast,
];

/// A static ability that is a whole sentence: the shape and its period.
pub fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    TEMPLATES.iter().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            r.sentence_end().then_some(found)
        })
    })
}

/// "Damage doesn't cause you to lose life.", also "As long as you're the monarch, damage doesn't cause you to lose life.": a
/// rule of the game for its controller, under the condition when there is one.
fn no_life_loss(r: &mut Reader<'_>) -> Option<Ability> {
    let condition = r.attempt(|r| {
        let held = as_long_as::read(r, None)?;
        r.punct(',').then_some(held)
    });
    r.words("damage doesn't cause you to lose life")
        .then_some(())?;
    let condition = condition.or_else(|| r.attempt(|r| as_long_as::read(r, None)));
    Some(Ability::Rule {
        rule: GameRule::NoLifeLossFromDamage(Ref::You),
        condition,
        from: ZoneKind::Battlefield,
    })
}

/// What `no_life_loss` reads, written back.
fn no_life_loss_texts(condition: Option<&Condition>) -> Vec<String> {
    let rule = "damage doesn't cause you to lose life";
    match condition {
        None => vec![rule.to_owned()],
        Some(held) => as_long_as::texts(held)
            .into_iter()
            .flat_map(|held| {
                [
                    format!("as long as {held}, {rule}"),
                    format!("{rule} as long as {held}"),
                ]
            })
            .collect(),
    }
}

/// The replacement of the card's own entering.
fn replacement(by: ReplaceBy, condition: Option<Condition>) -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by,
        condition,
        from: ZoneKind::Battlefield,
    }
}

/// "~ enters tapped.", "~ enters tapped unless you control two or more basic lands.",
/// "~ enters with two +1/+1 counters on it.", "~ enters tapped with two charge counters on it.",
/// "~ escapes with a +1/+1 counter on it."
fn entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    let escaped = r.word("escapes");
    if !escaped {
        r.need("enters")?;
    }
    let tapped = !escaped && r.word("tapped");
    if tapped && !r.words("with") {
        let condition = if r.word("unless") {
            Some(Condition::Not(Box::new(holds(r)?)))
        } else {
            None
        };
        return Some(replacement(ReplaceBy::Tapped, condition));
    }
    if !tapped {
        r.need("with")?;
    }
    // "enters with X +1/+1 counters on it, where X is the number of ...": the where clause gives X its count.
    let named = where_x::ahead(r);
    let outer = std::mem::replace(&mut r.where_x, named);
    let found = counters::entering(r);
    r.where_x = outer;
    let (counter, mut count) = found?;
    r.need("on it")?;
    if named {
        count = where_x::counted(r)?;
    }
    Some(replacement(
        ReplaceBy::WithCounters {
            counter,
            count,
            tapped,
        },
        escaped.then_some(Condition::CastWith(CastWay::Escape)),
    ))
}

/// "If you control two or more other lands, ~ enters tapped.": the card enters tapped when what a player controls holds.
fn entering_tapped_if(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if")?;
    let condition = control(r)?;
    r.punct(',').then_some(())?;
    r.source().then_some(())?;
    r.need("enters tapped")?;
    Some(replacement(ReplaceBy::Tapped, Some(condition)))
}

/// "If you control a commander, you may cast this spell without paying its mana cost.": a cast open while the condition holds.
fn free_cast(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if")?;
    let condition = control(r)?;
    r.punct(',').then_some(())?;
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("without paying its mana cost")?;
    Some(Ability::FreeCast {
        condition: Some(condition),
    })
}

/// "Creatures your opponents control enter tapped.", "Artifacts and creatures your opponents control enter tapped.": a
/// replacement of the entering of every permanent the group names.
fn group_enters_tapped(r: &mut Reader<'_>) -> Option<Ability> {
    let what = phrase(r, Form::All)?;
    let permanents = !what.0.is_empty()
        && !what
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::Spell | Pred::This | Pred::In(_)));
    permanents.then_some(())?;
    r.words("enter tapped").then_some(())?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by: ReplaceBy::Tapped,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// "As ~ enters, choose a color.", "As it enters, choose a color other than red.", "As ~ enters, you may pay 2 life.
/// If you don't, it enters tapped."
fn as_entering(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("as")?;
    (r.source() || r.word("it")).then_some(())?;
    r.need("enters")?;
    r.punct(',').then_some(())?;
    if r.words("choose a color other than") {
        let other = color(r.take_word()?)?;
        return Some(replacement(
            ReplaceBy::Choose(EntersChoice::ColorOtherThan(other)),
            None,
        ));
    }
    if r.words("choose a color") {
        return Some(replacement(ReplaceBy::Choose(EntersChoice::Color), None));
    }
    if r.words("choose a creature type") {
        return Some(replacement(
            ReplaceBy::Choose(EntersChoice::CreatureType),
            None,
        ));
    }
    if r.words("choose a player") {
        return Some(replacement(ReplaceBy::Choose(EntersChoice::Player), None));
    }
    if r.words("choose an opponent") {
        return Some(replacement(ReplaceBy::Choose(EntersChoice::Opponent), None));
    }
    r.need("you may pay")?;
    let life = r.number().filter(|life| *life > 0)?;
    r.need("life")?;
    r.punct('.').then_some(())?;
    r.need("if you don't")?;
    r.punct(',').then_some(())?;
    r.need("it enters tapped")?;
    Some(replacement(ReplaceBy::PayLifeOrTapped(life), None))
}

/// What must hold for a permanent not to enter tapped: one thing, or several joined by "or" ("you have two or more
/// opponents or a player has 13 or less life").
pub(super) fn holds(r: &mut Reader<'_>) -> Option<Condition> {
    let mut any = vec![held(r)?];
    while let Some(next) = r.attempt(|r| {
        r.need("or")?;
        held(r)
    }) {
        any.push(next);
    }
    if any.len() == 1 {
        any.pop()
    } else {
        Some(Condition::Any(any))
    }
}

/// One thing that can hold: what a player controls, how many opponents there are, or how low a life total is.
fn held(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(control)
        .or_else(|| r.attempt(opponents))
        .or_else(|| r.attempt(low_life))
}

/// "you have two or more opponents".
fn opponents(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you have")?;
    let number = number_word(r.take_word()?)?;
    r.need("or more opponents")?;
    Some(Condition::Compare {
        left: Amount::Players(Ref::Opponents),
        op: Cmp::Ge,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// The players a life total is about and whether one of them is enough ("a player", "an opponent") or all must have it
/// ("each opponent", "each player").
const LIFE_SUBJECTS: [(&str, Ref, bool); 4] = [
    ("a player has", Ref::EachPlayer, true),
    ("an opponent has", Ref::Opponents, true),
    ("each opponent has", Ref::Opponents, false),
    ("each player has", Ref::EachPlayer, false),
];

/// "a player has 13 or less life", "an opponent has 10 or less life", "each opponent has 20 or more life": the lowest or
/// highest life total among the players, whichever one player or every player having it comes to.
fn low_life(r: &mut Reader<'_>) -> Option<Condition> {
    let (who, any) = LIFE_SUBJECTS
        .iter()
        .find_map(|(words, who, any)| r.words(words).then(|| (who.clone(), *any)))?;
    let life = i32::try_from(r.number()?).ok()?;
    let less = if r.words("or less life") {
        true
    } else if r.words("or more life") {
        false
    } else {
        return None;
    };
    // One player with at most the amount is the lowest total at most it; all of them is the highest total at most it.
    let left = if any == less {
        Amount::LowestLife(who)
    } else {
        Amount::HighestLife(who)
    };
    Some(Condition::Compare {
        left,
        op: if less { Cmp::Le } else { Cmp::Ge },
        right: Amount::Fixed(life),
    })
}

/// What a permanent must control for it not to enter tapped: "you control a Forest or a Plains",
/// "you control two or fewer other lands", "your opponents control eight or more lands".
pub(super) fn control(r: &mut Reader<'_>) -> Option<Condition> {
    let who = if r.words("you control") {
        Ref::You
    } else if r.words("your opponents control") {
        Ref::Opponents
    } else {
        return None;
    };
    let (op, number, selector) = if r.words("your commander") {
        (Cmp::Ge, 1, your_commander())
    } else if r.word("a") || r.word("an") {
        (Cmp::Ge, 1, one_kind(r)?)
    } else {
        let number = number_word(r.take_word()?)?;
        let op = if r.words("or more") {
            Cmp::Ge
        } else if r.words("or fewer") {
            Cmp::Le
        } else {
            return None;
        };
        (op, number, phrase(r, Form::All)?)
    };
    // Who controls it is the verb's, not a suffix of the phrase.
    let mut preds = selector.0;
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_)))
    {
        return None;
    }
    preds.push(Pred::ControlledBy(who));
    Some(Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// "your commander": the card its owner designated, whoever controls it now (the controller is the verb's).
fn your_commander() -> Selector {
    Selector(vec![Pred::Commander, Pred::OwnedBy(Ref::You)])
}

/// One thing after "a": "planeswalker", "basic land", "Forest or a Plains", "Mount or Vehicle".
fn one_kind(r: &mut Reader<'_>) -> Option<Selector> {
    let first = phrase(r, Form::Subject)?;
    let [Pred::Subtype(_)] = first.0.as_slice() else {
        return Some(first);
    };
    let mut kinds = first.0;
    while r.word("or") {
        let _ = r.word("a") || r.word("an");
        kinds.push(Pred::Subtype(Subtype(subtype(r)?)));
    }
    Some(Selector(if kinds.len() > 1 {
        vec![Pred::AnyOf(kinds)]
    } else {
        kinds
    }))
}

/// "~ can't be countered.", "~ attacks each combat if able.", "All creatures attack each combat if able.",
/// "Play with the top card of your library revealed."
fn rule(r: &mut Reader<'_>) -> Option<Ability> {
    let rule = if r.words("play with the top card of your library revealed") {
        GameRule::RevealTop(Ref::You)
    } else if let Some(rule) = r.attempt(player_rule) {
        rule
    } else if r.source() {
        if r.words("can be your commander") {
            GameRule::CanBeCommander
        } else if r.words("can't be countered") {
            return Some(Ability::Rule {
                rule: GameRule::CantBeCountered(Ref::This),
                condition: None,
                from: ZoneKind::Stack,
            });
        } else if r.words("attacks each combat if able") {
            GameRule::MustAttack(Ref::This)
        } else if r.words("blocks each combat if able") {
            GameRule::MustBlock(Ref::This)
        } else {
            return None;
        }
    } else {
        let who = plural(r)?;
        if r.words("attack each combat if able") {
            GameRule::MustAttack(who)
        } else if r.words("block each combat if able") {
            GameRule::MustBlock(who)
        } else {
            return None;
        }
    };
    Some(Ability::Rule {
        rule,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The rules about a player's hand, land plays and library: "You have no maximum hand size.", "You may play an
/// additional land on each of your turns.", "You may look at the top card of your library any time."
fn player_rule(r: &mut Reader<'_>) -> Option<GameRule> {
    if r.words("you have no maximum hand size") {
        return Some(GameRule::NoMaxHandSize(Ref::You));
    }
    if r.words("players have no maximum hand size") {
        return Some(GameRule::NoMaxHandSize(Ref::EachPlayer));
    }
    if r.words("skip your draw step") {
        return Some(GameRule::SkipDrawStep(Ref::You));
    }
    if r.words("you may look at the top card of your library any time") {
        return Some(GameRule::LookAtTopAnyTime(Ref::You));
    }
    if let Some(rule) = life_gain::rule(r) {
        return Some(rule);
    }
    lands::additional(r)
}

/// A group of permanents: "all creatures", "creatures you control", "other creatures you control",
/// "creatures you control of the chosen type".
pub(super) fn plural(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(objects) = all(r) {
        return Some(objects);
    }
    let Selector(mut preds) = phrase(r, Form::All)?;
    if r.words("of the chosen color") {
        preds.push(Pred::ChosenColor);
    } else if r.words("of the chosen type") {
        preds.push(Pred::ChosenSubtype);
    }
    Some(Ref::Each(Selector(preds)))
}

/// The permanent a static ability changes: the card, what it is attached to or a group.
pub(super) fn subject(r: &mut Reader<'_>, itself: bool) -> Option<(Ref, bool)> {
    if itself && r.source() {
        return Some((Ref::This, false));
    }
    let attached = r.attempt(|r| {
        (r.word("equipped") || r.word("enchanted")).then_some(())?;
        r.word("creature").then_some(())
    });
    if attached.is_some() {
        return Some((Ref::Attached, false));
    }
    if let Some(group) = each(r) {
        return Some((group, false));
    }
    plural(r).map(|group| (group, true))
}

/// "Enchanted land has ...": an Aura that enchants something other than a creature gives it an ability, and the data
/// holds the permanent as `Attached` all the same. Only a verb "has" follows; the writer says "land", "artifact" and
/// "permanent" for an ability given and no other noun.
fn enchanted_other(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    r.attempt(|r| {
        r.word("enchanted").then_some(())?;
        (r.word("land") || r.word("artifact") || r.word("permanent")).then_some(())?;
        (r.peek_word() == Some("has")).then_some((Ref::Attached, false))
    })
}

/// "~ can't block.", "Enchanted creature can't attack or block.", "~ can't be blocked.", "~ can't be blocked by
/// creatures with power 2 or less."
fn cannot(r: &mut Reader<'_>) -> Option<Ability> {
    // The condition may come first: "As long as you have the city's blessing, Detectives you control can't be blocked."
    let leading = leading_condition(r);
    let (affected, _) = subject(r, true)?;
    r.need("can't")?;
    let mods = if r.words("attack or block") {
        vec![Modification::CantAttack, Modification::CantBlock]
    } else if r.words("block and can't be blocked") {
        vec![Modification::CantBlock, Modification::CantBeBlocked]
    } else if r.words("be blocked by more than one creature") {
        vec![Modification::MaxBlockers(1)]
    } else if r.words("be blocked by") {
        vec![Modification::CantBeBlockedBy(phrase(r, Form::All)?)]
    } else if r.words("be blocked except by") {
        if let Some(count) = evasion::or_more(r) {
            vec![Modification::MinBlockers(count)]
        } else {
            vec![Modification::CantBeBlockedBy(evasion::except(phrase(
                r,
                Form::All,
            )?)?)]
        }
    } else if r.words("be blocked") {
        vec![Modification::CantBeBlocked]
    } else if r.word("attack") {
        vec![Modification::CantAttack]
    } else if r.word("block") {
        vec![Modification::CantBlock]
    } else {
        return None;
    };
    let condition = leading.or_else(|| r.attempt(|r| as_long_as::read(r, Some(&affected))));
    Some(statics(affected, mods, condition))
}

/// The condition a sentence opens with: "As long as it's your turn, ...".
pub(super) fn leading_condition(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(|r| {
        // "During your turn, creature tokens you control get +1/+4."
        let condition = as_long_as::read(r, None)
            .or_else(|| r.words("during your turn").then_some(Condition::YourTurn))?;
        r.punct(',').then_some(condition)
    })
}

/// "Equipped creature gets +2/+0.", "Creatures you control have haste.", "Enchanted creature gets
/// +1/+1 and has flying.", "Equipped creature gets +2/+2 and has protection from white and from black.", "~ has haste as
/// long as an opponent has 10 or less life."; an X means nothing to a permanent, so it is not read. The card itself is
/// given something only under a condition.
fn modifies(r: &mut Reader<'_>) -> Option<Ability> {
    // The condition may come first: "As long as you're the monarch, permanents you control have hexproof."
    let leading = leading_condition(r);
    // After a condition about the card itself, "it" is the card: "As long as ~ is monstrous, it has menace."
    let it = leading
        .as_ref()
        .and_then(as_long_as::subject_of)
        .filter(|_| r.word("it"));
    // "As long as ~ has three or more fire counters on it, it's an artifact creature."
    let its = leading
        .as_ref()
        .and_then(as_long_as::subject_of)
        .filter(|_| it.is_none() && r.word("it's"));
    if let Some(its) = its {
        r.words("an artifact creature").then_some(())?;
        return Some(statics(
            its,
            super::becomes::artifact_creature_mods(),
            leading,
        ));
    }
    let (affected, plural) = if let Some(it) = it {
        (it, false)
    } else {
        subject(r, true).or_else(|| enchanted_other(r))?
    };
    // "Equipped creature gets +X/+X, where X is the number of creature cards in all graveyards."
    let named = where_x::ahead(r);
    let x_free = std::mem::replace(&mut r.x_free, false);
    let outer = std::mem::replace(&mut r.where_x, named);
    let granting = std::mem::replace(&mut r.static_grant, true);
    let mods = r.attempt(|r| {
        // "Enchanted creature is a Treefolk with base power and toughness 0/4 and loses all abilities."
        if affected == Ref::Attached
            && let Some(form) = r.attempt(aura_forms::form)
        {
            return Some(form);
        }
        predicate_mods(r, plural)
    });
    r.x_free = x_free;
    r.where_x = outer;
    r.static_grant = granting;
    let mut mods = mods?;
    if named {
        let count = where_x::counted(r)?;
        mods = where_x::put_in(&mods, &count)?;
    }
    let condition = leading.or_else(|| r.attempt(|r| as_long_as::read(r, Some(&affected))));
    if affected == Ref::This
        && condition.is_none()
        && !counts_stats(&mods)
        && !chosen_protection(&mods)
    {
        return None;
    }
    Some(statics(affected, mods, condition))
}

/// What a permanent gets and has: "gets +1/+1", "has flying", "gets +1/+1 and has flying", and as the last part "is
/// goaded" ("gets +2/+2 and is goaded", "has indestructible and is goaded", "gets +1/+1, has deathtouch, and is goaded").
pub(super) fn predicate_mods(r: &mut Reader<'_>, plural: bool) -> Option<Vec<Modification>> {
    let (gets, has, is) = if plural {
        ("get", "have", "are")
    } else {
        ("gets", "has", "is")
    };
    // The last part: "and is goaded", "and can't be blocked by more than one creature".
    let lose = if plural { "lose" } else { "loses" };
    let tail = |r: &mut Reader<'_>| {
        if r.words(&format!("and {is} goaded")) {
            Some(Modification::Goaded)
        } else if r.words("and can't be blocked by more than one creature") {
            Some(Modification::MaxBlockers(1))
        } else if r.words(&format!("and {lose} all creature types")) {
            Some(Modification::SetCreatureTypes(Vec::new()))
        } else {
            None
        }
    };
    if !plural && r.words("isn't a creature") {
        return Some(vec![Modification::RemoveType(CardType::Creature)]);
    }
    if r.words(&format!("{lose} all creature types")) {
        return Some(vec![Modification::SetCreatureTypes(Vec::new())]);
    }
    let mut mods = Vec::new();
    if r.word(gets) {
        let stats = power_toughness(r)?;
        mods.push(counted_stats(r, stats)?);
        if let Some(last) = tail(r) {
            mods.push(last);
            return Some(mods);
        }
        // ", where X is ..." names the X of the change and ends the clause.
        if r.at_punct(',') && r.word_at(1) == Some("where") {
            return Some(mods);
        }
        let listed = r.punct(',');
        if listed || r.word("and") {
            // "Equipped creature gets +1/+0 and is a Monk in addition to its other types."
            if let Some(kind) = r.attempt(|r| of_a_kind(r, is)) {
                mods.push(kind);
                return Some(mods);
            }
            // "Equipped creature gets +10/+10 and loses flying."
            if let Some(lost) = r.attempt(|r| {
                r.word(lose).then_some(())?;
                keyword(r)
            }) {
                mods.push(Modification::RemoveKeyword(lost));
                return Some(mods);
            }
            r.word(has).then_some(())?;
            mods.extend(gives(r)?);
        }
        if listed {
            r.punct(',').then_some(())?;
            if let Some(last) = tail(r) {
                mods.push(last);
            } else {
                mods.push(of_a_kind(r, is)?);
            }
        }
        return Some(mods);
    }
    // "Enchanted creature has base power and toughness 8/8 and has ...": the size comes first.
    if let Some(base) = r.attempt(|r| base_stats(r, has)) {
        mods.push(base);
        if r.words(&format!("and {has}")) {
            mods.extend(gives(r)?);
        }
        mods.extend(tail(r));
        return Some(mods);
    }
    r.word(has).then_some(())?;
    mods.extend(gives(r)?);
    mods.extend(tail(r));
    Some(mods)
}

/// "has base power and toughness 8/8": the size a permanent has whatever its card prints.
fn base_stats(r: &mut Reader<'_>, has: &str) -> Option<Modification> {
    r.word(has).then_some(())?;
    r.words("base power and toughness").then_some(())?;
    let power = i32::try_from(r.number()?).ok()?;
    r.punct('/').then_some(())?;
    let toughness = i32::try_from(r.number()?).ok()?;
    Some(Modification::SetPT {
        power: Amount::Fixed(power),
        toughness: Amount::Fixed(toughness),
    })
}

/// "and is a Monk in addition to its other types" (the "and" optional): the creature type a permanent gets as well.
fn of_a_kind(r: &mut Reader<'_>, is: &str) -> Option<Modification> {
    r.word("and");
    r.word(is).then_some(())?;
    (r.word("a") || r.word("an")).then_some(())?;
    let kind = subtype(r)?;
    r.words("in addition to its other types")
        .then_some(Modification::AddSubtype(Subtype(kind)))
}

/// What a permanent is given: keywords, or one activated or triggered ability in quotes ("Enchanted creature has
/// \"{R}, {T}: Copy target instant or sorcery spell you control.\"", "Lands you control have \"{T}: Add one mana of any
/// color.\"", "Sliver creatures you control have \"When this creature enters, ...\"").
pub(super) fn gives(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    if let Some(ability) = quoted_ability(r) {
        return Some(vec![Modification::GrantAbility(Arc::new(ability))]);
    }
    let mut mods = granted(r)?;
    // "Enchanted creature has flying and \"Sacrifice a creature: ~ gets +2/+1 until end of turn.\""
    if let [Modification::AddKeyword(_)] = mods.as_slice()
        && let Some(ability) = r.attempt(|r| {
            r.need("and")?;
            quoted_ability(r)
        })
    {
        mods.push(Modification::GrantAbility(Arc::new(ability)));
    }
    Some(mods)
}

/// One activated or triggered ability in quotes.
pub(super) fn quoted_ability(r: &mut Reader<'_>) -> Option<Ability> {
    r.quoted(abilities::activated)
        .or_else(|| r.quoted(abilities::triggered))
}

/// What `gives` reads, written back.
pub(super) fn given_text(mods: &[Modification]) -> Option<String> {
    match mods {
        [Modification::GrantAbility(ability)]
            if matches!(
                **ability,
                Ability::Activated { .. } | Ability::Triggered { .. }
            ) =>
        {
            Some(format!("\"{}\"", render::ability(ability)?))
        }
        [
            keyword @ Modification::AddKeyword(_),
            Modification::GrantAbility(ability),
        ] if matches!(
            **ability,
            Ability::Activated { .. } | Ability::Triggered { .. }
        ) =>
        {
            Some(format!(
                "{} and \"{}\"",
                granted_text(std::slice::from_ref(keyword))?,
                render::ability(ability)?
            ))
        }
        _ => granted_text(mods),
    }
}

/// The count of a spell's own cost increase for each target beyond the first.
fn beyond_the_first() -> Amount {
    Amount::Sum(vec![Amount::TargetCount(0), Amount::Fixed(-1)])
}

/// Whether the count is the targets beyond the first.
pub(super) fn counts_targets(per: &Amount) -> bool {
    *per == beyond_the_first()
}

/// The spells a cost modifier names: "This spell", "Noncreature spells", "Artifact spells you cast", "Spells your opponents
/// cast", "Spells you cast from anywhere other than your hand". The flag says the spell is the card itself.
pub(super) fn cost_subject(r: &mut Reader<'_>) -> Option<(Selector, bool)> {
    if r.source() {
        return Some((Selector::this(), true));
    }
    let first = match r.attempt(cost_conditions::shared_spells) {
        Some(shared) => shared,
        None => phrase(r, Form::All)?,
    };
    let mut applies_to = cost_conditions::more_spells(r, first)?;
    (applies_to.0.first() == Some(&Pred::Spell)).then_some(())?;
    if r.words("you cast") {
        applies_to.0.push(Pred::ControlledBy(Ref::You));
    } else if r.words("your opponents cast") {
        applies_to.0.push(Pred::ControlledBy(Ref::Opponents));
    }
    // "Creature spells you cast with power 4 or greater": a number of the spell after who casts it.
    if let Some(number) = r.attempt(|r| match selectors::suffix(r, Form::All)? {
        number @ Pred::Compare { .. } => Some(number),
        _ => None,
    }) {
        applies_to.0.push(number);
    }
    if r.words("from anywhere other than your hand") {
        applies_to.0.push(not_cast_from_hand());
    }
    Some((applies_to, false))
}

/// "Cast from anywhere other than your hand".
fn not_cast_from_hand() -> Pred {
    Pred::Not(Box::new(Pred::CastFrom(ZoneKind::Hand)))
}

/// The mana of a cost change and whether it is `{X}` (then one mana is changed as many times as "where X is" counts).
fn cost_mana(r: &mut Reader<'_>) -> Option<(ManaCostX, bool)> {
    let mut text = String::new();
    while let Some(symbol) = r.symbol() {
        text.extend(["{", symbol, "}"]);
    }
    let mana: ManaCostX = text.parse().ok()?;
    if text == "{X}" {
        return Some(("{1}".parse().ok()?, true));
    }
    (!mana.has_x()).then_some((mana, false))
}

/// How many times the mana changes the cost: once, for each target beyond the first, for each thing counted, or as
/// many times as the number "where X is" gives.
fn cost_times(r: &mut Reader<'_>, itself: bool, x: bool) -> Option<Amount> {
    if x {
        r.punct(',').then_some(())?;
        r.need("where x is")?;
        let amount = defining::counting(r)?;
        return matches!(amount, Amount::Greatest { .. } | Amount::Total { .. }).then_some(amount);
    }
    if itself && r.words("for each target beyond the first") {
        return Some(beyond_the_first());
    }
    Some(r.attempt(for_each).unwrap_or_else(Amount::one))
}

/// "Noncreature spells cost {1} more to cast.", "Artifact spells you cast cost {1} less to cast.", "Spells your
/// opponents cast cost {1} more to cast.", "This spell costs {1} more to cast for each target beyond the first.",
/// "This spell costs {1} less to cast for each creature card in your graveyard.", "This spell costs {X} less to cast,
/// where X is the total power of creatures you control."
fn cost_modifier(r: &mut Reader<'_>) -> Option<Ability> {
    let leading = r.attempt(cost_conditions::leading);
    let (applies_to, itself) = cost_subject(r)?;
    r.need(if itself { "costs" } else { "cost" })?;
    let (mana, x) = cost_mana(r)?;
    let change = if r.word("more") {
        CostChange::Increase
    } else if r.word("less") {
        CostChange::Reduce
    } else {
        return None;
    };
    r.need("to cast")?;
    let per = cost_times(r, itself, x)?;
    let condition = match leading {
        Some(held) => Some(held),
        None => r.attempt(cost_conditions::trailing),
    };
    Some(Ability::CostModifier {
        applies_to,
        change,
        mana,
        per,
        condition,
    })
}

/// The spells a cost modifier names, as the card words them before "cost".
pub(super) fn cost_subject_text(applies_to: &Selector) -> Option<String> {
    if *applies_to == Selector::this() {
        return Some("~".to_owned());
    }
    let mut other = false;
    let (mut who, mut spells) = (None, Vec::new());
    for pred in &applies_to.0 {
        match pred {
            Pred::ControlledBy(Ref::You) if who.is_none() => who = Some(" you cast"),
            Pred::ControlledBy(Ref::Opponents) if who.is_none() => {
                who = Some(" your opponents cast");
            }
            Pred::ControlledBy(_) => return None,
            _ if *pred == not_cast_from_hand() => other = true,
            _ => spells.push(pred.clone()),
        }
    }
    let selector = Selector(spells);
    (selector.0.first() == Some(&Pred::Spell)).then_some(())?;
    let cast = who.unwrap_or("");
    let group = match selector.0.as_slice() {
        [Pred::Spell, Pred::AnyOf(kinds)]
            if kinds.len() > 1
                && kinds
                    .iter()
                    .all(|kind| matches!(kind, Pred::Subtype(_) | Pred::Color(_))) =>
        {
            cost_conditions::spells_text(kinds)?
        }
        _ => phrase_text(&selector, Form::All)?,
    };
    let from = if other && who.is_some() {
        " from anywhere other than your hand"
    } else if other {
        return None;
    } else {
        ""
    };
    Some(format!("{group}{cast}{from}"))
}

/// The wording of a cost modifier the template reads.
fn cost_modifier_texts(
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
    per: &Amount,
    condition: Option<&Condition>,
) -> Vec<String> {
    let Some(subject) = cost_subject_text(applies_to) else {
        return Vec::new();
    };
    let mut subjects = Vec::new();
    subjects.extend(cost_conditions::shared_subject_text(applies_to));
    subjects.extend(cost_conditions::cast_before_number(&subject));
    subjects.push(subject);
    subjects
        .into_iter()
        .flat_map(|subject| cost_sentences(&subject, applies_to, change, mana, per, condition))
        .collect()
}

/// The sentences of a cost modifier whose spells are worded as `subject`.
fn cost_sentences(
    subject: &str,
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
    per: &Amount,
    condition: Option<&Condition>,
) -> Vec<String> {
    let itself = *applies_to == Selector::this();
    let verb = if itself { "costs" } else { "cost" };
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    let (mana, tail) = if per.is_one() {
        (mana.to_string(), String::new())
    } else if itself && *per == beyond_the_first() {
        (
            mana.to_string(),
            " for each target beyond the first".to_owned(),
        )
    } else if let Some(each) = for_each_text(per) {
        (mana.to_string(), format!(" for each {each}"))
    } else if matches!(per, Amount::Greatest { .. } | Amount::Total { .. })
        && mana.to_string() == "{1}"
    {
        let Some(count) = defining::counting_text(per) else {
            return Vec::new();
        };
        ("{X}".to_owned(), format!(", where X is {count}"))
    } else {
        return Vec::new();
    };
    let sentence = format!("{subject} {verb} {mana} {more} to cast{tail}");
    let Some(held) = condition else {
        return vec![sentence];
    };
    cost_conditions::texts(held)
        .into_iter()
        .flat_map(|fact| {
            [
                format!("{sentence} if {fact}"),
                format!("if {fact}, {sentence}"),
            ]
        })
        .collect()
}

pub(super) fn statics(
    affected: Ref,
    mods: Vec<Modification>,
    condition: Option<Condition>,
) -> Ability {
    Ability::Static {
        affected,
        mods,
        condition,
        defining: false,
        from: ZoneKind::Battlefield,
    }
}

/// Every wording of a static ability, a replacement of the entering or a rule the readers produce;
/// empty for any other ability.
pub fn texts(ability: &Ability) -> Vec<String> {
    let sentences = match ability {
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by,
            condition,
            from: ZoneKind::Battlefield,
        } if *what == Selector::this() => entering_texts(by, condition.as_ref()),
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by: ReplaceBy::Tapped,
            condition: None,
            from: ZoneKind::Battlefield,
        } => phrase_text(what, Form::All)
            .map(|group| format!("{group} enter tapped"))
            .into_iter()
            .collect(),
        Ability::Rule {
            rule: GameRule::NoLifeLossFromDamage(Ref::You),
            condition,
            from: ZoneKind::Battlefield,
        } => no_life_loss_texts(condition.as_ref()),
        Ability::Rule {
            rule,
            condition: None,
            from,
        } => rule_texts(rule, *from),
        Ability::Static {
            affected: affected @ Ref::Each(Selector(spells)),
            mods,
            condition: None,
            defining: false,
            from: ZoneKind::Battlefield,
        } if spells.first() == Some(&Pred::Spell) => super::spell_keywords::texts(affected, mods),
        Ability::Static {
            affected: affected @ Ref::Each(Selector(cards)),
            mods,
            condition: None,
            defining: false,
            from: ZoneKind::Battlefield,
        } if super::card_grants::names_cards(cards) => super::card_grants::texts(affected, mods),
        Ability::Static {
            affected,
            mods,
            condition,
            defining: false,
            from: ZoneKind::Battlefield,
        } => {
            let mut written = static_texts(affected, mods, condition.as_ref());
            written.extend(evasion::texts(affected, mods, condition.as_ref()));
            written
        }
        Ability::CostModifier {
            applies_to,
            change,
            mana,
            per,
            condition,
        } => cost_modifier_texts(applies_to, *change, mana, per, condition.as_ref()),
        Ability::FreeCast {
            condition: Some(held),
        } => control_texts(held)
            .into_iter()
            .map(|held| format!("if {held}, you may cast ~ without paying its mana cost"))
            .collect(),
        Ability::Replacement {
            when: ReplaceWhen::LifeGain { .. },
            ..
        } => life_gain::replacement_sentences(ability),
        Ability::Replacement {
            when,
            by,
            condition: None,
            from: ZoneKind::Battlefield,
        } if replacements::handles(when) => replacements::sentences(when, by),
        Ability::Static { defining: true, .. } => defining::sentences(ability),
        _ => Vec::new(),
    };
    sentences
        .iter()
        .map(|sentence| format!("{}.", capitalize(sentence)))
        .collect()
}

fn entering_texts(by: &ReplaceBy, condition: Option<&Condition>) -> Vec<String> {
    match (by, condition) {
        (ReplaceBy::Tapped, None) => vec!["~ enters tapped".to_owned()],
        (ReplaceBy::Tapped, Some(Condition::Not(held))) => holds_texts(held)
            .into_iter()
            .map(|held| format!("~ enters tapped unless {held}"))
            .collect(),
        (ReplaceBy::Tapped, Some(held)) => control_texts(held)
            .into_iter()
            .map(|held| format!("if {held}, ~ enters tapped"))
            .collect(),
        (
            ReplaceBy::WithCounters {
                counter,
                count,
                tapped,
            },
            None,
        ) => {
            let tapped = if *tapped { " tapped" } else { "" };
            // A counted number is X, named at the end of the sentence.
            let (shown, tail) = match defining::value_text(count) {
                Some(said) => (Amount::X, format!(", where X is {said}")),
                None => (count.clone(), String::new()),
            };
            counters::entering_text(counter, &shown)
                .map(|counters| format!("~ enters{tapped} with {counters} on it{tail}"))
                .into_iter()
                .collect()
        }
        (
            ReplaceBy::WithCounters { counter, count, .. },
            Some(Condition::CastWith(CastWay::Escape)),
        ) => counters::entering_text(counter, count)
            .map(|counters| format!("~ escapes with {counters} on it"))
            .into_iter()
            .collect(),
        (ReplaceBy::Choose(EntersChoice::Color), None) => {
            vec!["As ~ enters, choose a color".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::ColorOtherThan(other)), None) => {
            let other = color_name(*other);
            vec![
                format!("As ~ enters, choose a color other than {other}"),
                format!("As it enters, choose a color other than {other}"),
            ]
        }
        (ReplaceBy::Choose(EntersChoice::CreatureType), None) => {
            vec!["As ~ enters, choose a creature type".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::Player), None) => {
            vec!["As ~ enters, choose a player".to_owned()]
        }
        (ReplaceBy::Choose(EntersChoice::Opponent), None) => {
            vec!["As ~ enters, choose an opponent".to_owned()]
        }
        (ReplaceBy::PayLifeOrTapped(life), None) => vec![format!(
            "As ~ enters, you may pay {life} life. If you don't, it enters tapped"
        )],
        (ReplaceBy::DayIfNeither, None) => {
            vec!["If it's neither day nor night, it becomes day as ~ enters".to_owned()]
        }
        (
            ReplaceBy::AsCopy {
                of,
                except,
                tapped,
                until_end_of_turn,
            },
            None,
        ) => copies::enters_texts(of, except, *tapped, *until_end_of_turn),
        _ => Vec::new(),
    }
}

/// What holds for a permanent not to enter tapped, as `holds` reads it: one thing, or several joined by "or".
pub(super) fn holds_texts(condition: &Condition) -> Vec<String> {
    let (list, joiner) = match condition {
        Condition::Any(list) => (list, " or "),
        Condition::All(list) => (list, " and "),
        _ => return control_texts(condition),
    };
    list.iter().fold(vec![String::new()], |written, item| {
        let choices = control_texts(item);
        written
            .iter()
            .flat_map(|start| {
                choices.iter().map(move |choice| {
                    if start.is_empty() {
                        choice.clone()
                    } else if joiner == " and " {
                        // "you control an Urza's Mine and an Urza's Power-Plant": the verb is said once.
                        let rest = choice.strip_prefix("you control ").unwrap_or(choice);
                        format!("{start}{joiner}{rest}")
                    } else {
                        format!("{start}{joiner}{choice}")
                    }
                })
            })
            .collect()
    })
}

/// "you control two or more basic lands", "you have two or more opponents", "an opponent has 10 or less life": one thing
/// that holds, as `held` reads it.
pub(super) fn control_texts(condition: &Condition) -> Vec<String> {
    if let Condition::Compare {
        left: Amount::Players(Ref::Opponents),
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    } = condition
    {
        return u32::try_from(*number)
            .ok()
            .and_then(count_word)
            .map(|words| format!("you have {words} or more opponents"))
            .into_iter()
            .collect();
    }
    if let Condition::Compare {
        left: left @ (Amount::LowestLife(who) | Amount::HighestLife(who)),
        op: op @ (Cmp::Le | Cmp::Ge),
        right: Amount::Fixed(life),
    } = condition
    {
        let (less, any) = (
            *op == Cmp::Le,
            matches!(left, Amount::LowestLife(_)) == (*op == Cmp::Le),
        );
        return LIFE_SUBJECTS
            .iter()
            .find(|(_, subject, one)| subject == who && *one == any)
            .map(|(words, _, _)| {
                let end = if less { "less" } else { "more" };
                format!("{words} {life} or {end} life")
            })
            .into_iter()
            .collect();
    }
    let Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return Vec::new();
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    let mut rest: Vec<Pred> = preds.clone();
    let Some(at) = rest
        .iter()
        .position(|pred| matches!(pred, Pred::ControlledBy(_)))
    else {
        return Vec::new();
    };
    let (verb, one) = match rest.remove(at) {
        Pred::ControlledBy(Ref::You) => ("you control", true),
        Pred::ControlledBy(Ref::Opponents) => ("your opponents control", false),
        _ => return Vec::new(),
    };
    let selector = Selector(rest);
    if one && *op == Cmp::Ge && number == 1 && selector == your_commander() {
        return vec![format!("{verb} your commander")];
    }
    match (op, number) {
        (Cmp::Ge, 1) if one => single_texts(&selector)
            .into_iter()
            .map(|kind| format!("{verb} {kind}"))
            .collect(),
        (Cmp::Ge | Cmp::Le, 2..) | (Cmp::Le, 1) => {
            let (Some(words), Some(group)) =
                (count_word(number), phrase_text(&selector, Form::All))
            else {
                return Vec::new();
            };
            let more = if *op == Cmp::Ge { "more" } else { "fewer" };
            vec![format!("{verb} {words} or {more} {group}")]
        }
        _ => Vec::new(),
    }
}

/// "a planeswalker", "a Forest or a Plains" and "a Forest or Plains".
fn single_texts(selector: &Selector) -> Vec<String> {
    if let [Pred::AnyOf(kinds)] = selector.0.as_slice() {
        let names: Option<Vec<&str>> = kinds
            .iter()
            .map(|kind| match kind {
                Pred::Subtype(Subtype(name)) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        let Some((first, others)) = names.as_deref().and_then(|names| names.split_first()) else {
            return Vec::new();
        };
        let with_articles: Vec<String> = others
            .iter()
            .map(|name| format!("{} {name}", article(name)))
            .collect();
        return vec![
            format!(
                "{} {first} or {}",
                article(first),
                with_articles.join(" or ")
            ),
            format!("{} {first} or {}", article(first), others.join(" or ")),
        ];
    }
    if selector.0.contains(&Pred::Other) {
        return Vec::new();
    }
    phrase_text(selector, Form::Subject)
        .map(|text| format!("{} {text}", article(&text)))
        .into_iter()
        .collect()
}

fn rule_texts(rule: &GameRule, from: ZoneKind) -> Vec<String> {
    match (rule, from) {
        (GameRule::CantBeCountered(Ref::This), ZoneKind::Stack) => {
            vec!["~ can't be countered".to_owned()]
        }
        (GameRule::RevealTop(Ref::You), ZoneKind::Battlefield) => {
            vec!["Play with the top card of your library revealed".to_owned()]
        }
        (GameRule::NoMaxHandSize(Ref::You), ZoneKind::Battlefield) => {
            vec!["You have no maximum hand size".to_owned()]
        }
        (GameRule::NoMaxHandSize(Ref::EachPlayer), ZoneKind::Battlefield) => {
            vec!["Players have no maximum hand size".to_owned()]
        }
        (GameRule::SkipDrawStep(Ref::You), ZoneKind::Battlefield) => {
            vec!["Skip your draw step".to_owned()]
        }
        (GameRule::LookAtTopAnyTime(Ref::You), ZoneKind::Battlefield) => {
            vec!["You may look at the top card of your library any time".to_owned()]
        }
        (GameRule::AdditionalLandPlays { who, count }, ZoneKind::Battlefield) => {
            lands::additional_texts(who, count)
        }
        (GameRule::MayCast(permission), ZoneKind::Battlefield) => {
            lands::from_a_zone_text(permission)
                .into_iter()
                .chain(lands::cast_from_the_top_text(permission))
                .collect()
        }
        (GameRule::CantGainLife(who), ZoneKind::Battlefield) => life_gain::rule_sentences(who),
        (GameRule::CanBeCommander, ZoneKind::Battlefield) => {
            vec!["~ can be your commander".to_owned()]
        }
        (GameRule::MustAttack(who), ZoneKind::Battlefield) => each_combat(who, "attack"),
        (GameRule::MustBlock(who), ZoneKind::Battlefield) => each_combat(who, "block"),
        (GameRule::DamageCantBePrevented { source, combat }, ZoneKind::Battlefield) => {
            prevention::static_texts(source, *combat)
        }
        (GameRule::AttackTax { .. }, ZoneKind::Battlefield) => {
            super::attack_tax::static_texts(rule)
        }
        _ => Vec::new(),
    }
}

/// "~ attacks each combat if able", "all creatures attack each combat if able".
fn each_combat(who: &Ref, verb: &str) -> Vec<String> {
    subjects(who, true)
        .into_iter()
        .map(|(subject, plural)| {
            let verb = if plural {
                verb.to_owned()
            } else {
                format!("{verb}s")
            };
            format!("{subject} {verb} each combat if able")
        })
        .collect()
}

/// The subjects of a static ability and whether each is plural: the card, what it is attached to
/// (an Equipment says "equipped", an Aura "enchanted") or a group.
pub(super) fn subjects(affected: &Ref, itself: bool) -> Vec<(String, bool)> {
    match affected {
        Ref::You => vec![("You".to_owned(), true)],
        Ref::This if itself => vec![("~".to_owned(), false)],
        Ref::Attached => vec![
            ("Equipped creature".to_owned(), false),
            ("Enchanted creature".to_owned(), false),
        ],
        Ref::Each(selector) => {
            let (preds, chosen) = match selector.0.split_last() {
                Some((Pred::ChosenColor, rest)) => (rest, " of the chosen color"),
                Some((Pred::ChosenSubtype, rest)) => (rest, " of the chosen type"),
                _ => (selector.0.as_slice(), ""),
            };
            let chosen_inside = preds
                .iter()
                .any(|pred| matches!(pred, Pred::ChosenColor | Pred::ChosenSubtype));
            if chosen_inside {
                return Vec::new();
            }
            // One at a time: "Each creature you control with a +1/+1 counter on it has flying."
            let each = phrase_text(&Selector(preds.to_vec()), Form::Each)
                .map(|one| (format!("Each {one}{chosen}"), false));
            let Some(group) = phrase_text(&Selector(preds.to_vec()), Form::All) else {
                return each.into_iter().collect();
            };
            // "All creatures", but "Creatures you control" and "Attacking creatures".
            let prefix = if !preds.is_empty() && preds.iter().all(is_a_type) {
                "all "
            } else {
                ""
            };
            let plain = format!("{prefix}{group}{chosen}");
            // Slivers and tokens are everyone's: "All Sliver creatures get +1/+1", but "Creature tokens get -1/-0".
            let everyones = prefix.is_empty()
                && preds
                    .iter()
                    .any(|pred| matches!(pred, Pred::Subtype(_) | Pred::Token))
                && !preds
                    .iter()
                    .any(|pred| matches!(pred, Pred::ControlledBy(_) | Pred::Other));
            let mut written = if everyones {
                vec![(format!("all {plain}"), true), (plain, true)]
            } else {
                vec![(plain, true)]
            };
            written.extend(each);
            written
        }
        _ => Vec::new(),
    }
}

fn static_texts(
    affected: &Ref,
    mods: &[Modification],
    condition: Option<&Condition>,
) -> Vec<String> {
    // The wordings of the condition, each as the tail of the sentence; one empty tail for no condition.
    let helds: Vec<String> = condition.map(as_long_as::texts).unwrap_or_default();
    let tails: Vec<String> = if condition.is_some() {
        helds
            .iter()
            .map(|held| format!(" as long as {held}"))
            .collect()
    } else {
        vec![String::new()]
    };
    let mut subjects = subjects(affected, true);
    // An Aura that gives a keyword or an ability may enchant a land, an artifact or any permanent.
    if *affected == Ref::Attached
        && !mods.is_empty()
        && mods.iter().all(|m| {
            matches!(
                m,
                Modification::GrantAbility(_) | Modification::AddKeyword(_)
            )
        })
        && mods
            .iter()
            .filter(|m| matches!(m, Modification::GrantAbility(_)))
            .count()
            <= 1
    {
        subjects.extend(
            [
                "Enchanted land",
                "Enchanted artifact",
                "Enchanted permanent",
            ]
            .map(|subject| (subject.to_owned(), false)),
        );
    }
    subjects
        .into_iter()
        .flat_map(|(subject, plural)| {
            let predicates = predicate_variants(
                mods,
                plural,
                matches!(affected, Ref::This),
                condition.is_some(),
            );
            predicates
                .into_iter()
                .map(move |predicate| (subject.clone(), predicate))
        })
        .flat_map(|(subject, predicate)| {
            // The condition may also come first: "As long as you're the monarch, creatures you control have flying."
            let leading = helds.iter().map(|held| {
                let mut words = subject.chars();
                let lowered: String = words.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(words).collect()
                });
                format!("as long as {held}, {lowered} {predicate}")
            });
            let trailing = tails
                .iter()
                .map(|tail| format!("{subject} {predicate}{tail}"));
            let during = (condition == Some(&Condition::YourTurn)).then(|| {
                let mut words = subject.chars();
                let lowered: String = words.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(words).collect()
                });
                format!("during your turn, {lowered} {predicate}")
            });
            // "As long as ~ is monstrous, it has menace."
            let pronoun = helds
                .iter()
                .filter(|held| {
                    ["~ ", "equipped creature ", "enchanted creature "]
                        .iter()
                        .any(|start| held.starts_with(start))
                })
                .map(|held| match predicate.strip_prefix("is ") {
                    Some(rest) => format!("as long as {held}, it's {rest}"),
                    None => format!("as long as {held}, it {predicate}"),
                });
            trailing
                .chain(leading)
                .chain(pronoun)
                .chain(during)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The predicate in each wording of the ability it gives in quotes: a triggered ability says "it deals" where another
/// wording says "~ deals".
pub(super) fn predicate_variants(
    mods: &[Modification],
    plural: bool,
    itself: bool,
    conditional: bool,
) -> Vec<String> {
    let mut written = if itself && !conditional && !chosen_protection(mods) {
        Vec::new()
    } else {
        static_parts::predicates(mods, plural)
    };
    if !plural && !itself {
        written.extend(aura_forms::texts(mods));
    }
    let Some(first) = predicate(mods, plural, itself, conditional) else {
        return written;
    };
    written.insert(0, first.clone());
    for modification in mods {
        let Modification::GrantAbility(ability) = modification else {
            continue;
        };
        let Some(plain) = render::ability(ability) else {
            continue;
        };
        for inner in render::variants(ability) {
            if inner != plain {
                written.push(first.replace(&format!("\"{plain}\""), &format!("\"{inner}\"")));
            }
        }
    }
    count_wordings(&mut written);
    written
}

/// Whether the changes are protection from what the card's controller chose as it entered, which a card gives itself
/// without a printed keyword line: "This creature has protection from the chosen color."
fn chosen_protection(mods: &[Modification]) -> bool {
    !mods.is_empty()
        && mods.iter().all(|modification| {
            matches!(
                modification,
                Modification::AddKeyword(Keyword::Protection(Selector(preds)))
                    if preds.contains(&Pred::ChosenColor) || preds.contains(&Pred::ChosenSubtype)
            )
        })
}

/// The other ways a card words what a bonus counts: "for each opponent you have" and "for each artifact and/or
/// enchantment you control" for the "for each opponent" and "... artifact or enchantment ..." the writer says.
fn count_wordings(written: &mut Vec<String>) {
    let mut more = Vec::new();
    for text in written.iter() {
        let Some(at) = text.find("for each ") else {
            continue;
        };
        let (head, count) = text.split_at(at);
        if let Some(rest) = count.strip_prefix("for each opponent")
            && !rest.starts_with('s')
        {
            more.push(format!("{head}for each opponent you have{rest}"));
        }
        if count.contains(" or ") {
            more.push(format!("{head}{}", count.replacen(" or ", " and/or ", 1)));
        }
    }
    written.extend(more);
}

/// The modifications before the last part of a sentence and the last part: "is goaded", "can't be blocked by more than
/// one creature", "lose all creature types" and "is a Monk in addition to its other types".
fn last_part(mods: &[Modification], plural: bool) -> (&[Modification], Option<String>) {
    let is = if plural { "are" } else { "is" };
    match mods.split_last() {
        Some((Modification::Goaded, rest)) if !rest.is_empty() => {
            (rest, Some(format!("{is} goaded")))
        }
        Some((Modification::MaxBlockers(1), rest)) if !rest.is_empty() => (
            rest,
            Some("can't be blocked by more than one creature".to_owned()),
        ),
        Some((Modification::SetCreatureTypes(types), rest))
            if types.is_empty() && !rest.is_empty() =>
        {
            let lose = if plural { "lose" } else { "loses" };
            (rest, Some(format!("{lose} all creature types")))
        }
        Some((Modification::AddSubtype(Subtype(kind)), rest)) if !rest.is_empty() && !plural => (
            rest,
            Some(format!(
                "is {} {kind} in addition to its other types",
                article(kind)
            )),
        ),
        _ => (mods, None),
    }
}

/// "can't attack or block", "gets +1/+1 and has flying", "have haste". The card itself gets a bonus or a keyword only
/// under a condition, and a restriction is not read under one.
pub(super) fn predicate(
    mods: &[Modification],
    plural: bool,
    itself: bool,
    conditional: bool,
) -> Option<String> {
    match mods {
        [Modification::CantAttack, Modification::CantBlock] => {
            return Some("can't attack or block".to_owned());
        }
        [Modification::CantAttack] => return Some("can't attack".to_owned()),
        [Modification::CantBlock] => return Some("can't block".to_owned()),
        [Modification::CantBeBlocked] => {
            return Some("can't be blocked".to_owned());
        }
        [Modification::MaxBlockers(1)] => {
            return Some("can't be blocked by more than one creature".to_owned());
        }
        [Modification::MinBlockers(count @ 2..)] => {
            return Some(format!(
                "can't be blocked except by {} or more creatures",
                count_word(*count)?
            ));
        }
        [Modification::CanBlockOnly(only)] => {
            return Some(format!("can block only {}", phrase_text(only, Form::All)?));
        }
        [Modification::CantBlock, Modification::CantBeBlocked] => {
            return Some("can't block and can't be blocked".to_owned());
        }
        [Modification::SetCreatureTypes(types)] if types.is_empty() => {
            return Some(format!(
                "{} all creature types",
                if plural { "lose" } else { "loses" }
            ));
        }
        // Only a condition takes a type away: "As long as your devotion to white is less than five, ~ isn't a creature."
        [Modification::RemoveType(CardType::Creature)] if conditional && !plural => {
            return Some("isn't a creature".to_owned());
        }
        // A Vehicle that is a creature as long as it holds enough counters.
        [
            Modification::AddType(CardType::Artifact),
            Modification::AddType(CardType::Creature),
        ] if conditional && itself && !plural => {
            return Some("is an artifact creature".to_owned());
        }
        // A card does not give itself a bonus, but it may grow by a count: "This creature gets +1/+1 for each Elf."
        _ if !conditional && itself && !counts_stats(mods) && !chosen_protection(mods) => {
            return None;
        }
        _ => {}
    }
    let (gets, has) = if plural {
        ("get", "have")
    } else {
        ("gets", "has")
    };
    let (mods, last) = last_part(mods, plural);
    let (stats, rest) = match mods.split_first() {
        Some((Modification::ModifyPT { power, toughness }, rest)) => {
            let (stats, tail) = stats_text(power, toughness)?;
            (Some(format!("{gets} {stats}{tail}")), rest)
        }
        Some((
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            },
            rest,
        )) => (
            Some(format!(
                "{has} base power and toughness {power}/{toughness}"
            )),
            rest,
        ),
        _ => (None, mods),
    };
    let mut parts: Vec<String> = stats.into_iter().collect();
    if !rest.is_empty() {
        parts.push(format!("{has} {}", given_text(rest)?));
    }
    if let Some(last) = last {
        return match parts.as_slice() {
            [one] => Some(format!("{one} and {last}")),
            [first, second] => Some(format!("{first}, {second}, and {last}")),
            _ => None,
        };
    }
    (!parts.is_empty()).then(|| parts.join(" and "))
}
