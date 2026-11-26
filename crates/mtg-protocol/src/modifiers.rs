//! What changes an object now, in words, each with its source as the viewer may know it (`ObjectView.modifiers`): the
//! engine's modifiers (`mtg_engine::modifiers`) worded, and identical ones from sources of one name added together.

use mtg_cards::ability::CostChange;
use mtg_cards::compile::render_ability;
use mtg_cards::modify::Duration;
use mtg_cards::read::duration_wording;
use mtg_core::color::ColorSet;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::effects::Modification;
use mtg_engine::modifiers::{
    CostChangeBy, ModifierChange, ModifierSource, cost_changes_by_source, modifiers,
};
use mtg_engine::object::{GameObject, ObjectKind};
use mtg_engine::rules::{PlayerEffect, player_effect_sources};
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;

use crate::project::{Look, Sight, given_keyword_names, type_list};
use crate::view::{ModifierView, PlayerEffectSourceView};
use crate::wording::named;

/// What one origin does, as a key of the adding up: its power and toughness change apart, so identical ones add.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Phrase {
    power: i32,
    toughness: i32,
    words: Vec<String>,
}

/// One line before identical ones are added together.
struct Line {
    phrase: Phrase,
    source: String,
    id: Option<ObjectId>,
    duration: Option<String>,
}

/// The modifiers of `object` as `sight` sees them, `name` being the name the viewer reads on it. An object the viewer
/// sees face down without its card loses what it does to itself and its copy effect, which would tell the card.
pub(crate) fn modifier_views(
    state: &GameState,
    sight: &Sight<'_>,
    object: &GameObject,
    look: Look,
    name: &str,
) -> Vec<ModifierView> {
    let id = object.id();
    let mut lines: Vec<Line> = Vec::new();
    for modifier in modifiers(state, id) {
        let (source, shown) = match &modifier.source {
            ModifierSource::Object(source) if *source == id => {
                if look != Look::Whole {
                    continue;
                }
                (name.to_owned(), Some(id))
            }
            ModifierSource::Object(source) => source_name(state, sight, *source),
            ModifierSource::Ring { .. } => ("The Ring".to_owned(), None),
            ModifierSource::Counters => (String::new(), None),
            ModifierSource::Game => ("An effect".to_owned(), None),
        };
        let mut phrase = Phrase {
            power: 0,
            toughness: 0,
            words: Vec::new(),
        };
        let mut counters = None;
        for change in &modifier.changes {
            match change {
                ModifierChange::Modification(modification) => {
                    add(&mut phrase, modification, name);
                }
                ModifierChange::Copy { name: of } => {
                    if look != Look::Whole {
                        continue;
                    }
                    phrase.words.push(format!("Copy of {of}"));
                }
                ModifierChange::Counters { kind, count } => {
                    let (power, toughness) = kind.power_toughness();
                    phrase.power += power;
                    phrase.toughness += toughness;
                    if let Some(keyword) = kind.keyword() {
                        phrase.words.extend(given_keyword_names(&[keyword]));
                    }
                    counters = Some((format!("{} counter", kind.name()), *count));
                }
            }
        }
        if phrase.power == 0 && phrase.toughness == 0 && phrase.words.is_empty() {
            continue;
        }
        let duration = modifier
            .duration
            .as_ref()
            .and_then(|d| duration_text(d, &source));
        if let Some((source, count)) = counters {
            // Each counter is a source of its own: "+1/+1 counter ×3".
            for _ in 0..count {
                lines.push(Line {
                    phrase: phrase.clone(),
                    source: source.clone(),
                    id: None,
                    duration: None,
                });
            }
        } else {
            lines.push(Line {
                phrase,
                source,
                id: shown,
                duration,
            });
        }
    }
    add_up(lines)
}

/// The name the viewer reads for the source `id`, with the id when the viewer sees it on the table.
fn source_name(state: &GameState, sight: &Sight<'_>, id: ObjectId) -> (String, Option<ObjectId>) {
    let Some(source) = state.object(id) else {
        return ("A card that left the game".to_owned(), None);
    };
    match sight.look(source) {
        Look::Whole => {
            let card = if source.face_down().is_some() {
                source.whole_card()
            } else {
                source.card()
            };
            (card.name.clone(), Some(id))
        }
        Look::FaceDown => ("A face-down card".to_owned(), Some(id)),
        Look::Hidden => ("A hidden card".to_owned(), None),
    }
}

/// How long an effect lasts; "while this …" names its source, and one that lasts while its source stays or for good is no news.
fn duration_text(duration: &Duration, source: &str) -> Option<String> {
    if matches!(
        duration,
        Duration::WhileSourceOnBattlefield | Duration::Forever
    ) {
        return None;
    }
    let words = duration_wording(duration);
    Some(if words.starts_with("while ") {
        words.replace(" this", &format!(" {source}"))
    } else {
        words
    })
}

