//! The ban on paying costs with life or sacrifices (`Prohibited::PayCosts`): "Players can't pay life or sacrifice
//! creatures to cast spells or activate abilities.", "Players can't pay life to cast spells or to activate abilities
//! that aren't mana abilities."

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::modify::{GameRule, Prohibited};
use crate::refs::{Ref, Selector};

/// The end of the sentence: what the payments are for.
fn r4707_ends(but_mana_abilities: bool) -> &'static str {
    if but_mana_abilities {
        "to cast spells or to activate abilities that aren't mana abilities"
    } else {
        "to cast spells or activate abilities"
    }
}

/// The players a ban names, as the subject of "can't".
fn r4707_players(who: &Ref) -> Option<&'static str> {
    match who {
        Ref::EachPlayer => Some("Players"),
        Ref::Opponents => Some("Your opponents"),
        Ref::You => Some("You"),
        _ => None,
    }
}

/// Reads the ban: "players can't pay life or sacrifice nonland permanents to cast spells or activate abilities".
pub(super) fn read(r: &mut Reader<'_>) -> Option<GameRule> {
    let who = if r.words("players") {
        Ref::EachPlayer
    } else if r.words("your opponents") {
        Ref::Opponents
    } else {
        return None;
    };
    r.need("can't")?;
    let life = r.words("pay life");
    let sacrifice = if (!life || r.words("or")) && r.words("sacrifice") {
        Some(phrase(r, Form::All)?)
    } else {
        None
    };
    if !life && sacrifice.is_none() {
        return None;
    }
    let but_mana_abilities = if r.words(r4707_ends(true)) {
        true
    } else {
        r.need(r4707_ends(false))?;
        false
    };
    Some(GameRule::Cant {
        who,
        action: Prohibited::PayCosts {
            life,
            sacrifice,
            but_mana_abilities,
        },
    })
}

/// The wording of the ban, without its period; none for another prohibition.
pub(super) fn r4707_cant_texts(who: &Ref, action: &Prohibited) -> Vec<String> {
    let Prohibited::PayCosts {
        life,
        sacrifice,
        but_mana_abilities,
    } = action
    else {
        return Vec::new();
    };
    let Some(players) = r4707_players(who) else {
        return Vec::new();
    };
    let sacrificed = match sacrifice {
        Some(Selector(preds)) if preds.is_empty() => return Vec::new(),
        Some(filter) => match phrase_text(filter, Form::All) {
            Some(text) => Some(format!("sacrifice {text}")),
            None => return Vec::new(),
        },
        None => None,
    };
    let payments = match (life, sacrificed) {
        (true, Some(sacrificed)) => format!("pay life or {sacrificed}"),
        (true, None) => "pay life".to_owned(),
        (false, Some(sacrificed)) => sacrificed,
        (false, None) => return Vec::new(),
    };
    vec![format!(
        "{players} can't {payments} {}",
        r4707_ends(*but_mana_abilities)
    )]
}
