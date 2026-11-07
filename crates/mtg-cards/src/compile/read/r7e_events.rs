//! Events the Forge translator models its own way, written as the card words them: the damage one object deals or is
//! dealt as one batch ("Whenever ~ is dealt damage", "Whenever equipped creature deals combat damage"), the actions of
//! `TriggerPattern::Did` ("Whenever ~ mutates", "Whenever you scry", "When ~ transforms into ~") and the spells a cast
//! trigger names by a table row ("When you cast ~", "Whenever you cast a historic spell").

use std::sync::LazyLock;

use super::selectors::{Form, phrase_text};
use super::triggers::{happening, pattern_text, player_verb, subject_text};
use crate::ability::Ability;
use crate::did::Action;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use mtg_core::card::{CardType, Subtype};

/// Whether the selector names one object: the card itself, or the creature an Aura or Equipment is attached to.
fn one_object(what: &Selector) -> bool {
    matches!(
        what.0.as_slice(),
        [Pred::This]
            | [Pred::Is(Ref::Attached)]
            | [Pred::Type(CardType::Creature), Pred::Is(Ref::Attached)]
    )
}

/// The subject of one object: "~" or "equipped creature" (an Aura's "enchanted creature" is the head's other wording).
fn one_subject(what: &Selector) -> Option<String> {
    if what.0 == [Pred::This] {
        return Some("~".to_owned());
    }
    one_object(what).then(|| "equipped creature".to_owned())
}

/// "damage", "combat damage" or "noncombat damage".
const fn kind(combat: Option<bool>) -> &'static str {
    match combat {
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
        None => "damage",
    }
}

/// The damage of one batch that one object deals or is dealt, in the singular the card uses: such an event triggers
/// once for the batch whatever the number of sources or recipients (the cards' rulings), the same as the batch.
pub(super) fn r7e_one_object_damage(inner: &TriggerPattern) -> Option<String> {
    let TriggerPattern::DealsDamage { source, to, combat } = inner else {
        return None;
    };
    if source.is_empty() {
        let damaged = match to.as_ref()? {
            Ref::This => Selector::this(),
            Ref::Attached => Selector(vec![Pred::Is(Ref::Attached)]),
            Ref::Each(what) if one_object(what) => what.clone(),
            _ => return None,
        };
        return Some(format!(
            "Whenever {} is dealt {}",
            one_subject(&damaged)?,
            kind(*combat)
        ));
    }
    let dealer = one_subject(source)?;
    let to = match to {
        None => String::new(),
        Some(Ref::EachPlayer) => " to a player".to_owned(),
        Some(Ref::Opponents) => " to an opponent".to_owned(),
        Some(Ref::You) => " to you".to_owned(),
        // "to one or more blocking creatures": every object of the group in the one batch.
        Some(Ref::Each(group)) if !group.is_empty() && !one_object(group) => {
            format!(" to one or more {}", phrase_text(group, Form::All)?)
        }
        Some(_) => return None,
    };
    Some(format!("Whenever {dealer} deals {}{to}", kind(*combat)))
}

/// "Whenever you discard one or more cards", "Whenever a player discards one or more cards", "Whenever you sacrifice
/// one or more Foods": a batch of one player's discards or sacrifices, which triggers once for each player.
pub(super) fn r7e_player_batch(inner: &TriggerPattern) -> Option<String> {
    match inner {
        TriggerPattern::Discarded { what, by } => {
            let cards = match what.0.as_slice() {
                [] => "cards".to_owned(),
                [Pred::Type(_)] => format!("{} cards", phrase_text(what, Form::Subject)?),
                _ => return None,
            };
            Some(format!(
                "Whenever {} one or more {cards}",
                player_verb(by, "discard")?
            ))
        }
        TriggerPattern::Sacrificed { what, by } if !what.is_empty() => Some(format!(
            "Whenever {} one or more {}",
            player_verb(by, "sacrifice")?,
            phrase_text(what, Form::All)?
        )),
        _ => None,
    }
}