/// Identical lines added together in the order first seen: power and toughness by the count, the sources' ids listed.
fn add_up(lines: Vec<Line>) -> Vec<ModifierView> {
    let mut groups: Vec<(Line, u32, Vec<ObjectId>)> = Vec::new();
    for line in lines {
        let same = groups.iter_mut().find(|(seen, ..)| {
            seen.phrase == line.phrase
                && seen.source == line.source
                && seen.duration == line.duration
        });
        if let Some((_, count, ids)) = same {
            *count += 1;
            ids.extend(line.id.filter(|id| !ids.contains(id)));
        } else {
            let ids = line.id.into_iter().collect();
            groups.push((line, 1, ids));
        }
    }
    groups
        .into_iter()
        .map(|(line, count, sources)| {
            let factor = i32::try_from(count).unwrap_or(i32::MAX);
            let mut parts = Vec::new();
            if line.phrase.power != 0 || line.phrase.toughness != 0 {
                parts.push(format!(
                    "{}/{}",
                    signed(line.phrase.power.saturating_mul(factor)),
                    signed(line.phrase.toughness.saturating_mul(factor))
                ));
            }
            parts.extend(line.phrase.words);
            ModifierView {
                effect: parts.join(", "),
                source: line.source,
                sources,
                count,
                duration: line.duration,
            }
        })
        .collect()
}

/// What changes the cost the owner of `object` pays for it now, each change with its source (`ObjectView.modifiers`):
/// the increases and reductions of cost modifiers and the colors a rule draws anew. Only for a card the viewer sees
/// whole in a hand or a command zone, where it waits to be cast; `name` is the name the viewer reads on it.
pub(crate) fn cost_views(
    state: &GameState,
    sight: &Sight<'_>,
    object: &GameObject,
    look: Look,
    name: &str,
) -> Vec<ModifierView> {
    let waiting = matches!(object.zone(), Zone::Hand(_) | Zone::Command(_));
    if look != Look::Whole || !waiting || object.kind() != ObjectKind::Card {
        return Vec::new();
    }
    let Some(printed) = object.card().mana_cost.as_ref() else {
        return Vec::new();
    };
    let owner = object.owner();
    let named = |source: ObjectId| {
        if source == object.id() {
            (name.to_owned(), Some(source))
        } else {
            source_name(state, sight, source)
        }
    };
    let mut out: Vec<ModifierView> = cost_changes_by_source(state, owner, object.id())
        .into_iter()
        .map(|change| {
            let (source, id) = named(change.source.id);
            sourced(cost_words(&change), source, id)
        })
        .collect();
    let now = mtg_engine::cost_colors::redrawn(state, owner, printed);
    if now != *printed {
        let by = player_effect_sources(state, owner)
            .into_iter()
            .find(|(effect, _)| matches!(effect, PlayerEffect::CostColorsDrawn(_)));
        let (source, id) = by.map_or_else(
            || ("An effect".to_owned(), None),
            |(_, source)| named(source.id),
        );
        out.push(sourced(format!("Costs {now} this round"), source, id));
    }
    out
}

/// The effects on `player` an object brings, each with that object as the viewer may know it (`PlayerView.sources`).
pub(crate) fn player_effect_views(
    state: &GameState,
    viewer: PlayerId,
    player: PlayerId,
) -> Vec<PlayerEffectSourceView> {
    let found = player_effect_sources(state, player);
    if found.is_empty() {
        return Vec::new();
    }
    let sight = Sight::new(state, viewer);
    found
        .into_iter()
        .map(|(effect, source)| {
            let (name, id) = source_name(state, &sight, source.id);
            PlayerEffectSourceView {
                effect,
                source: name,
                id,
            }
        })
        .collect()
}

/// One line of the preview from `source` alone, lasting as long as it does.
fn sourced(effect: String, source: String, id: Option<ObjectId>) -> ModifierView {
    ModifierView {
        effect,
        source,
        sources: id.into_iter().collect(),
        count: 1,
        duration: None,
    }
}

/// A cost change in words: "Costs {1} less", "Costs {2} more", "Costs {1} and 2 life more".
fn cost_words(change: &CostChangeBy) -> String {
    let mana = (!change.mana.symbols().is_empty()).then(|| change.mana.to_string());
    let life = (change.life > 0).then(|| format!("{} life", change.life));
    let what = [mana, life]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" and ");
    match change.change {
        CostChange::Increase => format!("Costs {what} more"),
        CostChange::Reduce => format!("Costs {what} less"),
        CostChange::R122ReduceOrGeneric => format!("Costs {what} (or that much generic mana) less"),
    }
}

/// A number with its sign: "+2", "-1", "+0".
fn signed(value: i32) -> String {
    if value < 0 {
        value.to_string()
    } else {
        format!("+{value}")
    }
}

/// The colours of a set in words, "colorless" for none.
fn colors(set: ColorSet) -> String {
    let words: Vec<String> = set.iter().map(|color| format!("{color:?}")).collect();
    if words.is_empty() {
        "colorless".to_owned()
    } else {
        words.join(" and ")
    }
}

