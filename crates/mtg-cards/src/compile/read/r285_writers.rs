//! Wordings no other writer gives: casting conditions on counts, attacks and opponents' spells, alternative costs while a
//! kind of creature attacks, triggered abilities that trigger again for a kind of source, "bands with other legendary
//! creatures" given to legendary creatures of a color, an enchanted Wall attacking despite defender, an untap held back
//! without an enduring story, and the singular wording of permanents that don't untap.

use serde_json::{Value, json};

use super::amounts::count_word;
use super::render::capitalize;
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, StatKind};

/// Adds the wordings of this module for the ability to `texts`, each once.
pub(super) fn r285_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let found = [
        macro_x(ability),
        cast_only(&value),
        attack_cost(&value),
        trigger_again(&value),
        bands_with_legends(&value),
        despite_defender(&value),
        untaps_with_story(&value),
        each_doesnt_untap(&value),
        damage_destroys(&value),
        prohibitions(&value),
        cast_in_combat(&value),
        targeting_you_costs(&value),
        unless_exiled(&value),
        hexproof_unless_fighting(&value),
        enter_tapped(&value),
        prevent_one(&value),
        graveyard_keywords(&value),
        monarch_controls(&value),
        trap_costs(&value),
        sundry(&value),
    ];
    for text in found.into_iter().flatten() {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    let main_phases: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            text.strip_prefix("At the beginning of your precombat or postcombat main phase")
                .map(|rest| format!("At the beginning of each of your main phases{rest}"))
        })
        .collect();
    texts.extend(main_phases);
    let fights: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(", you may it fights "))
        .map(|text| text.replace(", you may it fights ", ", you may have it fight "))
        .collect();
    texts.extend(fights);
    let creature_types: Vec<String> = texts
        .iter()
        .filter_map(|text| other_creature_types(text))
        .collect();
    for text in creature_types {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "Firebending X, where X is the number of creatures you control.", "~ has bushido X, where X is the number of attacking
/// creatures.": a macro keyword with X.
fn macro_x(ability: &Ability) -> Vec<String> {
    let Ability::Keyword(Keyword::MacroX { name, x }) = ability else {
        return Vec::new();
    };
    let mut said = super::defining::value_texts(x);
    if *x
        == (Amount::Stat {
            of: Ref::This,
            stat: StatKind::Power,
        })
    {
        said.extend(["~'s power".to_owned(), "this creature's power".to_owned()]);
    }
    if let Amount::Counters {
        on: Ref::You,
        counter: crate::value::CounterKind::Other(kind),
    } = x
    {
        said.push(format!("the number of {kind} counters you have"));
    }
    said.iter()
        .flat_map(|said| {
            [
                format!("{} X, where X is {said}", capitalize(name)),
                format!("~ has {name} X, where X is {said}"),
            ]
        })
        .collect()
}

/// The two statics of one sentence about the enchanted creature: "can't attack, block, or transform", "can't become
/// untapped and can't have counters put on it", "loses all abilities and can't become untapped".
pub(super) fn r285_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (Ok(first), Ok(second)) = (serde_json::to_value(first), serde_json::to_value(second))
    else {
        return Vec::new();
    };
    let restricted =
        json!({ "Static": { "affected": "Attached", "mods": ["CantAttack", "CantBlock"] } });
    let transform =
        json!({ "Rule": { "rule": { "Cant": { "who": "Attached", "action": "Transform" } } } });
    let cant = |action: &str| json!({ "Rule": { "rule": { "Cant": { "who": "Attached", "action": action } } } });
    let loses = json!({ "Static": { "affected": "Attached", "mods": ["LoseAbilities"] } });
    let velocipede = |op: &str, count: u32| {
        json!({ "Replacement": {
            "when": { "Enters": { "what": [
                { "AnyOf": [{ "Subtype": "Vehicle" }, { "Type": "Creature" }] }, "Other", { "ControlledBy": "You" },
                { "Compare": { "stat": "ManaValue", "op": op, "to": 4 } }
            ] } },
            "by": { "WithCounters": { "counter": "+1/+1", "count": count } }
        } })
    };
    if first == velocipede("Le", 1) && second == velocipede("Gt", 3) {
        return vec!["Each other Vehicle and creature you control enters with an additional +1/+1 counter on it if its mana value is 4 or less. Otherwise, it enters with three additional +1/+1 counters on it".to_owned()];
    }
    let theirs = json!({ "Each": [{ "Type": "Creature" }, { "ControlledBy": "You" }, { "Not": { "OwnedBy": "You" } }] });
    if first
        == json!({ "Static": { "affected": theirs, "mods": [{ "ModifyPT": { "power": 2, "toughness": 2 } }] } })
        && second
            == json!({ "Rule": { "rule": { "Cant": { "who": theirs, "action": "Sacrifice" } } } })
    {
        return vec![
            "Creatures you control but don't own get +2/+2 and can't be sacrificed".to_owned(),
        ];
    }
    let eminence = eminence_texts(&first, &second);
    if !eminence.is_empty() {
        return eminence;
    }
    let text = if first == restricted && second == transform {
        "Enchanted creature can't attack, block, or transform"
    } else if first == cant("Untap") && second == cant("PutCounter") {
        "Enchanted creature can't become untapped and can't have counters put on it"
    } else if first == loses && second == cant("Untap") {
        "Enchanted creature loses all abilities and can't become untapped"
    } else if first == json!({ "Rule": { "rule": { "MustAttack": "Attached" } } })
        && second
            == json!({ "Static": { "affected": "Attached", "mods": [{ "CantAttackYou": "Player" }] } })
    {
        "Enchanted creature attacks each combat if able and can't attack you"
    } else {
        return Vec::new();
    };
    vec![text.to_owned()]
}