/// The dedicated pattern an action of an object stands for: the engine finds both on the same event.
fn as_pattern(action: Action, what: &Selector) -> Option<TriggerPattern> {
    let what = what.clone();
    Some(match action {
        Action::Mutated => TriggerPattern::Mutates { what },
        Action::Exerted => TriggerPattern::Exerted { what },
        Action::Explored => TriggerPattern::Explored { what },
        Action::Connived => TriggerPattern::Connived { what },
        Action::BecameRenowned => TriggerPattern::BecomesRenowned { what },
        Action::BecameMonstrous => TriggerPattern::BecomesMonstrous { what },
        Action::Adapted => TriggerPattern::Adapted { what },
        Action::TurnedFaceUp => TriggerPattern::TurnedFaceUp { what },
        _ => return None,
    })
}

/// The verb of an action a player takes alone, with no object: "you scry", "an opponent shuffles their library".
fn player_action(action: Action, who: &Ref) -> Option<String> {
    Some(match action {
        Action::Scried => player_verb(who, "scry")?,
        Action::Surveilled => player_verb(who, "surveil")?,
        Action::Proliferated => player_verb(who, "proliferate")?,
        Action::Shuffled => match who {
            Ref::You => "you shuffle your library".to_owned(),
            _ => format!("{} their library", player_verb(who, "shuffle")?),
        },
        Action::LostGame => match who {
            Ref::You => return None,
            _ => format!("{} the game", player_verb(who, "lose")?),
        },
        _ => return None,
    })
}

/// "Whenever an opponent plays a land", "Whenever you play a Forest": the land played and, from its controller, who
/// played it.
fn land_played(what: &Selector) -> Option<String> {
    let mut rest = what.0.clone();
    let who = match rest
        .iter()
        .position(|pred| matches!(pred, Pred::ControlledBy(_)))
    {
        Some(at) => match rest.remove(at) {
            Pred::ControlledBy(who @ (Ref::You | Ref::Opponents)) => who,
            _ => return None,
        },
        None => Ref::EachPlayer,
    };
    if rest.is_empty() || rest.contains(&Pred::Other) {
        return None;
    }
    Some(format!(
        "{} {}",
        player_verb(&who, "play")?,
        subject_text(&Selector(rest))?
    ))
}

/// The event of `TriggerPattern::Did` as the card words it.
pub(super) fn r7e_did_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::Did { action, who, what } = pattern else {
        return None;
    };
    if *action == Action::PlayedLand && *who == Ref::EachPlayer {
        return Some(format!("Whenever {}", land_played(what)?));
    }
    if what.is_empty() {
        return Some(format!("Whenever {}", player_action(*action, who)?));
    }
    if *action == Action::FullyUnlocked && what.0 == [Pred::Subtype(Subtype("Room".to_owned()))] {
        return Some(format!(
            "Whenever {} a Room",
            player_verb(who, "fully unlock")?
        ));
    }
    if *who != Ref::EachPlayer {
        return None;
    }
    if *action == Action::Transformed && what.0 == [Pred::This] {
        return Some("When ~ transforms into ~".to_owned());
    }
    pattern_text(&as_pattern(*action, what)?)
}

/// The subject and verb of an action of an object, for a list of events: "~ mutates", "~ transforms into ~".
pub(super) fn r7e_did_happening(pattern: &TriggerPattern) -> Option<(Selector, String)> {
    let TriggerPattern::Did { action, who, what } = pattern else {
        return None;
    };
    if *who != Ref::EachPlayer || what.is_empty() {
        return None;
    }
    let verb = match action {
        Action::Transformed if what.0 == [Pred::This] => "transforms into ~",
        Action::Mutated => "mutates",
        Action::Explored => "explores",
        Action::Connived => "connives",
        Action::BecameRenowned => "becomes renowned",
        Action::BecameMonstrous => "becomes monstrous",
        Action::Adapted => "adapts",
        Action::TurnedFaceUp => "is turned face up",
        _ => return None,
    };
    Some((what.clone(), verb.to_owned()))
}

