//! Moves of the fifth sample of M to R: "put it on the bottom of its owner's library", "each player may discard their
//! hand and draw seven cards", "Exile ~. Return it to the battlefield tapped under its owner's control at the beginning
//! of the next end step.", a move "into a graveyard ... or ... into exile from the battlefield", the exile from the
//! library of the player of an event and "Return two lands you control to their owner's hand" as a cost.

use super::{c5_mr_number_after, c5_mr_swapped, c5_mr_twin};
use crate::ability::Ability;
use crate::compile::read::amounts::count_word;
use crate::trigger::TriggerPattern;

/// A move of the source to the bottom of its owner's library, and the return to the hand the renderers write.
const C5_MR_BOTTOM: &str = r#"{"Move":{"what":"This","to":{"Library":{"place":"Bottom"}}}}"#;
const C5_MR_HAND: &str = r#"{"Move":{"what":"This","to":"Hand"}}"#;

/// The head of a wheel that each player may do, up to the number of cards drawn.
const C5_MR_WHEEL: &str = r#"{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Discard":{"who":{"Bound":"player"},"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]}}},{"Draw":{"who":{"Bound":"player"},"count":"#;

/// The exile of the source and its return at the next end step, tapped or not.
const C5_MR_BLINK: &str = r#"{"Exile":{"what":"This","as":"exiled"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":"#;
const C5_MR_EXILE: &str = r#"{"Exile":{"what":"This"}}"#;

/// The exile from the library of the player of the event with a free cast of a nonland card, and the exile from your
/// library the renderers write.
const C5_MR_THEIR_EXILE: &str = r#"{"ExileTop":{"who":{"ControllerOf":"Triggering"},"as":"card"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"card"},"selector":[{"Not":{"Type":"Land"}}]}},"then":[{"CastFromZone":{"what":{"Bound":"card"},"free":true}}]}}"#;
const C5_MR_OWN_EXILE: &str = r#"{"ExileTop":{"as":"card"}}"#;

/// The head of lands returned to their owner's hand as a cost.
const C5_MR_LANDS_BACK: &str = r#"{"ReturnToHand":{"filter":[{"Type":"Land"}],"count":"#;

/// Adds the wordings of the moves.
pub(super) fn add(ability: &Ability, json: &str, texts: &mut Vec<String>) {
    let twin = c5_mr_twin(json, C5_MR_BOTTOM, C5_MR_HAND);
    texts.extend(c5_mr_swapped(
        &twin,
        "return it to its owner's hand",
        "put it on the bottom of its owner's library",
    ));
    c5_mr_wheel(json, texts);
    c5_mr_blink(json, texts);
    let twin = c5_mr_twin(json, C5_MR_THEIR_EXILE, C5_MR_OWN_EXILE);
    texts.extend(c5_mr_swapped(
        &twin,
        "exile the top card of your library.",
        "that player exiles the top card of their library. If it's a nonland card, you may cast it without paying its \
         mana cost.",
    ));
    c5_mr_graveyard_or_exile(ability, texts);
    c5_mr_lands_back(json, texts);
}

/// "each player may discard their hand and draw seven cards", from the twin where each player draws.
fn c5_mr_wheel(json: &str, texts: &mut Vec<String>) {
    let Some((count, whole)) = c5_mr_number_after(json, C5_MR_WHEEL, "}}]}}]}}") else {
        return;
    };
    let draw = format!(r#"{{"Draw":{{"who":"EachPlayer","count":{count}}}}}"#);
    let twin = c5_mr_twin(json, whole, &draw);
    texts.extend(c5_mr_swapped(
        &twin,
        "each player draws ",
        "each player may discard their hand and draw ",
    ));
}

/// "Exile ~. Return it to the battlefield [tapped] under its owner's control at the beginning of the next end step."
fn c5_mr_blink(json: &str, texts: &mut Vec<String>) {
    for (place, tapped) in [(r#"{"tapped":true}"#, "tapped "), ("{}", "")] {
        let whole = format!("{C5_MR_BLINK}{place}}}}}}}]}}}}");
        let found: Vec<String> = c5_mr_twin(json, &whole, C5_MR_EXILE)
            .iter()
            .filter_map(|text| text.strip_suffix("Exile ~."))
            .map(|head| {
                format!(
                    "{head}Exile ~. Return it to the battlefield {tapped}under its owner's control at the beginning \
                     of the next end step."
                )
            })
            .collect();
        texts.extend(found);
    }
}

/// "is put into a graveyard from the battlefield or is put into exile from the battlefield" for the events of an
/// `AnyOf` whose second half is its first half moving to exile.
fn c5_mr_graveyard_or_exile(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        ..
    } = ability
    else {
        return;
    };
    if events.len() < 2 || events.len() % 2 != 0 {
        return;
    }
    let (graveyard, exile) = events.split_at(events.len() / 2);
    let as_exile = |event: &TriggerPattern| {
        serde_json::to_string(event)
            .unwrap_or_default()
            .replace(r#""to":"Graveyard""#, r#""to":"Exile""#)
    };
    let mirrored = graveyard.iter().zip(exile).all(|(left, right)| {
        let left_json = serde_json::to_string(left).unwrap_or_default();
        left_json.contains(r#""from":"Battlefield","to":"Graveyard""#)
            && as_exile(left) == serde_json::to_string(right).unwrap_or_default()
    });
    if !mirrored {
        return;
    }
    let mut twin = ability.clone();
    if let Ability::Triggered { when, .. } = &mut twin {
        *when = TriggerPattern::AnyOf(graveyard.to_vec());
    }
    let written = super::super::super::variants(&twin);
    texts.extend(c5_mr_swapped(
        &written,
        " dies, ",
        " is put into a graveyard from the battlefield or is put into exile from the battlefield, ",
    ));
}

/// "Return two lands you control to their owner's hand" as a cost, from the twin that discards as many cards.
fn c5_mr_lands_back(json: &str, texts: &mut Vec<String>) {
    let Some((count, whole)) = c5_mr_number_after(json, C5_MR_LANDS_BACK, "}}") else {
        return;
    };
    let Some(word) = count_word(count) else {
        return;
    };
    let discard = format!(r#"{{"Discard":{{"count":{count}}}}}"#);
    let twin = c5_mr_twin(json, whole, &discard);
    texts.extend(c5_mr_swapped(
        &twin,
        &format!("Discard {word} cards"),
        &format!("Return {word} lands you control to their owner's hand"),
    ));
}
