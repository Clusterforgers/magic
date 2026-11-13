//! Wordings of the card pack r28-0 that read the texts every other writer made: a target's amount read from a card
//! revealed at random from a hand ("Target creature gets +X/+X until end of turn, where X is the revealed card's mana
//! value."), a target spell or permanent ("Target spell or permanent becomes the color of your choice until end of
//! turn.") and every permanent with a name ("destroy all permanents named Wirefly").

use crate::ability::Ability;
use crate::effect::Effect;
use crate::effect::ValueKind;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::{Amount, Condition};
use serde_json::{Value, json};

/// The words the plain writer gives the mana value of a bound card, for each noun it may say.
const BOUND_VALUES: [&str; 3] = [
    "the mana value of the card exiled this way",
    "the mana value of the permanent exiled this way",
    "the mana value of the creature exiled this way",
];

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    spell_or_permanent(ability, texts);
    named_permanents(ability, texts);
    revealed_value(ability, texts);
    skip_after(ability, texts);
    each_creature_cant(texts);
    several_lands_become(ability, texts);
    all_creature_types(ability, texts);
    from_a_graveyard(ability, texts);
    dies_this_turn(ability, texts);
    targets_only_this(ability, texts);
    if_this_is_a_type(ability, texts);
    becomes_a_type(ability, texts);
    stolen_unless_paid(ability, texts);
    source_controller(ability, texts);
    x_not_zero(ability, texts);
    creatures_and_vehicles(ability, texts);
    lost_while_tapped(ability, texts);
    returned_face_down(ability, texts);
    each_prepared(ability, texts);
    named_and_revealed(ability, texts);
    became_for_good(ability, texts);
}

/// "~ becomes a Human Detective with base power and toughness 4/4 and gains vigilance.", and the same under "If ~ is a
/// Citizen, it becomes ...": a change of the source for good, written from the twin that lasts until end of turn.
fn became_for_good(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    let (subtype, change) = match effects.as_slice() {
        [change @ Effect::Continuous { .. }] => (None, change),
        [
            Effect::If {
                condition:
                    Condition::Matches {
                        subject: Ref::This,
                        selector,
                    },
                then,
                otherwise,
            },
        ] if otherwise.is_empty() => {
            let ([Pred::Subtype(subtype)], [change @ Effect::Continuous { .. }]) =
                (selector.0.as_slice(), then.as_slice())
            else {
                return;
            };
            (Some(subtype), change)
        }
        _ => return,
    };
    let Effect::Continuous {
        affected: Ref::This,
        mods,
        duration: Duration::Forever,
    } = change
    else {
        return;
    };
    if !mods
        .iter()
        .any(|m| matches!(m, Modification::SetCreatureTypes(_)))
    {
        return;
    }
    let mut twin = ability.clone();
    if let Ability::Activated { effects, .. } = &mut twin {
        *effects = vec![Effect::Continuous {
            affected: Ref::This,
            mods: mods.clone(),
            duration: Duration::UntilEndOfTurn,
        }];
    }
    let mut found = Vec::new();
    for text in crate::compile::written_variants(&twin) {
        let Some((cost, rest)) = text.split_once(": ~ becomes ") else {
            continue;
        };
        let Some(change) = rest.strip_suffix(" until end of turn.") else {
            continue;
        };
        match subtype {
            None => found.push(format!("{cost}: ~ becomes {change}.")),
            Some(subtype) => {
                let subtype = &subtype.0;
                let article = if subtype.starts_with(['A', 'E', 'I', 'O', 'U']) {
                    "an"
                } else {
                    "a"
                };
                found.push(format!(
                    "{cost}: If ~ is {article} {subtype}, it becomes {change}."
                ));
                if let Some((head, keyword)) = change.rsplit_once(" and gains ") {
                    found.push(format!(
                        "{cost}: If ~ is {article} {subtype}, it becomes {head} and {keyword}."
                    ));
                }
            }
        }
    }
    texts.extend(found);
}

