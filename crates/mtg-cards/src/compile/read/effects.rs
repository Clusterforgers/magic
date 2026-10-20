//! Effects: a sentence is read into the effects of one template. A template reads the whole
//! sentence or nothing; to add a wording, write a template and list it in [`TEMPLATES`].

use super::amounts::{cards, its_stat, points};
use super::selectors::{Form, all, each, keyword, phrase, target};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Targetable};
use crate::value::Amount;

/// A template: the effects of the sentence it reads, or `None` when the sentence is not its shape.
type Template = fn(&mut Reader<'_>) -> Option<Vec<Effect>>;

/// The sentence shapes, tried in this order.
const TEMPLATES: [Template; 5] = [chain, player_effect, damage, removal, pump];

/// Reads the next sentence and its period into `effects`; false when it is not a shape the
/// templates read.
pub fn sentence(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> bool {
    if r.attempt(|r| regeneration(r, effects)).is_some() {
        return true;
    }
    for template in TEMPLATES {
        let read = r.attempt(|r| {
            let found = template(r)?;
            r.sentence_end().then_some(found)
        });
        if let Some(found) = read {
            effects.extend(found);
            return true;
        }
    }
    false
}

/// "It can't be regenerated." changes the destruction before it.
fn regeneration(r: &mut Reader<'_>, effects: &mut [Effect]) -> Option<()> {
    let Some(Effect::Destroy { what, regenerate }) = effects.last_mut() else {
        return None;
    };
    let single = matches!(what, Ref::Target(_)) && r.targets.first()?.bounds.is_one();
    r.need(if single {
        "it can't be regenerated"
    } else {
        "they can't be regenerated"
    })?;
    if !r.sentence_end() || !*regenerate {
        return None;
    }
    *regenerate = false;
    Some(())
}

/// Steps of "you" joined by ", then": "Scry 1, then draw a card."
fn chain(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = vec![imperative(r)?];
    while let Some(next) = r.attempt(|r| {
        r.punct(',').then_some(())?;
        r.need("then")?;
        imperative(r)
    }) {
        effects.push(next);
    }
    Some(effects)
}

fn imperative(r: &mut Reader<'_>) -> Option<Effect> {
    let who = Ref::You;
    Some(match r.take_word()? {
        "scry" => Effect::Scry {
            who,
            count: points(r)?,
        },
        "draw" => Effect::Draw {
            who,
            count: cards(r)?,
        },
        "mill" => Effect::Mill {
            who,
            count: cards(r)?,
        },
        "discard" => {
            let count = cards(r)?;
            let random = r.words("at random");
            Effect::Discard { who, count, random }
        }
        _ => return None,
    })
}

/// The player an effect is about: "you", "target player", "each opponent", "its controller".
fn player(r: &mut Reader<'_>) -> Option<Ref> {
    if r.word("you") {
        return Some(Ref::You);
    }
    if r.words("each opponent") {
        return Some(Ref::Opponents);
    }
    if r.words("each player") {
        return Some(Ref::EachPlayer);
    }
    if r.words("its controller") {
        return r.it.clone().map(|it| Ref::ControllerOf(Box::new(it)));
    }
    r.attempt(|r| {
        let who = target(r)?;
        let spec = r.targets.first()?;
        (matches!(spec.what, Targetable::Player | Targetable::Opponent) && spec.bounds.is_one())
            .then_some(who)
    })
}

/// "gain 3 life", "lose X life", "gain life equal to its power": what follows the verb.
fn life(r: &mut Reader<'_>) -> Option<Amount> {
    if r.word("life") {
        r.need("equal to")?;
        return its_stat(r);
    }
    let amount = points(r)?;
    r.word("life").then_some(amount)
}

/// A player draws, discards, mills, gains or loses.
fn player_effect(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = player(r)?;
    let effect = match r.take_word()? {
        "draw" | "draws" => Effect::Draw {
            who,
            count: cards(r)?,
        },
        "mill" | "mills" => Effect::Mill {
            who,
            count: cards(r)?,
        },
        "discard" | "discards" => {
            let count = cards(r)?;
            let random = r.words("at random");
            Effect::Discard { who, count, random }
        }
        "gain" | "gains" => Effect::GainLife {
            who,
            amount: life(r)?,
        },
        "lose" | "loses" => Effect::LoseLife {
            who,
            amount: life(r)?,
        },
        _ => return None,
    };
    Some(vec![effect])
}

/// Who or what takes damage: a single target, "each opponent", "each creature".
fn recipient(r: &mut Reader<'_>) -> Option<Ref> {
    if r.words("each opponent") {
        return Some(Ref::Opponents);
    }
    if r.words("each player") {
        return Some(Ref::EachPlayer);
    }
    if r.word("you") {
        return Some(Ref::You);
    }
    if let Some(objects) = each(r) {
        return Some(objects);
    }
    r.attempt(|r| {
        let to = target(r)?;
        let spec = r.targets.first()?;
        (spec.bounds.is_one() && spec.divide.is_none()).then_some(to)
    })
}

/// "~ deals 3 damage to any target."
fn damage(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.source().then_some(())?;
    r.need("deals")?;
    let amount = points(r)?;
    r.need("damage to")?;
    let to = recipient(r)?;
    Some(vec![Effect::DealDamage {
        source: None,
        to,
        amount,
    }])
}

/// A target object chosen one at a time.
fn target_object(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        (matches!(spec.what, Targetable::Object(_)) && spec.bounds.is_one()).then_some(what)
    })
}