/// "As long as ~ is in the command zone or on the battlefield, other Dragon spells you cast cost {1} less to cast.": the
/// same rule from the battlefield and from the command zone.
fn eminence_texts(first: &Value, second: &Value) -> Vec<String> {
    let mut command = second.clone();
    let Some(from) = command.get_mut("Rule").and_then(Value::as_object_mut) else {
        return Vec::new();
    };
    if from.remove("from") != Some(json!("Command")) || &command != first {
        return Vec::new();
    }
    let Some(cost) = first.pointer("/Rule/rule/SpellCost") else {
        return Vec::new();
    };
    if first.pointer("/Rule/condition").is_some()
        || cost.get("change") != Some(&json!("Reduce"))
        || cost.as_object().map(serde_json::Map::len) != Some(3)
    {
        return Vec::new();
    }
    let (Some(mana), Some(mut spells)) = (
        cost.get("mana").and_then(Value::as_str),
        cost.get("applies_to").and_then(selector),
    ) else {
        return Vec::new();
    };
    let wanted = [Pred::Spell, Pred::Other, Pred::ControlledBy(Ref::You)];
    if !wanted.iter().all(|pred| spells.0.contains(pred)) {
        return Vec::new();
    }
    spells.0.retain(|pred| !wanted.contains(pred));
    let Some(kind) = phrase_text(&spells, Form::Subject) else {
        return Vec::new();
    };
    vec![format!(
        "As long as ~ is in the command zone or on the battlefield, other {kind} spells you cast cost {mana} less to cast"
    )]
}

/// A selector of the JSON form.
fn selector(value: &Value) -> Option<Selector> {
    serde_json::from_value(value.clone()).ok()
}

/// "a" or "an" before the word.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

/// "a snow land", "two or more Vampires": a count of permanents you control, at least `many`.
fn you_control(preds: &[Pred], many: u64) -> Option<String> {
    let (last, kind) = preds.split_last()?;
    (last == &Pred::ControlledBy(crate::refs::Ref::You)).then_some(())?;
    let kind = Selector(kind.to_vec());
    if many == 1 {
        let one = phrase_text(&kind, Form::Subject)?;
        return Some(format!("you control {} {one}", article(&one)));
    }
    let words = count_word(u32::try_from(many).ok()?)?;
    let all = phrase_text(&kind, Form::All)?;
    Some(format!("you control {words} or more {all}"))
}

/// "a white creature is attacking", "a black creature with flying is attacking".
fn attacking(preds: &[Pred]) -> Option<String> {
    let at = preds.iter().position(|pred| pred == &Pred::Attacking)?;
    let mut kind = preds.to_vec();
    kind.remove(at);
    let one = phrase_text(&Selector(kind), Form::Subject)?;
    Some(format!("{} {one} is attacking", article(&one)))
}

/// The count a condition holds at least, with what it counts: `Compare { Count, Ge, n }`.
fn counted(condition: &Value) -> Option<(Selector, u64)> {
    let compare = condition.get("Compare")?;
    (compare.get("op")? == "Ge").then_some(())?;
    let many = compare.get("right")?.as_u64()?;
    Some((selector(compare.get("left")?.get("Count")?)?, many))
}

