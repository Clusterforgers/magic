//! Land plays a rule forbids by the card (`Prohibited::PlayLands`), written back: "Players can't play nonbasic lands with
//! the same name as a nontoken permanent." and, beside the same prohibition on casting, "You can't play lands or cast
//! spells from your hand." No template reads them: only the proposal checker writes them.

use super::render::capitalize;
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::modify::{GameRule, Prohibited};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// The players a prohibition names, as the subject of "can't".
fn r408_players(who: &Ref) -> Option<&'static str> {
    match who {
        Ref::You => Some("You"),
        Ref::EachPlayer => Some("Players"),
        Ref::Opponents => Some("Your opponents"),
        _ => None,
    }
}

/// The land cards of a filter, as the object of "play": "lands from your hand", "nonbasic lands with the same name as a
/// nontoken permanent".
fn r408_lands_text(filter: &Selector, whose: &str) -> Option<String> {
    let hand = Pred::In(ZoneKind::Hand);
    let in_hand = filter.0.contains(&hand);
    let nontoken = Pred::SharesName(Ref::Each(Selector(vec![Pred::Not(Box::new(Pred::Token))])));
    let same_name = filter.0.contains(&nontoken);
    let rest: Vec<Pred> = filter
        .0
        .iter()
        .filter(|pred| **pred != hand && **pred != nontoken)
        .cloned()
        .collect();
    let lands = if rest.is_empty() {
        "lands".to_owned()
    } else {
        let mut preds = vec![Pred::Type(mtg_core::card::CardType::Land)];
        preds.extend(
            rest.into_iter()
                .filter(|pred| *pred != Pred::Type(mtg_core::card::CardType::Land)),
        );
        phrase_text(&Selector(preds), Form::All)?
    };
    let lands = if same_name {
        format!("{lands} with the same name as a nontoken permanent")
    } else {
        lands
    };
    Some(if in_hand {
        format!("{lands} from {whose} hand")
    } else {
        lands
    })
}

/// The wordings of a prohibition on playing some land cards, without the period; none for another prohibition.
pub(super) fn r408_cant_texts(who: &Ref, action: &Prohibited) -> Vec<String> {
    let Prohibited::PlayLands(filter) = action else {
        return Vec::new();
    };
    if filter.0.is_empty() {
        return Vec::new();
    }
    let Some(players) = r408_players(who) else {
        return Vec::new();
    };
    let whose = if *who == Ref::You { "your" } else { "their" };
    r408_lands_text(filter, whose)
        .map(|lands| format!("{players} can't play {lands}"))
        .into_iter()
        .collect()
}

/// "You can't play lands or cast spells from your hand.": the prohibition on land plays from a hand and the one on casts
/// from it, for the same players, as one sentence; empty for any other two abilities.
pub(super) fn r408_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Rule {
            rule:
                GameRule::Cant {
                    who,
                    action: Prohibited::PlayLands(lands),
                },
            condition: None,
            from: ZoneKind::Battlefield,
        },
        Ability::Rule {
            rule:
                GameRule::Cant {
                    who: casters,
                    action:
                        Prohibited::Cast {
                            filter,
                            more_than: None,
                            unless: None,
                        },
                },
            condition: None,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    let hand = Selector(vec![Pred::In(ZoneKind::Hand)]);
    if who != casters || *lands != hand || *filter != hand {
        return Vec::new();
    }
    let Some(players) = r408_players(who) else {
        return Vec::new();
    };
    let whose = if *who == Ref::You { "your" } else { "their" };
    vec![capitalize(&format!(
        "{players} can't play lands or cast spells from {whose} hand."
    ))]
}
