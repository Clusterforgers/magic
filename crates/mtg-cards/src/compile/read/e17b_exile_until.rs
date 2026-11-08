//! "Exile cards from the top of your library until you exile a legendary card", written from a reveal that exiles the card
//! found and then the rest under names of their own, what the player may do with the card found, and where the rest go.

use serde_json::{Value, json};

use super::library::filter_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

/// The name the translator gives the card found once exiled, and the name of the other cards exiled.
const FOUND_MOVED: &str = "found_moved";
const REST_MOVED: &str = "rest_moved";

/// The sentences the effects at the front of the list are, and how many effects they take; `None` for another shape.
pub fn sequence(effects: &[Effect]) -> Option<(String, usize)> {
    let [reveal, found, rest, after @ ..] = effects else {
        return None;
    };
    let Effect::RevealUntil {
        who,
        filter,
        count,
        bind,
        revealed_as,
    } = reveal
    else {
        return None;
    };
    count.is_one().then_some(())?;
    exiles(found, bind, FOUND_MOVED)?;
    exiles(rest, revealed_as, REST_MOVED)?;
    let voice = Voice::of(who)?;
    let card = card_text(filter)?;
    let head = format!(
        "{} cards from the top of {} library until {} exile {} {card}",
        voice.exiles,
        voice.their,
        voice.they,
        super::triggers::article(&card)
    );
    let (middle, used) = middle(after, filter)?;
    let tail = after.get(used)?;
    let tails = tails(tail, &voice, matches!(middle, Middle::Put))?;
    let text = match middle {
        Middle::Put => format!("{head}. {tails}"),
        Middle::Sentence(said) => format!("{head}. {said}. {tails}"),
    };
    Some((text, 3 + used + 1))
}

/// "Choose up to three Doctors. You may exile all other creatures. If you do, ~ deals 13 damage to you.": up to a number
/// of permanents of one subtype chosen, then the choice between exiling every other creature with the damage and nothing.
pub fn chosen_spared(effects: &[Effect]) -> Option<(String, usize)> {
    let [choose, either, ..] = effects else {
        return None;
    };
    let choose = serde_json::to_value(choose).ok()?;
    let choose = choose.get("Choose")?;
    let name = choose["as"].as_str()?;
    let [kind] = choose["from"].get("Each")?.as_array()?.as_slice() else {
        return None;
    };
    let kind = kind.get("Subtype")?.as_str()?;
    let most = choose["count"].get("max")?.as_u64()?;
    (choose.as_object()?.len() == 3 && choose["count"]["min"] == json!(0)).then_some(())?;
    let either = serde_json::to_value(either).ok()?;
    let either = either.get("Either")?;
    let exile = json!({"Exile": {"what": {"Each": [{"Type": "Creature"}, {"Not": {"Is": {"Bound": name}}}]}}});
    let [exiled, damage] = either["first"].as_array()?.as_slice() else {
        return None;
    };
    let amount = damage["DealDamage"]["amount"].as_u64()?;
    (either.as_object()?.len() == 2
        && either["second"] == json!([])
        && *exiled == exile
        && *damage == json!({"DealDamage": {"amount": amount, "to": "You"}}))
    .then_some(())?;
    let most = super::amounts::count_word(u32::try_from(most).ok()?)?;
    Some((
        format!(
            "choose up to {most} {kind}s. You may exile all other creatures. If you do, ~ deals {amount} damage to you"
        ),
        2,
    ))
}

/// Who exiles, in the words of the sentence.
struct Voice {
    exiles: &'static str,
    their: &'static str,
    they: &'static str,
    puts: &'static str,
}

impl Voice {
    fn of(who: &Ref) -> Option<Voice> {
        match who {
            Ref::You => Some(Voice {
                exiles: "exile",
                their: "your",
                they: "you",
                puts: "put",
            }),
            Ref::Triggering => Some(Voice {
                exiles: "that player exiles",
                their: "their",
                they: "they",
                puts: "that player puts",
            }),
            _ => None,
        }
    }
}