/// "Cast ~ only if you control a snow land.", "Cast ~ only during the declare attackers step and only if you've been
/// attacked this step."
fn cast_only(value: &Value) -> Vec<String> {
    let Some(condition) = value.get("CastOnly").and_then(|body| body.get("condition")) else {
        return Vec::new();
    };
    let attacked =
        json!({ "All": [{ "Step": "DeclareAttackers" }, { "AttackedThisCombat": "You" }] });
    if condition == &attacked {
        return vec![
            "Cast ~ only during the declare attackers step and only if you've been attacked this step"
                .to_owned(),
        ];
    }
    let Some((kind, many)) = counted(condition) else {
        return Vec::new();
    };
    you_control(&kind.0, many)
        .map(|held| format!("Cast ~ only if {held}"))
        .into_iter()
        .collect()
}

/// "If a white creature is attacking, you may pay {B}{B} rather than pay ~'s mana cost."
fn attack_cost(value: &Value) -> Vec<String> {
    let Some(body) = value.get("FreeCast") else {
        return Vec::new();
    };
    let (Some(condition), Some(cost)) = (
        body.get("condition"),
        body.get("cost").and_then(Value::as_str),
    ) else {
        return Vec::new();
    };
    if !cost.starts_with('{') {
        return Vec::new();
    }
    let Some((kind, 1)) = counted(condition) else {
        return Vec::new();
    };
    attacking(&kind.0)
        .map(|held| format!("If {held}, you may pay {cost} rather than pay ~'s mana cost"))
        .into_iter()
        .collect()
}

/// "If a triggered ability of a Doctor you control triggers, that ability triggers an additional time.", with "while
/// you control six or more Shrines" for a rule under a count of permanents.
fn trigger_again(value: &Value) -> Vec<String> {
    let Some(rule) = value.get("Rule") else {
        return Vec::new();
    };
    let Some(again) = rule.get("rule").and_then(|rule| rule.get("TriggerAgain")) else {
        return Vec::new();
    };
    if again.get("cause").is_some() {
        return Vec::new();
    }
    let whose = match again.get("source") {
        None => "a permanent you control".to_owned(),
        Some(source) => {
            let Some(mut source) = selector(source) else {
                return Vec::new();
            };
            let other = source.0.contains(&Pred::Other);
            source.0.retain(|pred| pred != &Pred::Other);
            let Some(phrase) = phrase_text(&source, Form::Subject) else {
                return Vec::new();
            };
            if other {
                format!("another {phrase}")
            } else {
                format!("{} {phrase}", article(&phrase))
            }
        }
    };
    let held = match rule.get("condition") {
        None => String::new(),
        Some(condition) => {
            let Some(held) = counted(condition).and_then(|(kind, many)| you_control(&kind.0, many))
            else {
                return Vec::new();
            };
            format!(" while {held}")
        }
    };
    vec![format!(
        "If a triggered ability of {whose} triggers{held}, that ability triggers an additional time"
    )]
}

/// "Black legendary creatures you control have \"bands with other legendary creatures.\""
fn bands_with_legends(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Static") else {
        return Vec::new();
    };
    let legends = json!([{ "Supertype": "Legendary" }, { "Type": "Creature" }]);
    if body.get("mods") != Some(&json!([{ "AddKeyword": { "BandsWithOther": legends } }]))
        || body.as_object().map(serde_json::Map::len) != Some(2)
    {
        return Vec::new();
    }
    let Some(affected) = body
        .get("affected")
        .and_then(|affected| affected.get("Each"))
    else {
        return Vec::new();
    };
    let Some(affected) = selector(affected) else {
        return Vec::new();
    };
    phrase_text(&affected, Form::All)
        .map(|who| {
            format!(
                "{} have \"bands with other legendary creatures.\"",
                capitalize(&who)
            )
        })
        .into_iter()
        .collect()
}

/// "Enchanted Wall can attack as though it didn't have defender.", "Enchanted creature can attack as though it had haste."
fn despite_defender(value: &Value) -> Vec<String> {
    let rule = |quirk: &str| json!({ "Rule": { "rule": { "Combat": { "who": "Attached", "quirk": quirk } } } });
    if value == &rule("AttackAsThoughHaste") {
        return vec!["Enchanted creature can attack as though it had haste".to_owned()];
    }
    if value != &rule("AttackDespiteDefender") {
        return Vec::new();
    }
    ["creature", "Wall"]
        .iter()
        .map(|noun| format!("Enchanted {noun} can attack as though it didn't have defender"))
        .collect()
}

