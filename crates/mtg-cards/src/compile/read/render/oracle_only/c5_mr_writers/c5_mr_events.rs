//! Players of the fifth sample of M to R: "each player sacrifices ... of their choice. Each player who can't discards a
//! card.", "At the beginning of the draw step of enchanted creature's controller, that player draws an additional
//! card." and "~ deals 1 damage to that player" for the controller of the permanent of an event.

use super::{c5_mr_number_after, c5_mr_swapped, c5_mr_twin};

/// The sacrifice of each player with the discard of who can't, around the filter of the sacrifice.
const C5_MR_SACRIFICE_HEAD: &str = r#"{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Sacrifice":{"who":{"Bound":"player"},"filter":"#;
const C5_MR_SACRIFICE_TAIL: &str = r#"}},{"If":{"condition":{"Not":"Performed"},"then":[{"Discard":{"who":{"Bound":"player"}}}]}}]}}"#;

/// The draw step of the controller of the enchanted creature, and your draw step the renderers write.
const C5_MR_THEIR_DRAW: &str = r#"{"StepBegins":{"step":"Draw","whose":{"ControllerOf":"Attached"}}},"effects":[{"Draw":{"who":{"ControllerOf":"Attached"}}}]"#;
const C5_MR_YOUR_DRAW: &str =
    r#"{"StepBegins":{"step":"Draw","whose":"You"}},"effects":[{"Draw":{}}]"#;

/// Damage to the controller of the permanent of the event, which the twin has that player lose as life.
const C5_MR_DAMAGE_HEAD: &str = r#"{"DealDamage":{"to":{"ControllerOf":"Triggering"},"amount":"#;

/// The names the twin gives the controller of the permanent of the event.
const C5_MR_CONTROLLERS: [&str; 3] = [
    "that player",
    "that creature's controller",
    "that permanent's controller",
];

/// Adds the wordings of the players.
pub(super) fn add(json: &str, texts: &mut Vec<String>) {
    c5_mr_who_cant(json, texts);
    let twin = c5_mr_twin(json, C5_MR_THEIR_DRAW, C5_MR_YOUR_DRAW);
    texts.extend(c5_mr_swapped(
        &twin,
        "At the beginning of your draw step, draw an additional card.",
        "At the beginning of the draw step of enchanted creature's controller, that player draws an additional card.",
    ));
    if let Some((amount, whole)) = c5_mr_number_after(json, C5_MR_DAMAGE_HEAD, "}}") {
        let lose = format!(
            r#"{{"LoseLife":{{"who":{{"ControllerOf":"Triggering"}},"amount":{amount}}}}}"#
        );
        let twin = c5_mr_twin(json, whole, &lose);
        for player in C5_MR_CONTROLLERS {
            texts.extend(c5_mr_swapped(
                &twin,
                &format!("{player} loses {amount} life"),
                &format!("~ deals {amount} damage to {player}"),
            ));
        }
        // The twin calls the controller of a creature "that permanent's controller".
        if json.contains(r#"{"Type":"Creature"}"#) {
            texts.extend(c5_mr_swapped(
                &twin,
                &format!("that permanent's controller loses {amount} life"),
                &format!("~ deals {amount} damage to that creature's controller"),
            ));
        }
    }
}

/// "each player sacrifices a creature or planeswalker of their choice. Each player who can't discards a card." from the
/// twin where each player sacrifices.
fn c5_mr_who_cant(json: &str, texts: &mut Vec<String>) {
    let Some(at) = json.find(C5_MR_SACRIFICE_HEAD) else {
        return;
    };
    let Some(rest) = json.get(at + C5_MR_SACRIFICE_HEAD.len()..) else {
        return;
    };
    let Some((filter, _)) = rest.split_once(C5_MR_SACRIFICE_TAIL) else {
        return;
    };
    let whole = format!("{C5_MR_SACRIFICE_HEAD}{filter}{C5_MR_SACRIFICE_TAIL}");
    let each = format!(r#"{{"Sacrifice":{{"who":"EachPlayer","filter":{filter}}}}}"#);
    let found: Vec<String> = c5_mr_twin(json, &whole, &each)
        .iter()
        .filter(|text| text.contains("each player sacrifices "))
        .filter_map(|text| text.strip_suffix('.'))
        .map(|head| format!("{head} of their choice. Each player who can't discards a card."))
        .collect();
    texts.extend(found);
}