/// "Choose a card name, then reveal a card at random from your hand. If that card has the chosen name, ~ deals 2 damage
/// to any target.": written from the twin that only deals the damage.
fn named_and_revealed(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    let [
        Effect::ChooseValue {
            who: Ref::You,
            kind: ValueKind::CardName(any),
            bind: name,
        },
        Effect::ChooseAtRandom {
            from: Ref::Each(hand),
            count: Amount::Fixed(1),
            bind: picked,
        },
        Effect::Reveal {
            what: Ref::Bound(revealed),
        },
        Effect::If {
            condition:
                Condition::Matches {
                    subject: Ref::Bound(tested),
                    selector,
                },
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let yours = [Pred::OwnedBy(Ref::You), Pred::In(ZoneKind::Hand)];
    if !any.0.is_empty()
        || hand.0 != yours
        || picked != revealed
        || tested != picked
        || selector.0 != [Pred::ChosenAs(name.clone())]
        || !otherwise.is_empty()
        || serde_json::to_string(then).is_ok_and(|text| text.contains("Bound"))
    {
        return;
    }
    let mut twin = ability.clone();
    if let Ability::Activated { effects, .. } = &mut twin {
        effects.clone_from(then);
    }
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (cost, rest) = text.split_once(": ")?;
            let (sentence, after) = rest.split_once('.')?;
            Some(format!(
                "{cost}: Choose a card name, then reveal a card at random from your hand. If that card has the chosen name, {}.{after}",
                super::twin_json::lowered(sentence)
            ))
        })
        .collect();
    texts.extend(found);
}

/// The ability as JSON with `from` replaced by `to`, read back; `None` when `from` is not in it.
fn json_twin(ability: &Ability, from: &str, to: &str) -> Option<Ability> {
    let text = serde_json::to_string(ability).ok()?;
    text.contains(from)
        .then(|| serde_json::from_str(&text.replace(from, to)).ok())
        .flatten()
}

/// "~'s controller loses 2 life": the controller of the source as the player of a step, written from the twin that names
/// each opponent.
fn source_controller(ability: &Ability, texts: &mut Vec<String>) {
    let twin = json_twin(
        ability,
        r#""who":{"ControllerOf":"This"}"#,
        r#""who":"Opponents""#,
    );
    swapped(
        twin.clone(),
        "Each opponent loses ",
        "~'s controller loses ",
        texts,
    );
    swapped(
        twin,
        " each opponent loses ",
        " ~'s controller loses ",
        texts,
    );
}

/// "Transform ~. X can't be 0. Activate only as a sorcery." and "Kicker {X}. X can't be 0.": an activation or a keyword
/// cost whose X is at least 1, written from the twin without that part of the cost.
fn x_not_zero(ability: &Ability, texts: &mut Vec<String>) {
    let Some(twin) = json_twin(ability, r#",{"XAtLeast":1}"#, "") else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.contains("{X}"))
        .map(|text| match text.find(" Activate only") {
            Some(at) => format!("{} X can't be 0.{}", &text[..at], &text[at..]),
            // A keyword's cost ("Kicker {X}. X can't be 0.") ends with no stop of its own.
            None if matches!(ability, Ability::Keyword(_)) => format!("{text}. X can't be 0."),
            None => format!("{text} X can't be 0."),
        })
        .collect();
    texts.extend(found);
}

/// "Creatures and Vehicles you control gain trample until end of turn.": written from the twin about creatures alone.
fn creatures_and_vehicles(ability: &Ability, texts: &mut Vec<String>) {
    let twin = json_twin(
        ability,
        r#"[{"AnyOf":[{"Type":"Creature"},{"Subtype":"Vehicle"}]},{"ControlledBy":"You"}]"#,
        r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    );
    swapped(
        twin,
        "Creatures you control ",
        "Creatures and Vehicles you control ",
        texts,
    );
}

/// "For as long as ~ remains tapped, another target permanent loses all abilities and can't attack or block.": written
/// from the twin that keeps the restriction for the turn.
fn lost_while_tapped(ability: &Ability, texts: &mut Vec<String>) {
    let Some(twin) = json_twin(
        ability,
        r#""mods":["LoseAbilities","CantAttack","CantBlock"],"duration":"WhileSourceTapped""#,
        r#""mods":["CantAttack","CantBlock"],"duration":"UntilEndOfTurn""#,
    ) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (cost, rest) = text.split_once(": ")?;
            let subject = rest.strip_suffix(" can't attack or block this turn.")?;
            let subject = super::twin_json::lowered(subject);
            Some(format!(
                "{cost}: For as long as ~ remains tapped, {subject} loses all abilities and can't attack or block."
            ))
        })
        .collect();
    texts.extend(found);
}