/// Adds what one modification does to the phrase; `host` is the name of the object for `~` in a granted ability.
#[allow(clippy::too_many_lines)]
fn add(phrase: &mut Phrase, modification: &Modification, host: &str) {
    let words = match modification {
        Modification::ModifyPowerToughness { power, toughness } => {
            phrase.power += power;
            phrase.toughness += toughness;
            return;
        }
        Modification::SetPowerToughness { power, toughness } => {
            format!("Base power and toughness {power}/{toughness}")
        }
        Modification::SetPower(power) => format!("Base power {power}"),
        Modification::SetToughness(toughness) => format!("Base toughness {toughness}"),
        Modification::SwitchPowerToughness => "Power and toughness switched".to_owned(),
        Modification::AddKeyword(keyword) => {
            phrase
                .words
                .extend(given_keyword_names(std::slice::from_ref(keyword)));
            return;
        }
        Modification::AddKeywords(keywords) => {
            phrase.words.extend(given_keyword_names(keywords));
            return;
        }
        Modification::RemoveKeyword(keyword) | Modification::LockKeyword(keyword) => {
            format!("Loses {}", keyword.shown_name())
        }
        Modification::RemoveKeywordKind(kind) => format!("Loses all {kind:?}"),
        Modification::GrantAbility(ability) => render_ability(ability).map_or_else(
            || "Gains an ability".to_owned(),
            |text| format!("Gains \u{201c}{}\u{201d}", named(&text, host)),
        ),
        Modification::GainAbilities(_) => "Gains the abilities of other cards".to_owned(),
        Modification::R114StickerAbilities(_) => {
            "Has the abilities of ability stickers on other objects".to_owned()
        }
        Modification::GrantText(text) => text.clone(),
        Modification::LoseAbilities => "Loses all abilities".to_owned(),
        Modification::LoseAbilitiesExceptMana => {
            "Loses all abilities but mana abilities".to_owned()
        }
        Modification::LoseAbility(_) => "Loses an ability".to_owned(),
        Modification::SetController(_) => "Control changed".to_owned(),
        Modification::ChangeText { from, to } => format!("Text changed: {from:?} to {to:?}"),
        Modification::R612ExchangeTextBoxes => "Text box exchanged".to_owned(),
        Modification::R612TextBoxOf(_) => "Has the text box of another creature".to_owned(),
        Modification::AddType(card_type) => format!("Is also {card_type:?}"),
        Modification::RemoveType(card_type) => format!("Is not {card_type:?}"),
        Modification::SetTypes(types) => {
            let names: Vec<String> = type_list(*types).iter().map(|t| format!("{t:?}")).collect();
            format!("Is {}", names.join(" "))
        }
        Modification::AddSupertype(supertype) => format!("Is {supertype:?}"),
        Modification::RemoveSupertype(supertype) => format!("Is not {supertype:?}"),
        Modification::AddSubtype(subtype) => format!("Is also {}", subtype.0),
        Modification::R2051RemoveSubtype(subtype) => format!("Is not {}", subtype.0),
        Modification::SetCreatureTypes(subtypes) | Modification::SetLandTypes(subtypes) => {
            let names: Vec<&str> = subtypes.iter().map(|s| s.0.as_str()).collect();
            format!("Is {}", names.join(" "))
        }
        Modification::AddAllCreatureTypes => "Is every creature type".to_owned(),
        Modification::SetColors(set) => format!("Is {}", colors(*set)),
        Modification::AddColors(set) => format!("Is also {}", colors(*set)),
        Modification::AddNameWords(words) => format!("Name adds \u{201c}{words}\u{201d}"),
        Modification::SetName(name) => format!("Named {name}"),
        Modification::R2013AllCreatureNames => {
            "Has every nonlegendary creature card's name".to_owned()
        }
        Modification::CantAttack => "Can't attack".to_owned(),
        Modification::CantBlock => "Can't block".to_owned(),
        Modification::DoesntUntap => "Doesn't untap".to_owned(),
        Modification::R6700KeepsLastLoyalty => "Keeps its last loyalty counter".to_owned(),
        Modification::CantBeTurnedFaceUp => "Can't be turned face up".to_owned(),
        Modification::R119CantBeBeamedUp => "Can't be beamed up".to_owned(),
        Modification::R1012ControlKept => "Can't come under another player's control".to_owned(),
        Modification::R3034CantBeEnchanted => "Can't be enchanted".to_owned(),
        Modification::CantBeBlocked => "Can't be blocked".to_owned(),
        Modification::CantBeBlockedBy { .. } | Modification::CantBeBlockedUnlessAttackingOwner => {
            "Can't be blocked by some creatures".to_owned()
        }
        Modification::CanBlockOnly { .. } => "Can block only some creatures".to_owned(),
        Modification::MinBlockers(count) => format!("Can't be blocked except by {count} or more"),
        Modification::MaxBlockers(count) => format!("Can't be blocked by more than {count}"),
        Modification::BlockedByAll => "Must be blocked by every creature able".to_owned(),
        Modification::BlocksAdditional(count) => format!("Can block {count} more"),
        Modification::CantAttackYou { .. }
        | Modification::CantAttackOwner { .. }
        | Modification::R6601CantAttackPlayers { .. } => "Can't attack some players".to_owned(),
        Modification::Goaded { .. } => "Goaded".to_owned(),
    };
    if !phrase.words.contains(&words) {
        phrase.words.push(words);
    }
}
