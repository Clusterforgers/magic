//! The sticker wording (CR 123): "put a sticker on a nonland permanent you own", "put a name sticker on it", "put up to
//! two name stickers on it", "distribute up to two art stickers among one or two nonland permanents you own", read into
//! `Effect::PutSticker` and written back the same way.

use super::amounts::{count_word, number_word};
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::ability::StickerKind;
use crate::did::Action;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};

/// Whether the objects are ones the player owns: a sticker goes only on an object its player owns (CR 123).
fn owned(on: &Selector) -> bool {
    on.0.contains(&Pred::OwnedBy(Ref::You))
}

/// The kind word before "sticker" or "stickers", if any.
fn kind(r: &mut Reader<'_>) -> Option<StickerKind> {
    [
        StickerKind::Name,
        StickerKind::Ability,
        StickerKind::Art,
        StickerKind::PowerToughness,
    ]
    .into_iter()
    .find(|kind| r.words(kind.word()))
}

/// "it" (the object with the ability), or the objects after "a"/"an" that the player owns.
fn on(r: &mut Reader<'_>) -> Option<Selector> {
    if r.word("it") {
        return Some(Selector(vec![Pred::This]));
    }
    (r.word("a") || r.word("an")).then_some(())?;
    owned_phrase(r, Form::Subject)
}

/// Objects of a phrase that the player owns: "... you own", or cards "in your graveyard".
fn owned_phrase(r: &mut Reader<'_>, form: Form) -> Option<Selector> {
    let mut on = phrase(r, form)?;
    if r.words("you own") {
        (!owned(&on)).then_some(())?;
        on.0.push(Pred::OwnedBy(Ref::You));
    }
    owned(&on).then_some(on)
}

/// "put a sticker on a nonland permanent you own", "put a name sticker on it", "put up to two name stickers on it" and
/// "distribute up to two art stickers among one or two nonland permanents you own".
pub(super) fn un34_put_sticker(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.word("distribute") {
        r.need("up to")?;
        let up_to = number_word(r.take_word()?).filter(|n| *n >= 2)?;
        let kind = kind(r);
        r.need("stickers among one or")?;
        (count_word(up_to) == Some(r.take_word()?)).then_some(())?;
        let on = owned_phrase(r, Form::Targets)?;
        return Some(vec![Effect::PutSticker {
            who: Ref::You,
            on,
            kind,
            up_to,
        }]);
    }
    r.need("put")?;
    let up_to = if r.words("up to") {
        number_word(r.take_word()?).filter(|n| *n >= 2)?
    } else {
        (r.word("a") || r.word("an")).then_some(())?;
        0
    };
    let kind = kind(r);
    r.need(if up_to == 0 {
        "sticker on"
    } else {
        "stickers on"
    })?;
    let on = on(r)?;
    Some(vec![Effect::PutSticker {
        who: Ref::You,
        on,
        kind,
        up_to,
    }])
}

/// What `un34_put_sticker` read, written back for "you".
pub(super) fn un34_put_text(
    who: &Ref,
    on: &Selector,
    kind: Option<StickerKind>,
    up_to: u32,
) -> Option<String> {
    if *who != Ref::You {
        return None;
    }
    let word = kind.map_or(String::new(), |kind| format!("{} ", kind.word()));
    if on.0 == [Pred::This] {
        return Some(if up_to == 0 {
            let article = kind.map_or("a", StickerKind::article);
            format!("put {article} {word}sticker on it")
        } else {
            format!("put up to {} {word}stickers on it", count_word(up_to)?)
        });
    }
    if !owned(on) {
        return None;
    }
    let in_zone = on.0.iter().any(|pred| matches!(pred, Pred::In(_)));
    // The phrase writes "you own" where the owner stands among its words.
    let (rest, own) = (on.clone(), "");
    if up_to == 0 {
        let form = if in_zone { Form::Count } else { Form::Subject };
        let objects = phrase_text(&rest, form)?;
        let article = kind.map_or("a", StickerKind::article);
        return Some(format!(
            "put {article} {word}sticker on {} {objects}{own}",
            super::triggers::article(&objects)
        ));
    }
    let objects = phrase_text(&rest, Form::Targets)?;
    let count = count_word(up_to)?;
    Some(format!(
        "distribute up to {count} {word}stickers among one or {count} {objects}{own}"
    ))
}

/// After "with": "an art sticker on it", "name stickers on them", "an art sticker" (the end of a phrase).
pub(super) fn un34_with_sticker(r: &mut Reader<'_>) -> Option<Pred> {
    let plural = !(r.word("a") || r.word("an"));
    let kind = kind(r)?;
    r.need(if plural { "stickers" } else { "sticker" })?;
    let _ = r.words("on it") || r.words("on them");
    Some(Pred::WithSticker(kind))
}

/// After "for each": "name sticker on it", "sticker on it".
pub(super) fn un34_stickers_on(r: &mut Reader<'_>) -> Option<crate::value::Amount> {
    let kind = kind(r);
    r.need("sticker on it")?;
    Some(crate::value::Amount::Stickers {
        on: Ref::This,
        kind,
    })
}

/// After "When" or "Whenever": "you place a sticker", "you put an art sticker on a creature".
pub(super) fn un34_sticker_event(r: &mut Reader<'_>) -> Option<crate::trigger::TriggerPattern> {
    r.need("you")?;
    if r.words("place a sticker") {
        return Some(crate::trigger::TriggerPattern::Did {
            action: Action::PutSticker,
            who: Ref::You,
            what: Selector::default(),
        });
    }
    r.need("put")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let action = Action::sticker_put(kind(r)?)?;
    r.need("sticker on")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let what = phrase(r, Form::Subject)?;
    Some(crate::trigger::TriggerPattern::Did {
        action,
        who: Ref::You,
        what,
    })
}

/// The words of [`un34_sticker_event`]'s patterns, after "Whenever".
pub(super) fn un34_sticker_event_text(
    action: Action,
    who: &Ref,
    what: &Selector,
) -> Option<String> {
    if *who != Ref::You {
        return None;
    }
    if action == Action::PutSticker && what.is_empty() {
        return Some("Whenever you place a sticker".to_owned());
    }
    let kind = [StickerKind::Name, StickerKind::Ability, StickerKind::Art]
        .into_iter()
        .find(|kind| Action::sticker_put(*kind) == Some(action))?;
    let objects = phrase_text(what, Form::Subject)?;
    Some(format!(
        "Whenever you put {} {} sticker on {} {objects}",
        kind.article(),
        kind.word(),
        super::triggers::article(&objects)
    ))
}