/// "~ doesn't untap during your untap step unless you have an enduring story."
fn untaps_with_story(value: &Value) -> Vec<String> {
    let held = json!({ "Static": { "affected": "This", "mods": ["DoesntUntap"], "condition": { "Not": { "EnduringStory": "You" } } } });
    if value != &held {
        return Vec::new();
    }
    vec!["~ doesn't untap during your untap step unless you have an enduring story".to_owned()]
}

/// "Each land with an activated ability that isn't a mana ability doesn't untap during its controller's untap step."
fn each_doesnt_untap(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Static") else {
        return Vec::new();
    };
    if body.get("mods") != Some(&json!(["DoesntUntap"]))
        || body.as_object().map(serde_json::Map::len) != Some(2)
    {
        return Vec::new();
    }
    let Some(affected) = body
        .get("affected")
        .and_then(|affected| affected.get("Each"))
        .and_then(selector)
    else {
        return Vec::new();
    };
    if affected
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_)))
    {
        return Vec::new();
    }
    phrase_text(&affected, Form::Each)
        .map(|each| format!("Each {each} doesn't untap during its controller's untap step"))
        .into_iter()
        .collect()
}

/// "Whenever a creature deals damage to enchanted planeswalker, destroy that creature."
fn damage_destroys(value: &Value) -> Vec<String> {
    let trigger = json!({ "Triggered": {
        "when": { "DealsDamage": { "source": [{ "Type": "Creature" }], "to": "Attached" } },
        "effects": [{ "Destroy": { "what": "Dealer" } }]
    } });
    if value != &trigger {
        return Vec::new();
    }
    vec![
        "Whenever a creature deals damage to enchanted planeswalker, destroy that creature"
            .to_owned(),
    ]
}

/// "Each opponent can cast spells only any time they could cast a sorcery.", "Creature cards in graveyards can't enter the
/// battlefield.", "Players can't cast spells from graveyards or libraries.": prohibitions of casting and entering.
fn prohibitions(value: &Value) -> Vec<String> {
    let Some(cant) = value
        .get("Rule")
        .filter(|rule| rule.as_object().map(serde_json::Map::len) == Some(1))
        .and_then(|rule| rule.get("rule"))
        .and_then(|rule| rule.get("Cant"))
    else {
        return Vec::new();
    };
    let (Some(who), Some(action)) = (cant.get("who"), cant.get("action")) else {
        return Vec::new();
    };
    let creatures = |zone: &str| json!({ "Each": [{ "Type": "Creature" }, { "In": zone }] });
    let zone = |zone: &str| json!({ "In": zone });
    let text = if who == "Opponents" && action == &json!({ "Cast": { "unless": "SorceryTiming" } })
    {
        "Each opponent can cast spells only any time they could cast a sorcery"
    } else if action == "EnterBattlefield" && who == &creatures("Graveyard") {
        "Creature cards in graveyards can't enter the battlefield"
    } else if action == "EnterBattlefield"
        && who == &json!({ "Union": [creatures("Graveyard"), creatures("Library")] })
    {
        "Creature cards in graveyards and libraries can't enter the battlefield"
    } else if who == "EachPlayer" && action == &json!({ "Cast": { "filter": [zone("Graveyard")] } })
    {
        "Players can't cast spells from graveyards"
    } else if who == "EachPlayer"
        && action
            == &json!({ "Cast": { "filter": [{ "AnyOf": [zone("Graveyard"), zone("Library")] }] } })
    {
        "Players can't cast spells from graveyards or libraries"
    } else {
        return Vec::new();
    };
    vec![text.to_owned()]
}

/// "Whenever you cast a spell during combat, you get an experience counter."
fn cast_in_combat(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Triggered") else {
        return Vec::new();
    };
    let shape = json!({
        "when": { "SpellCast": { "by": "You" } },
        "if": { "AtTrigger": "InCombat" },
        "effects": [{ "PutCounters": { "on": "You", "counter": { "Other": "experience" } } }]
    });
    if body != &shape {
        return Vec::new();
    }
    vec!["Whenever you cast a spell during combat, you get an experience counter".to_owned()]
}