/// "Exile ~, then return it to the battlefield face down under its owner's control.": written from the twin that returns
/// it face up.
fn returned_face_down(ability: &Ability, texts: &mut Vec<String>) {
    let twin = json_twin(
        ability,
        r#"{"BattlefieldFaceDown":{"under":{"OwnerOf":{"Bound":"exiled"}}}}"#,
        r#"{"Battlefield":{}}"#,
    );
    swapped(
        twin,
        "return it to the battlefield under its owner's control",
        "return it to the battlefield face down under its owner's control",
        texts,
    );
}

/// "Each creature you control becomes prepared.": written from the twin that prepares the source.
fn each_prepared(ability: &Ability, texts: &mut Vec<String>) {
    let twin = json_twin(
        ability,
        r#"{"Prepare":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#,
        r#"{"Prepare":{"what":"This"}}"#,
    );
    swapped(
        twin,
        "~ becomes prepared",
        "Each creature you control becomes prepared",
        texts,
    );
}

/// "Whenever a permanent an opponent controls is put into a graveyard, put that card onto the battlefield under your
/// control unless that opponent pays 3 life.": the card that left taken unless its controller pays, written from the twin
/// that draws a card instead.
fn stolen_unless_paid(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let taken = r#""unpaid":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]"#;
    let opponents = r#"{"ZoneChange":{"what":[{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}}"#;
    if !text.contains(taken)
        || !text.contains(opponents)
        || !text.contains(r#""who":{"ControllerOf":"Triggering"}"#)
    {
        return;
    }
    let Ok(twin) =
        serde_json::from_str::<Ability>(&text.replace(taken, r#""unpaid":[{"Draw":{}}]"#))
    else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (head, payer) = text.split_once(", draw a card unless that permanent's controller pays ")?;
            let head = head
                .strip_suffix(" dies")
                .or_else(|| head.strip_suffix(" is put into a graveyard from the battlefield"))?;
            Some(format!(
                "{head} is put into a graveyard, put that card onto the battlefield under your control unless that opponent pays {payer}"
            ))
        })
        .collect();
    texts.extend(found);
}

/// "Exile a Fungus card from a graveyard: Put a spore counter on each Fungus on the battlefield.": a cost that takes a
/// card from any graveyard, written as the twin that takes it from yours, and counters put on each permanent of a kind.
fn from_a_graveyard(ability: &Ability, texts: &mut Vec<String>) {
    let any = json!({"any_player": true}).to_string();
    let any = any.trim_start_matches('{').trim_end_matches('}');
    let twin = serde_json::to_string(ability).ok().and_then(|text| {
        text.contains(any)
            .then(|| serde_json::from_str::<Ability>(&text.replace(&format!(",{any}"), "")).ok())
            .flatten()
    });
    let Some(twin) = twin else {
        return;
    };
    for text in crate::compile::written_variants(&twin) {
        let Some((cost, effect)) = text.split_once(": ") else {
            continue;
        };
        if cost.matches(" from your graveyard").count() != 1 {
            continue;
        }
        let cost = cost.replacen(" from your graveyard", " from a graveyard", 1);
        let mut wordings = vec![format!("{cost}: {effect}")];
        let each_kind = effect
            .strip_suffix('.')
            .and_then(|rest| rest.rsplit_once(" on each "))
            .is_some_and(|(_, kind)| !kind.contains(' '));
        if each_kind {
            wordings.push(format!(
                "{cost}: {} on the battlefield.",
                &effect[..effect.len() - 1]
            ));
        }
        for wording in wordings {
            if !texts.contains(&wording) {
                texts.push(wording);
            }
        }
    }
}

/// "Choose target creature with a counter on it. When that creature dies this turn, return that card to its owner's
/// hand.": the card that died returned, written from the twin that draws a card instead.
fn dies_this_turn(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let returned = r#"[{"Move":{"what":"Triggering","to":"Hand"}}]"#;
    if !text.contains(r#""this_turn":true"#) || text.matches(returned).count() != 1 {
        return;
    }
    let Ok(twin) = serde_json::from_str::<Ability>(&text.replace(returned, r#"[{"Draw":{}}]"#))
    else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (cost, rest) = text.split_once(": When target ")?;
            let (object, _) = rest.split_once(" dies this turn, draw a card.")?;
            Some(format!(
                "{cost}: Choose target {object}. When that creature dies this turn, return that card to its owner's hand."
            ))
        })
        .collect();
    texts.extend(found);
}

/// "Change the target of target spell that targets only ~.": a spell with a single target that is the source, written
/// from the twin with a single target of any kind.
fn targets_only_this(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let only = r#"["Spell","SingleTarget",{"Targeting":["This"]}]"#;
    if !text.contains(only) {
        return;
    }
    let Ok(twin) =
        serde_json::from_str::<Ability>(&text.replace(only, r#"["Spell","SingleTarget"]"#))
    else {
        return;
    };
    swapped(
        Some(twin),
        "target spell with a single target",
        "target spell that targets only ~",
        texts,
    );
}

/// "Whenever an opponent casts a creature spell, if this permanent is an enchantment, it becomes a 2/4 Soldier
/// creature.": an intervening "if" that the source has a card type, written from the twin without it.
fn if_this_is_a_type(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if:
            Some(Condition::Matches {
                subject: Ref::This,
                selector,
            }),
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return;
    };
    let [Pred::Type(kind)] = selector.0.as_slice() else {
        return;
    };
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: None,
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    let noun = format!("{kind:?}").to_lowercase();
    let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ~ ")?;
            Some(format!("{head}, if ~ is {article} {noun}, it {rest}"))
        })
        .collect();
    texts.extend(found);
}

