//! Lands played beyond the first and from other places: "You may play an additional land on each of your turns.", "You may
//! play up to three additional lands this turn.", "You may play lands from your graveyard.", "You may play lands from the
//! top of your library." Read from the wording and written back the same way.

use super::amounts::{count_word, number_word};
use super::effects::Template;
use super::selectors::{Form, phrase, phrase_text, plural_subtype};
use super::subtypes::plural_subtype as plural_subtype_word;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{CastPermission, Duration, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use mtg_core::card::{CardType, Subtype};

/// The effects of a spell: the land plays of one turn.
pub const TEMPLATES: [Template; 1] = [additional_this_turn];

/// How many more lands: "an additional land", "two additional lands", "up to three additional lands".
fn how_many(r: &mut Reader<'_>, up_to: bool) -> Option<u32> {
    if up_to {
        r.need("up to")?;
    }
    let count = number_word(r.take_word()?)?;
    let plural = r.word("additional") && r.peek_word() == Some("lands");
    if plural {
        r.skip();
    } else {
        r.need("land")?;
    }
    (plural == (count > 1)).then_some(count)
}

/// "You may play an additional land this turn.", "You may play up to three additional lands this turn."
fn additional_this_turn(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may play")?;
    let count = how_many(r, r.peek_word() == Some("up"))?;
    r.need("this turn")?;
    Some(vec![Effect::Rule {
        rule: GameRule::AdditionalLandPlays {
            who: Ref::You,
            count: Amount::Fixed(i32::try_from(count).ok()?),
        },
        duration: Duration::UntilEndOfTurn,
    }])
}

/// The effect `additional_this_turn` reads, written.
pub fn effect_text(effect: &Effect) -> Option<String> {
    let Effect::Rule {
        rule:
            GameRule::AdditionalLandPlays {
                who: Ref::You,
                count: Amount::Fixed(count),
            },
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    let count = u32::try_from(*count).ok().filter(|count| *count > 0)?;
    Some(if count == 1 {
        "you may play an additional land this turn".to_owned()
    } else {
        format!(
            "you may play up to {} additional lands this turn",
            count_word(count)?
        )
    })
}

/// "You may play an additional land on each of your turns.", "Each player may play an additional land on each of their
/// turns."
pub fn additional(r: &mut Reader<'_>) -> Option<GameRule> {
    let who = if r.words("you may play") {
        Ref::You
    } else {
        r.need("each player may play")?;
        Ref::EachPlayer
    };
    let count = how_many(r, false)?;
    let own = if who == Ref::You { "your" } else { "their" };
    (r.words("on each of") || r.words("during each of")).then_some(())?;
    r.need(own)?;
    r.need("turns")?;
    Some(GameRule::AdditionalLandPlays {
        who,
        count: Amount::Fixed(i32::try_from(count).ok()?),
    })
}

/// The wordings of the rule `additional` reads.
pub fn additional_texts(who: &Ref, count: &Amount) -> Vec<String> {
    let Amount::Fixed(count) = count else {
        return Vec::new();
    };
    let Some(count) = u32::try_from(*count).ok().filter(|count| *count > 0) else {
        return Vec::new();
    };
    let (subject, own) = match who {
        Ref::You => ("You may play", "your"),
        Ref::EachPlayer => ("Each player may play", "their"),
        _ => return Vec::new(),
    };
    let lands = if count == 1 {
        "an additional land".to_owned()
    } else if let Some(word) = count_word(count) {
        format!("{word} additional lands")
    } else {
        return Vec::new();
    };
    ["on", "during"]
        .iter()
        .map(|preposition| format!("{subject} {lands} {preposition} each of {own} turns"))
        .collect()
}

/// "You may play lands from your graveyard.", "You may play lands from the top of your library."
pub fn from_a_zone(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may play")?;
    // "lands", or the lands of a basic land type: "Forests".
    let lands = if r.word("lands") {
        Selector(vec![Pred::Type(CardType::Land)])
    } else {
        Selector(vec![Pred::Subtype(Subtype(plural_subtype(r)?))])
    };
    is_lands(&lands).then_some(())?;
    r.need("from")?;
    let from = if r.words("your graveyard") {
        ZoneKind::Graveyard
    } else {
        r.need("the top of your library")?;
        ZoneKind::Library
    };
    Some(Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            who: Ref::You,
            what: lands,
            from: Some(from),
            free: false,
            flash: false,
            only: None,
            play: true,
            once: false,
        }),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The basic land types, as the subtypes of the lands a permission may name.
const BASIC_LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// Whether the selector names lands: all of them, or the lands of one basic land type.
fn is_lands(selector: &Selector) -> bool {
    match selector.0.as_slice() {
        [Pred::Type(CardType::Land)] => true,
        [Pred::Subtype(Subtype(kind))] => BASIC_LAND_TYPES.contains(&kind.as_str()),
        _ => false,
    }
}

/// The wording of the permission `from_a_zone` reads.
pub fn from_a_zone_text(permission: &CastPermission) -> Option<String> {
    let ordinary = permission.who == Ref::You
        && is_lands(&permission.what)
        && !permission.free
        && !permission.flash
        && permission.only.is_none()
        && permission.play;
    if !ordinary {
        return None;
    }
    let lands = match permission.what.0.as_slice() {
        [Pred::Subtype(Subtype(kind))] => plural_subtype_word(kind),
        _ => "lands".to_owned(),
    };
    match permission.from {
        Some(ZoneKind::Graveyard) => Some(format!("You may play {lands} from your graveyard")),
        Some(ZoneKind::Library) => {
            Some(format!("You may play {lands} from the top of your library"))
        }
        _ => None,
    }
}

/// "You may cast creature spells from the top of your library.", "You may cast Merfolk spells from the top of your
/// library.": a `MayCast` permission over the cards that match, cast from the library.
pub fn cast_from_the_top(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    let spells = phrase(r, Form::All)?;
    let [Pred::Spell, kinds @ ..] = spells.0.as_slice() else {
        return None;
    };
    (!kinds.is_empty()).then_some(())?;
    r.need("from the top of your library")?;
    Some(Ability::Rule {
        rule: GameRule::MayCast(CastPermission {
            who: Ref::You,
            what: Selector(kinds.to_vec()),
            from: Some(ZoneKind::Library),
            free: false,
            flash: false,
            only: None,
            play: false,
            once: false,
        }),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The wording of the permission `cast_from_the_top` reads.
pub fn cast_from_the_top_text(permission: &CastPermission) -> Option<String> {
    let ordinary = permission.who == Ref::You
        && permission.from == Some(ZoneKind::Library)
        && !permission.free
        && !permission.flash
        && permission.only.is_none()
        && !permission.play
        && !permission.what.0.is_empty();
    if !ordinary {
        return None;
    }
    let mut preds = vec![Pred::Spell];
    preds.extend(permission.what.0.iter().cloned());
    let spells = phrase_text(&Selector(preds), Form::All)?;
    Some(format!(
        "You may cast {spells} from the top of your library"
    ))
}
