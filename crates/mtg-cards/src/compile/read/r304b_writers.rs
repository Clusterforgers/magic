//! Wordings of pack r30-4b: a rule of the attached permanent after its bonus ("Enchanted creature gets +3/+0 and can
//! only attack alone."), three rules of the attached permanent in one sentence, Animate Dead's trigger with the card put
//! onto the battlefield tapped, and the upkeep payment of the enchanted creature's controller that untaps it.

use serde_json::{Value, json};

use super::tokens::same_text;
use crate::ability::Ability;

/// Rules of the attached permanent said after its bonus that no other writer words alone, each with its predicate
/// after the subject; the probe card is named first.
pub(super) const R304B_BACKS: &[(&str, &str)] = &[
    // Errantry
    (
        r#"{"Rule":{"rule":{"Combat":{"quirk":"AttackAlone","who":"Attached"}}}}"#,
        "can only attack alone",
    ),
    // Airtight Alibi
    (
        r#"{"Rule":{"rule":{"Cant":{"action":"BecomeSuspected","who":"Attached"}}}}"#,
        "can't become suspected",
    ),
];

/// The third rule of a sentence about the attached permanent, said after a comma: Faith's Fetters' ban on its
/// activations, mana abilities aside.
const R304B_TAILS: &[(&str, &str)] = &[(
    r#"{"Rule":{"rule":{"Cant":{"action":{"Activate":[]},"who":{"Each":[{"Is":"Attached"}]}}}}}"#,
    "its activated abilities can't be activated unless they're mana abilities",
)];

/// Every wording of three abilities as one sentence about the attached permanent: the first two as their pair writes
/// them, then the third after a comma (Bound in Gold), or the card's own three rules of combat (`r371_owner`); empty for
/// any other three.
pub fn r304b_triple_texts(first: &Ability, second: &Ability, third: &Ability) -> Vec<String> {
    let owner = super::r371_owner::r371_three_texts(first, second, third);
    if !owner.is_empty() {
        return owner;
    }
    let Ok(value) = serde_json::to_value(third) else {
        return Vec::new();
    };
    let Some((_, tail)) = R304B_TAILS
        .iter()
        .find(|(json, _)| serde_json::from_str::<Value>(json).ok().as_ref() == Some(&value))
    else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for pair in super::static_pairs::texts(first, second) {
        let pair = pair.strip_suffix('.').unwrap_or(&pair);
        if !(pair.starts_with("Enchanted ") || pair.starts_with("Equipped ")) {
            continue;
        }
        texts.push(format!("{pair}, and {tail}."));
        // The noun only repeats what the Aura enchants, which its enchant keyword says.
        if let Some(rest) = pair.strip_prefix("Enchanted creature ") {
            texts.push(format!("Enchanted permanent {rest}, and {tail}."));
        }
    }
    texts
}

/// Animate Dead's trigger with the card put onto the battlefield tapped (Dance of the Dead), with the card's name as `~`.
const R304B_TAPPED_TEXT: &str = "When ~ enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with ~.\" Put enchanted creature card onto the battlefield tapped under your control and attach ~ to it. When ~ leaves the battlefield, that creature's controller sacrifices it.";

/// The rules of [`R304B_TAPPED_TEXT`]: Animate Dead's, with the card entering tapped.
fn r304b_tapped_rules() -> Value {
    json!({"Triggered": {
        "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
        "if": {"Matches": {"subject": "This", "selector": [{"In": "Battlefield"}]}},
        "effects": [
            {"Move": {"what": "Attached", "to": {"Battlefield": {"tapped": true, "under": "You"}}, "as": "returned"}},
            {"Attach": {"what": "This", "to": {"Bound": "returned"}}},
            {"DelayedTrigger": {
                "when": {"ZoneChange": {"what": ["This"], "from": "Battlefield"}},
                "effects": [{"Sacrifice": {
                    "who": {"ControllerOf": {"Bound": "returned"}},
                    "filter": [{"Is": {"Bound": "returned"}}]
                }}]
            }}
        ]
    }})
}

/// The ability, when the clause is the words of [`R304B_TAPPED_TEXT`].
pub(super) fn r304b_tapped_read(text: &str) -> Option<Ability> {
    (text.contains("put onto the battlefield with") && same_text(text, R304B_TAPPED_TEXT))
        .then(|| serde_json::from_value(r304b_tapped_rules()).ok())
        .flatten()
}

/// Adds the wordings of the shapes of this pack that no other writer words.
pub(super) fn r304b_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    if value == r304b_tapped_rules() {
        texts.push(R304B_TAPPED_TEXT.to_owned());
    }
    texts.extend(upkeep_untap(&value, written));
    if value
        == json!({"Rule": {"rule": {"Cant": {"who": "EachPlayer", "action": {"SkipStep": "Untap"}}}}})
    {
        texts.push("Each player skips their untap step.".to_owned());
    }
    texts.extend(toggles(&value, written));
    if value == crypt_draw() {
        texts.push("If you would draw a card, return a card from your graveyard to your hand instead. If you can't, you lose the game.".to_owned());
    }
    if value
        == json!({"Replacement": {
            "when": {"ZoneChange": {"what": [{"OwnedBy": "You"}, {"Not": "Token"}], "to": "Graveyard"}},
            "by": {"MoveTo": "Exile"}
        }})
    {
        texts.push(
            "If a card would be put into your graveyard from anywhere, exile that card instead."
                .to_owned(),
        );
        texts.push(
            "If a card would be put into your graveyard from anywhere, exile it instead."
                .to_owned(),
        );
    }
}