/// "Spells your opponents cast that target you or a permanent you control cost {2} more to cast."
fn targeting_you_costs(value: &Value) -> Vec<String> {
    let Some(body) = value.get("CostModifier") else {
        return Vec::new();
    };
    let applies = json!(["Spell", { "ControlledBy": "Opponents" }, { "AnyOf": [{ "TargetsPlayer": "You" }, { "Targeting": [{ "ControlledBy": "You" }] }] }]);
    let (Some(mana), true, true) = (
        body.get("mana").and_then(Value::as_str),
        body.get("applies_to") == Some(&applies),
        body.get("change") == Some(&json!("Increase")),
    ) else {
        return Vec::new();
    };
    if body.as_object().map(serde_json::Map::len) != Some(3) {
        return Vec::new();
    }
    vec![format!(
        "Spells your opponents cast that target you or a permanent you control cost {mana} more to cast"
    )]
}

/// "~ can't attack or block unless there are seven or more cards in exile."
fn unless_exiled(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Static") else {
        return Vec::new();
    };
    let shape = |many: u64| {
        json!({ "affected": "This", "mods": ["CantAttack", "CantBlock"], "condition": { "Compare": {
            "left": { "Count": [{ "In": "Exile" }] }, "op": "Lt", "right": many } } })
    };
    let Some(many) = body
        .pointer("/condition/Compare/right")
        .and_then(Value::as_u64)
        .filter(|many| body == &shape(*many))
    else {
        return Vec::new();
    };
    u32::try_from(many)
        .ok()
        .and_then(count_word)
        .map(|words| {
            format!("~ can't attack or block unless there are {words} or more cards in exile")
        })
        .into_iter()
        .collect()
}

/// "~ has hexproof unless it's attacking or blocking."
fn hexproof_unless_fighting(value: &Value) -> Vec<String> {
    let shape = json!({ "Static": { "affected": "This", "mods": [{ "AddKeyword": "Hexproof" }], "condition": { "Not": {
        "Matches": { "subject": "This", "selector": [{ "AnyOf": ["Attacking", "Blocking"] }] } } } } });
    if value != &shape {
        return Vec::new();
    }
    vec!["~ has hexproof unless it's attacking or blocking".to_owned()]
}

/// "Snow lands your opponents control enter tapped."
fn enter_tapped(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Replacement") else {
        return Vec::new();
    };
    if body.get("by") != Some(&json!("Tapped"))
        || body.as_object().map(serde_json::Map::len) != Some(2)
    {
        return Vec::new();
    }
    let Some(what) = body.pointer("/when/Enters/what").and_then(selector) else {
        return Vec::new();
    };
    if what.0.iter().any(|pred| matches!(pred, Pred::This)) {
        return Vec::new();
    }
    [Form::All, Form::Targets]
        .into_iter()
        .filter_map(|form| phrase_text(&what, form))
        .map(|all| format!("{} enter tapped", capitalize(&all)))
        .collect()
}

/// "During your turn, each Lesson card in your graveyard has flashback {1}.", "Each instant and sorcery card in your
/// graveyard that's exactly two colors has jump-start.": keywords given to cards in your graveyard.
fn graveyard_keywords(value: &Value) -> Vec<String> {
    let mine = [json!({ "In": "Graveyard" }), json!({ "OwnedBy": "You" })];
    let spells = json!({ "AnyOf": [{ "Type": "Instant" }, { "Type": "Sorcery" }] });
    let given = |kind: Vec<Value>, keyword: Value, turn: bool| {
        let mut each = kind;
        each.extend(mine.iter().cloned());
        let mut body = json!({ "affected": { "Each": each }, "mods": [{ "AddKeyword": keyword }] });
        if turn {
            body["condition"] = json!("YourTurn");
        }
        json!({ "Static": body })
    };
    let own_cost = json!({ "Flashback": ["OwnManaCost"] });
    let text = if value
        == &given(
            vec![spells.clone(), json!({ "Not": { "Subtype": "Lesson" } })],
            own_cost,
            true,
        ) {
        "During your turn, each non-Lesson instant and sorcery card in your graveyard has flashback. The flashback cost is equal to that card's mana cost"
    } else if value
        == &given(
            vec![json!({ "Subtype": "Lesson" })],
            json!({ "Flashback": "{1}" }),
            true,
        )
    {
        "During your turn, each Lesson card in your graveyard has flashback {1}"
    } else if value
        == &given(
            vec![
                spells,
                json!({ "Compare": { "stat": "Colors", "op": "Eq", "to": 2 } }),
            ],
            json!("JumpStart"),
            false,
        )
    {
        "Each instant and sorcery card in your graveyard that's exactly two colors has jump-start"
    } else {
        return Vec::new();
    };
    vec![text.to_owned()]
}