/// What a verb acts on: target objects, or "all creatures".
fn object(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(objects) = all(r) {
        return Some(objects);
    }
    r.attempt(|r| {
        let what = target(r)?;
        matches!(r.targets.first()?.what, Targetable::Object(_)).then_some(what)
    })
}

/// "Destroy target creature.", "Exile all creatures.", "Counter target spell."
fn removal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let verb = r.take_word()?;
    if !matches!(verb, "destroy" | "exile" | "counter") {
        return None;
    }
    let what = object(r)?;
    let spell = r.targets.first().is_some_and(|spec| {
        matches!(&spec.what, Targetable::Object(selector) if selector.0.first() == Some(&Pred::Spell))
    });
    if verb == "counter" && !spell {
        return None;
    }
    if matches!(what, Ref::Target(_)) && r.targets.first()?.bounds.is_one() && !spell {
        r.it = Some(what.clone());
    }
    Some(vec![match verb {
        "destroy" => Effect::Destroy {
            what,
            regenerate: true,
        },
        "exile" => Effect::Exile { what },
        _ => Effect::Counter { what },
    }])
}

/// The object a pump changes and whether it is several: "target creature", "~", "creatures you control".
fn subject(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.source() {
        return Some((Ref::This, false));
    }
    if let Some(what) = target_object(r) {
        return Some((what, false));
    }
    phrase(r, Form::All).map(|selector| (Ref::Each(selector), true))
}

/// "+2/+2", "-3/-3", "+X/+X".
fn signed(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        if r.punct('+') {
            if r.x_free && r.word("x") {
                return Some(Amount::X);
            }
            return Some(Amount::Fixed(i32::try_from(r.number()?).ok()?));
        }
        r.punct('-').then_some(())?;
        Some(Amount::Fixed(-i32::try_from(r.number()?).ok()?))
    })
}

fn power_toughness(r: &mut Reader<'_>) -> Option<Modification> {
    let power = signed(r)?;
    r.punct('/').then_some(())?;
    let toughness = signed(r)?;
    Some(Modification::ModifyPT { power, toughness })
}

/// "flying", "flying and haste": keywords given.
fn granted(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let mut mods = vec![Modification::AddKeyword(keyword(r)?)];
    while let Some(next) = r.attempt(|r| {
        r.need("and")?;
        keyword(r)
    }) {
        mods.push(Modification::AddKeyword(next));
    }
    Some(mods)
}

/// "Target creature gets +3/+3 until end of turn.", "... gains flying until end of turn."
fn pump(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (affected, plural) = subject(r)?;
    let (gets, gains) = if plural {
        ("get", "gain")
    } else {
        ("gets", "gains")
    };
    let mut mods = Vec::new();
    if r.word(gets) {
        mods.push(power_toughness(r)?);
        if r.word("and") {
            r.word(gains).then_some(())?;
            mods.extend(granted(r)?);
        }
    } else {
        r.word(gains).then_some(())?;
        mods = granted(r)?;
    }
    r.need("until end of turn")?;
    Some(vec![Effect::Continuous {
        affected,
        mods,
        duration: Duration::UntilEndOfTurn,
    }])
}