/// The step exiles the cards a name holds and names them as they are in exile.
fn exiles(effect: &Effect, held: &str, stem: &str) -> Option<()> {
    let Effect::Exile {
        what: Ref::Bound(what),
        bind: Some(name),
        duration: None,
    } = effect
    else {
        return None;
    };
    (what == held && plain(name) == stem).then_some(())
}

/// A bookkeeping name without its number (`found_moved3` is `found_moved`).
fn plain(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// The card that ends the exiling: "legendary card", "land card", "legendary nonland card with lesser mana value" (lesser
/// than the spell that triggered the ability). The kinds of permanent a card of a permanent type is are left unsaid.
fn card_text(filter: &Selector) -> Option<String> {
    let lesser = json!({"Compare": {"op": "Lt", "stat": "ManaValue", "to": {"Stat": {"of": "Triggering", "stat": "ManaValue"}}}});
    let kinds: Vec<Value> = [
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
    ]
    .iter()
    .map(|kind| json!({ "Type": kind }))
    .collect();
    let permanent = json!({ "AnyOf": kinds });
    let mut rest = Vec::new();
    let mut with_lesser = false;
    for pred in &filter.0 {
        let value = serde_json::to_value(pred).ok()?;
        if value == lesser {
            with_lesser = true;
        } else if value != permanent
            || !filter
                .0
                .contains(&Pred::Type(mtg_core::card::CardType::Land))
        {
            rest.push(pred.clone());
        }
    }
    // "legendary nonland card": the supertype comes first.
    rest.sort_by_key(|pred| !matches!(pred, Pred::Supertype(_)));
    let card = filter_text(&Selector(rest), false)?;
    Some(if with_lesser {
        format!("{card} with lesser mana value")
    } else {
        card
    })
}

/// What the player does with the card found.
enum Middle {
    /// It goes onto the battlefield, said with the rest: "Put that card onto the battlefield and the rest ...".
    Put,
    /// A sentence of its own.
    Sentence(&'static str),
}

/// The step or two after the exiles, about the card found, and how many steps they take.
fn middle(after: &[Effect], filter: &Selector) -> Option<(Middle, usize)> {
    let step = serde_json::to_value(after.first()?).ok()?;
    let found = |value: &Value| {
        value
            .get("Bound")
            .and_then(Value::as_str)
            .is_some_and(|name| plain(name) == FOUND_MOVED)
    };
    if let Some(cast) = step.get("CastFromZone") {
        let keys: Vec<&str> = cast.as_object()?.keys().map(String::as_str).collect();
        return (keys == ["free", "what"] && cast["free"] == json!(true) && found(&cast["what"]))
            .then_some((
                Middle::Sentence("You may cast that card without paying its mana cost"),
                1,
            ));
    }
    if let Some(moved) = step.get("Move") {
        return (moved.as_object()?.len() == 2
            && moved["to"] == json!({"Battlefield": {}})
            && found(&moved["what"]))
        .then_some((Middle::Put, 1));
    }
    if let Some(rule) = step.get("Rule") {
        let may = rule["rule"].get("MayCast")?;
        found(&may["only"]).then_some(())?;
        let mut rest = may.clone();
        rest.as_object_mut()?.remove("only");
        return match (rule["duration"].as_str()?, rest) {
            ("UntilEndOfTurn", rest)
                if rest == json!({"free": true, "from": "Exile"})
                    || rest
                        == json!({"free": true, "from": "Exile", "what": [{"Not": {"Type": "Land"}}]}) =>
            {
                Some((
                    Middle::Sentence(
                        "Until end of turn, you may cast that card without paying its mana cost",
                    ),
                    1,
                ))
            }
            ("WhileSourceOnBattlefield", rest)
                if rest == json!({"from": "Exile", "play": true}) =>
            {
                Some((
                    Middle::Sentence(
                        "You may play that card for as long as ~ remains on the battlefield",
                    ),
                    1,
                ))
            }
            _ => None,
        };
    }
    // "You may cast that card": a choice of at most one of the card found, among the cards the exiling looked for.
    let choose = step.get("Choose")?;
    let picked = choose["as"].as_str()?;
    let cast = serde_json::to_value(after.get(1)?).ok()?;
    let mut from = serde_json::to_value(filter).ok()?.as_array()?.clone();
    from.push(json!({"Is": choose["from"]["Each"].get(from.len())?.get("Is")?}));
    from.push(json!({"In": "Exile"}));
    (choose.as_object()?.len() == 3
        && choose["count"] == json!({"min": 0, "max": 1})
        && choose["from"] == json!({ "Each": from })
        && found(&from[from.len() - 2]["Is"])
        && cast == json!({"CastFromZone": {"free": true, "what": {"Bound": picked}}}))
    .then_some((
        Middle::Sentence("You may cast that card without paying its mana cost"),
        2,
    ))
}

/// Where the other cards go: the bottom of the library in a random order, in the words the cards use for them.
fn tails(step: &Effect, voice: &Voice, put: bool) -> Option<String> {
    let step = serde_json::to_value(step).ok()?;
    let moved = step.get("Move")?;
    (moved.as_object()?.len() == 2
        && moved["to"] == json!({"Library": {"place": "Bottom", "order": "Random"}}))
    .then_some(())?;
    let name = |value: &Value, stem: &str| {
        value
            .get("Is")
            .and_then(|is| is.get("Bound"))
            .and_then(Value::as_str)
            .is_some_and(|name| plain(name) == stem)
    };
    let preds = moved["what"].get("Each")?.as_array()?;
    let place = format!("on the bottom of {} library in a random order", voice.their);
    match preds.as_slice() {
        [rest, exile] if name(rest, REST_MOVED) && *exile == json!({"In": "Exile"}) => {
            if put {
                return (voice.puts == "put")
                    .then(|| format!("Put that card onto the battlefield and the rest {place}"));
            }
            Some(format!("{} the rest {place}", capital(voice.puts)))
        }
        [rest, not_found, exile]
            if !put
                && name(rest, REST_MOVED)
                && not_found.get("Not").is_some_and(|is| name(is, FOUND_MOVED))
                && *exile == json!({"In": "Exile"}) =>
        {
            Some(format!("{} the rest {place}", capital(voice.puts)))
        }
        [both, exile] if !put && *exile == json!({"In": "Exile"}) => {
            let [one, other] = both.get("AnyOf")?.as_array()?.as_slice() else {
                return None;
            };
            (name(one, REST_MOVED) && name(other, FOUND_MOVED)).then(|| {
                format!(
                    "Then {} the exiled cards that weren't cast this way {place}",
                    voice.puts
                )
            })
        }
        _ => None,
    }
}

fn capital(words: &str) -> String {
    let mut chars = words.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The other wordings of "the rest" in a sentence this module writes.
pub fn other_wordings(texts: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for text in texts
        .iter()
        .filter(|text| text.contains(" cards from the top of "))
    {
        for other in [
            "each other card exiled this way",
            "the rest of those exiled cards",
        ] {
            if let Some(at) = text.find("Put the rest on the bottom") {
                let mut said = text.clone();
                said.replace_range(at + 4..at + 12, other);
                found.push(said);
            }
        }
    }
    found
}

/// Adds "When you next cast a spell this turn" to the wordings of an ability that sets up a delayed trigger on your next
/// cast this turn.
pub fn add(ability: &Ability, texts: &mut Vec<String>) {
    let next_cast = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::DelayedTrigger {
                when: TriggerPattern::SpellCast { by: Ref::You, .. },
                once: true,
                this_turn: true,
                until: None,
                ..
            }
        )
    });
    if !next_cast {
        return;
    }
    let mut next = Vec::new();
    for text in texts.iter() {
        for opening in ["When you cast ", "when you cast "] {
            if let Some(at) = text.find(opening)
                && let Some(turn) = text[at..].find(" this turn, ")
                && !text[at..at + turn].contains(['.', ','])
            {
                let mut said = text.clone();
                said.insert_str(at + opening.len() - "cast ".len(), "next ");
                next.push(said);
            }
        }
    }
    texts.extend(next);
}