/// "If a creature spell you cast this turn was countered by a spell or ability an opponent controlled, you may pay {0}
/// rather than pay ~'s mana cost.", and the same of a noncreature permanent destroyed.
fn trap_costs(value: &Value) -> Vec<String> {
    let Some(body) = value.get("FreeCast") else {
        return Vec::new();
    };
    let Some(tally) = body.pointer("/condition/Compare") else {
        return Vec::new();
    };
    let shape = |event: &str, filter: Value| json!({ "left": { "Tally": { "event": event, "who": "Opponents", "filter": filter } }, "op": "Ge", "right": 1 });
    let held = if tally
        == &shape(
            "Countered",
            json!([{ "Type": "Creature" }, { "ControlledBy": "You" }]),
        ) {
        "a creature spell you cast this turn was countered by a spell or ability an opponent controlled"
    } else if tally
        == &shape(
            "Destroyed",
            json!([{ "Not": { "Type": "Creature" } }, { "ControlledBy": "You" }]),
        )
    {
        "a noncreature permanent under your control was destroyed this turn by a spell or ability an opponent controlled"
    } else {
        return Vec::new();
    };
    let cost = match body.get("cost") {
        None => "{0}".to_owned(),
        Some(Value::String(mana)) if mana.starts_with('{') => mana.clone(),
        Some(_) => return Vec::new(),
    };
    vec![format!(
        "If {held}, you may pay {cost} rather than pay ~'s mana cost"
    )]
}

