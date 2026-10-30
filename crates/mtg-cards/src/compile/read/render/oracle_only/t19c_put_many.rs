//! Cards of the hand put onto the battlefield, more than one: "put any number of creature cards from your hand onto the
//! battlefield" (Ghalta, Stampede Tyrant) and "put up to two creature cards from your hand onto the battlefield". The
//! plain wording is the choice of one card; its phrase is swapped for the count and the plural.

use super::super::super::amounts::count_word;
use super::super::super::selectors::{Form, phrase_text};
use super::super::{article, capitalize, variants};
use super::twin_json::{inner_mut, twin};
use super::w4b_stand::swapped_in;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use serde_json::Value;

/// Adds the wordings of an ability that chooses several cards of the hand and puts them onto the battlefield.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((bind, from, count)) = many_from_hand(ability.effects()) else {
        return;
    };
    let Some((one, many)) = phrases(&from, &count) else {
        return;
    };
    let plain = twin(ability, |value| unchoose(value, &bind));
    let Some(plain) = plain else {
        return;
    };
    let wordings = variants(&plain);
    texts.extend(swapped_in(&wordings, &[(&one, &many)]));
    texts.extend(swapped_in(
        &wordings,
        &[(&capitalize(&one), &capitalize(&many))],
    ));
}

/// The name, the cards and the number of the first choice of several cards of the hand whose move puts them onto the
/// battlefield untouched.
fn many_from_hand(effects: &[Effect]) -> Option<(String, Selector, Bounds)> {
    effects.windows(2).find_map(|pair| {
        let [
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(from),
                count,
                bind,
                rest_as: None,
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to:
                    Destination::Battlefield {
                        under: None,
                        transformed: false,
                        counters,
                        ..
                    },
            },
        ] = pair
        else {
            return None;
        };
        let in_hand = from
            .0
            .starts_with(&[Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
        (in_hand && bind == moved && counters.is_empty() && !count.is_one())
            .then(|| (bind.clone(), from.clone(), count.clone()))
    })
}

/// The words of the plain choice of one card and of the choice of the count: "put a creature card from your hand" and
/// "put any number of creature cards from your hand".
fn phrases(from: &Selector, count: &Bounds) -> Option<(String, String)> {
    let single = phrase_text(from, Form::Subject)?;
    let plural = phrase_text(from, Form::All)?;
    let number = match (&count.min, &count.max) {
        (Amount::Fixed(0), None) => "any number of".to_owned(),
        (Amount::Fixed(0), Some(Amount::Fixed(most @ 2..))) => {
            format!("up to {}", count_word(u32::try_from(*most).ok()?)?)
        }
        _ => return None,
    };
    Some((
        format!("put {} {single} ", article(&single)),
        format!("put {number} {plural} "),
    ))
}

/// The ability's JSON with the number of the choice that binds `bind` left out, which makes it a choice of one card.
fn unchoose(value: &mut Value, bind: &str) -> Option<()> {
    let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
    let choice = effects.iter_mut().find_map(|effect| {
        let choose = effect.get_mut("Choose")?;
        (choose.get("as")?.as_str() == Some(bind)).then_some(choose)
    })?;
    choice.as_object_mut()?.remove("count").map(|_| ())
}