/// "{0}: This permanent becomes an enchantment.": the source's card types replaced by one for good.
fn becomes_a_type(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    let [
        Effect::Continuous {
            affected: Ref::This,
            mods,
            duration: Duration::Forever,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [Modification::SetTypes(kinds)] = mods.as_slice() else {
        return;
    };
    let [kind] = kinds.as_slice() else {
        return;
    };
    let mut twin = ability.clone();
    if let Ability::Activated { effects, .. } = &mut twin {
        *effects = vec![Effect::Draw {
            who: Ref::You,
            count: Amount::one(),
            bind: None,
        }];
    }
    let noun = format!("{kind:?}").to_lowercase();
    let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    swapped(
        Some(twin),
        "Draw a card",
        &format!("~ becomes {article} {noun}"),
        texts,
    );
}

/// "Put an eon counter on ~. Skip your next turn.": the skip said as a sentence of its own, without "you".
fn skip_after(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let skip = r#"{"Rule":{"rule":{"Cant":{"who":"You","action":"SkipTurn"}},"duration":"UntilTurnSkipped"}}"#;
    if !text.contains(skip) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.matches(" and skip your next turn").count() == 1)
        .map(|text| text.replacen(" and skip your next turn", ". Skip your next turn", 1))
        .collect();
    texts.extend(found);
}

/// "Each creature with mana value X can't attack or block this turn.": a restriction of a group of creatures said of
/// each, whose verb "can't" reads the same in the singular.
fn each_creature_cant(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (cost, rest) = text.split_once(": Creatures with ")?;
            let (filter, after) = rest.split_once(" can't ")?;
            (!filter.contains(' ') || filter.starts_with("mana value"))
                .then(|| format!("{cost}: Each creature with {filter} can't {after}"))
        })
        .collect();
    texts.extend(found);
}

/// "X target lands become Forests until end of turn": X targets of one kind given a land type, written as the twin with
/// one target.
fn several_lands_become(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let x = r#""bounds":{"min":"X","max":"X"}"#;
    if !text.contains(x) || !text.contains("SetLandTypes") {
        return;
    }
    let Some(twin) = serde_json::from_str::<Ability>(&text.replace(&format!(",{x}"), ""))
        .ok()
        .or_else(|| serde_json::from_str(&text.replace(x, "")).ok())
    else {
        return;
    };
    for land in ["Plains", "Island", "Swamp", "Mountain", "Forest"] {
        let plural = if land == "Plains" {
            land.to_owned()
        } else {
            format!("{land}s")
        };
        let single = format!("arget land becomes a {land} ");
        let several = format!("X target lands become {plural} ");
        let found: Vec<String> = crate::compile::written_variants(&twin)
            .into_iter()
            .filter(|text| text.matches(&single).count() == 1)
            .filter_map(|text| {
                let at = text.find(&single)?.checked_sub(1)?;
                Some(format!(
                    "{}{several}{}",
                    &text[..at],
                    &text[at + 1 + single.len()..]
                ))
            })
            .collect();
        texts.extend(found);
    }
}