/// Single sentences of one card each: "Snow lands your opponents control enter tapped.", a Saga chapter that deals damage
/// to each opponent and their planeswalkers, a cost tax given to your creatures and enchantments, and creatures with no
/// abilities assigning combat damage as though unblocked.
fn sundry(value: &Value) -> Vec<String> {
    let snow = json!({ "Replacement": {
        "when": { "Enters": { "what": [{ "Supertype": "Snow" }, { "Type": "Land" }, { "ControlledBy": "Opponents" }] } },
        "by": "Tapped"
    } });
    let chapter = json!({ "Triggered": {
        "when": { "Chapter": { "number": 1 } },
        "effects": [{ "DealDamage": {
            "to": { "Union": ["Opponents", { "Each": [{ "Type": "Planeswalker" }, { "ControlledBy": "Opponents" }] }] },
            "amount": 1
        } }]
    } });
    let taxed = json!({ "Static": {
        "affected": { "Each": [{ "AnyOf": [{ "Type": "Creature" }, { "Type": "Enchantment" }] }, { "ControlledBy": "You" }] },
        "mods": [{ "GrantAbility": { "CostModifier": {
            "applies_to": ["Spell", { "ControlledBy": "Opponents" }, { "Targeting": ["This"] }],
            "change": "Increase",
            "mana": "{1}"
        } } }]
    } });
    let unblocked = json!({ "Rule": { "rule": { "Combat": {
        "who": { "Each": [{ "Type": "Creature" }, { "ControlledBy": "You" }, { "Not": { "HasAbility": "Any" } }] },
        "quirk": "AssignsAsUnblocked"
    } } } });
    let encore = |kind: Value| {
        json!({ "Static": {
            "affected": { "Each": [kind, { "Type": "Creature" }, { "In": "Graveyard" }, { "OwnedBy": "You" }] },
            "mods": [{ "AddKeyword": { "Encore": ["OwnManaValue"] } }]
        } })
    };
    let outlaws = json!({ "AnyOf": [
        { "Subtype": "Assassin" }, { "Subtype": "Mercenary" }, { "Subtype": "Pirate" }, { "Subtype": "Rogue" }, { "Subtype": "Warlock" }
    ] });
    let legends = json!({ "Rule": { "rule": { "NoLegendRuleFor": [{ "Subtype": "Sliver" }, { "ControlledBy": "You" }] } } });
    let lose = json!({ "Triggered": {
        "when": { "ZoneChange": { "what": ["This"], "from": "Battlefield", "to": "Graveyard" } },
        "effects": [{ "LoseGame": {} }]
    } });
    let rager = json!({ "Triggered": {
        "when": { "StepBegins": { "step": "Upkeep" } },
        "effects": [{ "CreateToken": { "who": "Triggering", "token": { "Custom": {
            "name": "Lightning Rager", "colors": ["Red"], "types": ["Creature"], "subtypes": ["Elemental"],
            "power": 5, "toughness": 1,
            "abilities": [
                { "Keyword": "Trample" }, { "Keyword": "Haste" },
                { "Triggered": { "when": { "StepBegins": { "step": "End" } }, "effects": [{ "Sacrifice": { "filter": ["This"] } }] } }
            ]
        } } } }]
    } });
    let rager_cant = json!({ "Static": {
        "affected": { "Each": [{ "Type": "Creature" }, { "Named": "Lightning Rager" }] },
        "mods": [{ "CantAttackYou": "Both" }]
    } });
    let halls = json!({ "Static": { "affected": { "Each": [{ "Type": "Creature" }, "AttackedLastTurn"] }, "mods": ["CantAttack"] } });
    let kelp = json!({ "Static": {
        "affected": "Attached",
        "mods": ["DoesntUntap"],
        "condition": { "Matches": { "subject": "Attached", "selector": ["AttackedLastTurn"] } }
    } });
    let texts: &[&str] = if value == &halls {
        &["Creatures that attacked during their controller's last turn can't attack"]
    } else if value == &kelp {
        &[
            "Enchanted creature doesn't untap during its controller's untap step if it attacked during its controller's last turn",
        ]
    } else if value == &lose {
        &["When ~ is put into a graveyard from the battlefield, you lose the game"]
    } else if value == &rager {
        &[
            "At the beginning of each player's upkeep, that player creates a 5/1 red Elemental creature token named Lightning Rager. It has trample, haste, and \"At the beginning of the end step, sacrifice this token.\"",
        ]
    } else if value == &rager_cant {
        &["Creatures named Lightning Rager can't attack you or planeswalkers you control"]
    } else if value == &encore(json!({ "Subtype": "Sliver" })) {
        &["Each Sliver creature card in your graveyard has encore {X}, where X is its mana value"]
    } else if value == &encore(outlaws) {
        &["Each outlaw creature card in your graveyard has encore {X}, where X is its mana value"]
    } else if value == &legends {
        &["The \"legend rule\" doesn't apply to Slivers you control"]
    } else if value == &snow {
        &["Snow lands your opponents control enter tapped"]
    } else if value == &chapter {
        &[
            "~ deals 1 damage to each opponent and each planeswalker they control",
            "This Saga deals 1 damage to each opponent and each planeswalker they control",
        ]
    } else if value == &taxed {
        &[
            "Creatures and enchantments you control have \"Spells your opponents cast that target this permanent cost {1} more to cast.\"",
        ]
    } else if value == &unblocked {
        &[
            "For each creature you control with no abilities, you may have that creature assign its combat damage as though it weren't blocked",
        ]
    } else {
        return Vec::new();
    };
    texts.iter().map(|text| (*text).to_owned()).collect()
}

/// "The monarch controls enchanted creature."
fn monarch_controls(value: &Value) -> Vec<String> {
    if value
        != &json!({ "Static": { "affected": "Attached", "mods": [{ "SetController": "Monarch" }] } })
    {
        return Vec::new();
    }
    vec!["The monarch controls enchanted creature".to_owned()]
}

/// "If a source an opponent controls would deal damage to you or a permanent you control, prevent 1 of that damage."
fn prevent_one(value: &Value) -> Vec<String> {
    let shape = json!({ "Replacement": {
        "when": { "Damage": { "to": { "Union": ["You", { "Each": [{ "ControlledBy": "You" }] }] }, "source": [{ "ControlledBy": "Opponents" }] } },
        "by": { "PreventUpTo": 1 }
    } });
    if value != &shape {
        return Vec::new();
    }
    vec!["If a source an opponent controls would deal damage to you or a permanent you control, prevent 1 of that damage".to_owned()]
}

/// "except it's a Reflection in addition to its other creature types": the copy's added creature type said as the card
/// says it.
fn other_creature_types(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(" in addition to its other types")?;
    let (_, word) = head.rsplit_once("except it's a ")?;
    let subtype: mtg_core::card::Subtype = serde_json::from_value(json!(word)).ok()?;
    subtype
        .is_creature_type()
        .then(|| format!("{head} in addition to its other creature types{tail}"))
}