/// Forbidden Crypt's draw: a card of the graveyard returns to the hand instead, and with none there the player loses.
fn crypt_draw() -> Value {
    let yard = json!([{"In": "Graveyard"}, {"OwnedBy": "You"}]);
    json!({"Replacement": {"when": {"Draw": {}}, "by": {"Instead": [{"If": {
        "condition": {"Compare": {"left": {"Count": yard}, "op": "Ge", "right": 1}},
        "then": [
            {"Choose": {"from": {"Each": yard}, "as": "card"}},
            {"Move": {"what": {"Bound": "card"}, "to": "Hand"}}
        ],
        "otherwise": [{"LoseGame": {}}]
    }}]}}})
}

/// "that player simultaneously untaps each tapped artifact, creature, and land they control and taps each untapped
/// artifact, creature, and land they control" (Sands of Time): the untapped ones are named first, then the tapped ones
/// untap and the named ones tap, which leaves each permanent as one simultaneous change would.
fn toggles(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Triggered") else {
        return Vec::new();
    };
    if body.get("targets").is_some() || body.get("if").is_some() {
        return Vec::new();
    }
    let kinds = json!({"AnyOf": [{"Type": "Artifact"}, {"Type": "Creature"}, {"Type": "Land"}]});
    let owner = json!({"ControlledBy": "Triggering"});
    let steps = json!([
        {"Bind": {"what": {"Each": [kinds, owner, "Untapped"]}, "as": "untapped"}},
        {"Untap": {"what": {"Each": [kinds, owner, "Tapped"]}}},
        {"Tap": {"what": {"Bound": "untapped"}}}
    ]);
    if body["effects"] != steps {
        return Vec::new();
    }
    heads(body, written)
        .into_iter()
        .map(|head| format!("{head} that player simultaneously untaps each tapped artifact, creature, and land they control and taps each untapped artifact, creature, and land they control."))
        .collect()
}

/// "At the beginning of the upkeep of enchanted creature's controller, that player may pay {1}{B}. If the player does,
/// untap that creature.": the controller of the attached creature pays to untap it.
fn upkeep_untap(value: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Some(body) = value.get("Triggered") else {
        return Vec::new();
    };
    let Some(tail) = upkeep_untap_tail(body) else {
        return Vec::new();
    };
    heads(body, written)
        .into_iter()
        .map(|head| format!("{head} {tail}"))
        .collect()
}

/// The steps of [`upkeep_untap`] in words, when the trigger has them.
fn upkeep_untap_tail(body: &Value) -> Option<String> {
    let controller = json!({"ControllerOf": "Attached"});
    (body["when"]["StepBegins"]["whose"] == controller && body.get("targets").is_none())
        .then_some(())?;
    let [step] = body["effects"].as_array()?.as_slice() else {
        return None;
    };
    let pay = step.get("Pay")?;
    let cost = pay["cost"].as_str()?;
    let fits = pay["who"] == controller
        && pay["paid"] == json!([{"Untap": {"what": "Attached"}}])
        && pay.get("unpaid").is_none()
        && pay.get("times").is_none();
    fits.then_some(())?;
    Some(format!(
        "that player may pay {cost}. If the player does, untap that creature."
    ))
}

/// The heads of a triggered ability as its twin that draws a card is written, without the step.
fn heads(body: &Value, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let mut twin = body.clone();
    let Some(map) = twin.as_object_mut() else {
        return Vec::new();
    };
    map.insert("effects".to_owned(), json!([{"Draw": {}}]));
    let Ok(twin) = serde_json::from_value::<Ability>(json!({ "Triggered": twin })) else {
        return Vec::new();
    };
    written(&twin)
        .into_iter()
        .filter_map(|text| text.strip_suffix(" draw a card.").map(str::to_owned))
        .collect()
}

/// "~ enters with six +1/+1 counters on it. It escapes with twelve +1/+1 counters on it instead.": the counters it
/// enters with when it did not escape, then those it enters with when it did (Polukranos, Unchained).
pub(super) fn r304b_escapes_instead(first: &Ability, second: &Ability) -> Vec<String> {
    use crate::trigger::{ReplaceBy, ReplaceWhen};
    use crate::value::{CastWay, Condition};
    let counters = |ability: &Ability, escaped: bool| -> Option<String> {
        let Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by:
                ReplaceBy::WithCounters {
                    counter,
                    count,
                    tapped: false,
                },
            condition: Some(condition),
            from,
        } = ability
        else {
            return None;
        };
        let wanted = if escaped {
            Condition::CastWith(CastWay::Escape)
        } else {
            Condition::Not(Box::new(Condition::CastWith(CastWay::Escape)))
        };
        (*condition == wanted && from.is_battlefield() && *what == crate::refs::Selector::this())
            .then_some(())?;
        super::counters::entering_text(counter, count)
    };
    let (Some(plain), Some(escaped)) = (counters(first, false), counters(second, true)) else {
        return Vec::new();
    };
    vec![format!(
        "~ enters with {plain} on it. It escapes with {escaped} on it instead."
    )]
}