/// "~ becomes a 3/3 creature with vigilance and all creature types": the twin without every creature type, with the
/// types said after its keywords.
fn all_creature_types(ability: &Ability, texts: &mut Vec<String>) {
    let all = Value::String("AddAllCreatureTypes".to_owned());
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let mods = [format!(",{all}"), format!("{all},")];
    let Some(twin) = mods.iter().find_map(|cut| {
        text.contains(cut.as_str())
            .then(|| serde_json::from_str::<Ability>(&text.replacen(cut.as_str(), "", 1)).ok())
            .flatten()
    }) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches(" creature with ").count() == 1)
        .filter_map(|text| {
            let at = text.find(" creature with ")?;
            let end = at + text[at..].find('.')?;
            let said = &text[at..end];
            (!said.contains(" until ") && !said.contains(" in addition "))
                .then(|| format!("{} and all creature types{}", &text[..end], &text[end..]))
        })
        .collect();
    texts.extend(found);
}

/// The words a twin with one part changed is written with, said back with the words of the part it stands for: each
/// written wording of the twin that has `twin_words` exactly once, with `words` in their place.
fn swapped(twin: Option<Ability>, twin_words: &str, words: &str, texts: &mut Vec<String>) {
    let Some(twin) = twin else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches(twin_words).count() == 1)
        .map(|text| text.replacen(twin_words, words, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// The ability with every occurrence of one JSON value replaced by another; `None` when it has none.
fn replaced(ability: &Ability, from: &Value, to: &Value) -> Option<Ability> {
    let text = serde_json::to_string(ability).ok()?;
    let (from, to) = (from.to_string(), to.to_string());
    text.contains(&from)
        .then(|| serde_json::from_str(&text.replace(&from, &to)).ok())
        .flatten()
}

/// "Target spell or permanent ...": written as the twin that targets a permanent.
fn spell_or_permanent(ability: &Ability, texts: &mut Vec<String>) {
    let either = json!({"Object": [{"AnyOf": ["Spell", {"In": "Battlefield"}]}]});
    let twin = replaced(ability, &either, &json!({"Object": []}));
    for (twin_words, words) in [
        ("target permanent", "target spell or permanent"),
        ("Target permanent", "Target spell or permanent"),
    ] {
        swapped(twin.clone(), twin_words, words, texts);
    }
}

/// "Destroy all permanents named Wirefly": written as the twin that names creatures.
fn named_permanents(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(text) = serde_json::to_string(ability) else {
        return;
    };
    let head = r#"{"Each":[{"Named":""#;
    let Some(name) = text
        .split(head)
        .nth(1)
        .and_then(|rest| rest.split_once(r#""}]}"#))
        .map(|(name, _)| name.to_owned())
    else {
        return;
    };
    let alone = json!({"Each": [{"Named": name}]});
    let twin = replaced(
        ability,
        &alone,
        &json!({"Each": [{"Type": "Creature"}, {"Named": name}]}),
    );
    swapped(
        twin,
        &format!("all creatures named {name}"),
        &format!("all permanents named {name}"),
        texts,
    );
}

/// "where X is the revealed card's mana value" after a card revealed at random from a hand.
fn revealed_value(ability: &Ability, texts: &mut Vec<String>) {
    if !reveals_at_random(ability) {
        return;
    }
    let mut added = Vec::new();
    for text in texts.iter() {
        for said in BOUND_VALUES {
            if text.contains(said) {
                added.push(text.replace(said, "the revealed card's mana value"));
            }
        }
    }
    texts.extend(added);
}

/// Whether the ability begins with a card revealed at random from a player's hand and reads nothing else bound.
fn reveals_at_random(ability: &Ability) -> bool {
    let (Ability::Activated { effects, .. } | Ability::Spell { effects, .. }) = ability else {
        return false;
    };
    let [
        Effect::ChooseAtRandom {
            from: Ref::Each(selector),
            bind,
            ..
        },
        Effect::Reveal {
            what: Ref::Bound(revealed),
        },
        ..,
    ] = effects.as_slice()
    else {
        return false;
    };
    revealed == bind
        && matches!(
            selector.0.as_slice(),
            [Pred::OwnedBy(_), Pred::In(ZoneKind::Hand)]
        )
        && effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    Effect::ChooseAtRandom { .. } | Effect::Choose { .. }
                )
            })
            .count()
            == 1
}
