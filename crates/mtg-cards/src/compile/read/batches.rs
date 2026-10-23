//! Events that happen together, worded for a group: "Whenever one or more artifacts you control enter", "... creatures you
//! control die", "... cards leave your graveyard", "... land cards are put into your graveyard from anywhere", and
//! "Whenever you attack with one or more Birds" for the group of attackers. Written from the data only.

use super::amounts::count_word;
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// "Whenever one or more other Elves you control enter": a batch of moves between zones.
pub(super) fn zone_change(
    what: &Selector,
    from: Option<ZoneKind>,
    to: Option<ZoneKind>,
) -> Option<String> {
    let verb = match (from, to) {
        (None, Some(ZoneKind::Battlefield)) => "enter",
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => "die",
        (Some(ZoneKind::Battlefield), None) => "leave the battlefield",
        (Some(ZoneKind::Graveyard), None) => {
            return Some(format!(
                "Whenever one or more {} leave your graveyard",
                your_cards(what)?
            ));
        }
        (None, Some(ZoneKind::Graveyard)) => {
            return Some(format!(
                "Whenever one or more {} are put into your graveyard from anywhere",
                your_cards(what)?
            ));
        }
        (Some(ZoneKind::Library), Some(ZoneKind::Graveyard)) => {
            return Some(format!(
                "Whenever one or more {} are put into your graveyard from your library",
                your_cards(what)?
            ));
        }
        _ => return None,
    };
    if what.is_empty() {
        return None;
    }
    Some(format!(
        "Whenever one or more {} {verb}",
        phrase_text(what, Form::All)?
    ))
}

/// The cards of a group owned by you, in the plural: "cards", "creature cards", "nonland cards".
fn your_cards(what: &Selector) -> Option<String> {
    if !what.0.contains(&Pred::OwnedBy(Ref::You)) {
        return None;
    }
    let mut preds: Vec<Pred> = what
        .0
        .iter()
        .filter(|pred| **pred != Pred::OwnedBy(Ref::You))
        .cloned()
        .collect();
    preds.extend([Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]);
    let cards = phrase_text(&Selector(preds), Form::All)?;
    cards
        .strip_suffix(" from your graveyard")
        .map(str::to_owned)
}

/// "Whenever you attack with one or more Birds, ..." and "Whenever you attack, ..." for the attackers that are
/// creatures you control, in each wording the head of the ability has.
pub(super) fn attack_wordings(ability: &Ability, texts: &[String]) -> Vec<String> {
    let Ability::Triggered {
        when: TriggerPattern::GroupAttacks { what, at_least },
        ..
    } = ability
    else {
        return Vec::new();
    };
    let (Some(count), Some(group)) = (count_word(*at_least), phrase_text(what, Form::All)) else {
        return Vec::new();
    };
    let plain = group.replacen(" you control", "", 1);
    let from = format!("Whenever {count} or more {group} attack,");
    let bare_creatures = *at_least == 1 && group == "creatures you control";
    let mut found = Vec::new();
    for rest in texts.iter().filter_map(|text| text.strip_prefix(&from)) {
        if plain != group {
            found.push(format!(
                "Whenever you attack with {count} or more {plain},{rest}"
            ));
        }
        if bare_creatures {
            found.push(format!("Whenever you attack,{rest}"));
        }
    }
    // The attackers of the event are "each of them" and "those creatures" in what the ability does to them.
    let mut attacking = what.clone();
    attacking.0.push(Pred::Attacking);
    let (each, all) = (
        phrase_text(&attacking, Form::Each),
        phrase_text(&attacking, Form::All),
    );
    let mut named: Vec<String> = Vec::new();
    for text in texts.iter().chain(&found) {
        let each_of = each
            .as_ref()
            .map(|one| text.replacen(&format!("each {one}"), "each of them", 1));
        let those = all
            .as_ref()
            .map(|many| text.replacen(many.as_str(), "those creatures", 1));
        named.extend(
            [each_of, those]
                .into_iter()
                .flatten()
                .filter(|other| other != text),
        );
    }
    found.extend(named.into_iter().filter(|text| !texts.contains(text)));
    found
}