/// The spells a cast trigger of you names by one row: the selector as data and the words after "Whenever you cast".
const YOUR_SPELLS: [(&str, &str); 8] = [
    (
        r#"[{"AnyOf":[{"Supertype":"Legendary"},{"Type":"Artifact"},{"Subtype":"Saga"}]}]"#,
        "a historic spell",
    ),
    // Cast from a hand and owned by you is cast from your own hand.
    (
        r#"[{"AnyOf":[{"Not":{"WasCast":{"from":"Hand"}}},{"Not":{"OwnedBy":"You"}}]}]"#,
        "a spell from anywhere other than your hand",
    ),
    (r#"["Kicked"]"#, "a kicked spell"),
    (r#"[{"Not":{"OwnedBy":"You"}}]"#, "a spell you don't own"),
    (
        r#"[{"WasCast":{"from":"Graveyard"}},{"OwnedBy":"You"}]"#,
        "a spell from your graveyard",
    ),
    (r#"[{"WasCast":{"from":"Exile"}}]"#, "a spell from exile"),
    (
        r#"[{"ControlledBy":"You"},{"Targeting":[{"Type":"Creature"},{"ControlledBy":"You"},{"In":"Battlefield"}]}]"#,
        "a spell that targets a creature you control",
    ),
    (
        r#"[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"Targeting":[{"Type":"Creature"},{"In":"Battlefield"}]}]"#,
        "an instant or sorcery spell that targets a creature",
    ),
];

/// The rows of [`YOUR_SPELLS`] as selectors.
static SPELL_ROWS: LazyLock<Vec<(Selector, &'static str)>> = LazyLock::new(|| {
    YOUR_SPELLS
        .iter()
        .filter_map(|(json, words)| Some((serde_json::from_str(json).ok()?, *words)))
        .collect()
});

/// "When you cast ~" for the card cast by whoever casts it, and the spells of a row cast by you.
pub(super) fn r7e_cast_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::SpellCast { what, by } = pattern else {
        return None;
    };
    // The one who casts the card is the "you" of its own ability.
    if what.0 == [Pred::This] && *by == Ref::EachPlayer {
        return Some("When you cast ~".to_owned());
    }
    if *by != Ref::You {
        return None;
    }
    let (_, words) = SPELL_ROWS.iter().find(|(row, _)| row == what)?;
    Some(format!("Whenever you cast {words}"))
}

/// Whether an action of the card itself lets its effects call it "it": "When you do, it gets +1/+0", "Whenever ~
/// mutates, it deals 4 damage".
pub(super) fn r7e_did_itself(pattern: &TriggerPattern) -> bool {
    let TriggerPattern::Did { action, who, what } = pattern else {
        return false;
    };
    *who == Ref::EachPlayer
        && what.0 == [Pred::This]
        && matches!(
            action,
            Action::Exerted
                | Action::Mutated
                | Action::Transformed
                | Action::Explored
                | Action::Connived
        )
}

/// The events of an `AnyOf` of two as two clauses: "When you cast ~ and whenever ~ attacks", "At the beginning of your
/// upkeep and whenever you cast a red spell". A step comes first, as only it opens with "At".
fn joined(patterns: &[TriggerPattern]) -> Option<String> {
    let [first, second] = patterns else {
        return None;
    };
    // Two happenings of objects are one list of verbs ("~ enters or attacks"), or no wording at all.
    if happening(first).is_some() && happening(second).is_some() {
        return None;
    }
    let (mut first, mut second) = (pattern_text(first)?, pattern_text(second)?);
    if second.starts_with("At ") {
        std::mem::swap(&mut first, &mut second);
    }
    let opens = |text: &str| text.starts_with("When ") || text.starts_with("Whenever ");
    (opens(&first) || first.starts_with("At the beginning of ")).then_some(())?;
    opens(&second).then_some(())?;
    let mut rest = second.chars();
    let lowered: String = rest.next()?.to_lowercase().chain(rest).collect();
    Some(format!("{first} and {lowered}"))
}

/// Two events of an `AnyOf` no other writer words: "Whenever you scry or surveil", "When you cast or cycle ~", and two
/// clauses joined with "and whenever".
pub(super) fn r7e_any_of(patterns: &[TriggerPattern]) -> Option<String> {
    let scry = TriggerPattern::Did {
        action: Action::Scried,
        who: Ref::You,
        what: Selector::default(),
    };
    let surveil = TriggerPattern::Did {
        action: Action::Surveilled,
        who: Ref::You,
        what: Selector::default(),
    };
    let cast = TriggerPattern::SpellCast {
        what: Selector::this(),
        by: Ref::EachPlayer,
    };
    let cycle = TriggerPattern::Cycled {
        what: Selector::this(),
        by: Ref::EachPlayer,
    };
    let pair = |a: &TriggerPattern, b: &TriggerPattern| matches!(patterns, [x, y] if (x == a && y == b) || (x == b && y == a));
    if pair(&scry, &surveil) {
        return Some("Whenever you scry or surveil".to_owned());
    }
    if pair(&cast, &cycle) {
        return Some("When you cast or cycle ~".to_owned());
    }
    joined(patterns)
}

/// Whether the two events are you casting a spell and you playing a land, in either order.
fn land_or_spell(patterns: &[TriggerPattern]) -> bool {
    let cast = |pattern: &TriggerPattern| matches!(pattern, TriggerPattern::SpellCast { what, by: Ref::You } if what.is_empty());
    let land = |pattern: &TriggerPattern| {
        matches!(
            pattern,
            TriggerPattern::Did { action: Action::PlayedLand, who: Ref::EachPlayer, what }
                if what.0 == [Pred::Type(CardType::Land), Pred::ControlledBy(Ref::You)]
        )
    };
    matches!(patterns, [a, b] if (cast(a) && land(b)) || (land(a) && cast(b)))
}

/// The other heads the card uses for the event of a trigger.
fn other_heads(when: &TriggerPattern) -> Vec<String> {
    let mut heads = Vec::new();
    match when {
        TriggerPattern::AnyOf(patterns) => {
            heads.extend(joined(patterns));
            if land_or_spell(patterns) {
                heads.extend(
                    [
                        "Whenever you play a land or cast a spell",
                        "Whenever you play a card",
                        "When you play a card",
                    ]
                    .map(str::to_owned),
                );
            }
        }
        // The kicker of the spell cast cannot change, so the condition checked again on resolution is the same.
        TriggerPattern::SpellCast { what, by: Ref::You } if what.0 == [Pred::Kicked] => {
            heads.push("Whenever you cast a spell, if that spell was kicked".to_owned());
        }
        _ => {}
    }
    heads
}

/// The wordings the card uses for the events of this module beside the one written: two events joined with "and
/// whenever", "Whenever you play a card" for a land or a spell, "if that spell was kicked" for a kicked spell,
/// "Whenever ~ transforms into ~" beside "When", and "When" for a player who loses the game (that happens once).
pub(super) fn r7e_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { when, .. } = ability else {
        return;
    };
    let mut more: Vec<String> = Vec::new();
    if let Some(head) = pattern_text(when) {
        for other in other_heads(when) {
            more.extend(texts.iter().filter_map(|text| {
                text.strip_prefix(head.as_str())
                    .filter(|rest| rest.starts_with(','))
                    .map(|rest| format!("{other}{rest}"))
            }));
        }
    }
    let transforms = |pattern: &TriggerPattern| {
        matches!(
            pattern,
            TriggerPattern::Did {
                action: Action::Transformed,
                ..
            }
        )
    };
    let named = match when {
        TriggerPattern::AnyOf(patterns) => patterns.iter().any(transforms),
        other => transforms(other),
    };
    let lost = matches!(
        when,
        TriggerPattern::Did {
            action: Action::LostGame,
            ..
        }
    );
    more.extend(texts.iter().filter_map(|text| {
        if named && let Some(rest) = text.strip_prefix("When ~ ") {
            return rest
                .contains("transforms into ~")
                .then(|| format!("Whenever ~ {rest}"));
        }
        let rest = text.strip_prefix("Whenever ")?;
        lost.then(|| format!("When {rest}"))
    }));
    texts.extend(more);
}
