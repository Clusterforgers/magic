//! The renderer: rules data back to the oracle wording the readers read, so a reading can be
//! checked against the text it came from. An effect is a sentence of its own, or the effects of one
//! player in a row are one compound sentence; the readers take ", then" between two steps as the
//! same sequence.

mod damage_by;
mod divided;
mod graveyards;
mod may_have;

use super::amounts::{cards_text, count_word, counted_parts, points_text, stat_name};
use super::attackers;
use super::becomes;
use super::cast;
use super::conditional;
use super::copies;
use super::copy_spells;
use super::costs;
use super::counters;
use super::create;
use super::defining;
use super::draws;
use super::event_wordings;
use super::effects::{BACK, RETURNED, dead_card, returns_itself};
use super::flicker;
use super::group_pump;
use super::lands;
use super::library;
use super::modal::{BULLET, DASH};
use super::payments;
use super::prevention;
use super::pronouns;
use super::reveal;
use super::selectors::{Form, granted_text, phrase_text, spec_text};
use super::spend;
use super::spent;
use super::statics;
use super::subtypes::plural_subtype;
use super::triggers::{
    article, cast_itself, condition_text, cycled_itself, graveyard_itself, itself_acts,
    leaves_itself, pattern_text, targeted_itself,
};
use super::where_x;
use crate::ability::{Ability, Limit, Timing};
use crate::compile::classify;
use crate::cost::Cost;
use crate::effect::{Destination, Effect, Mode, Order, Place, Produce, TokenRef};
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::spend::SpendOn;
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Condition, CounterKind};

/// "draws a card for each card in your hand" is also "draws X cards, where X is the number of cards in your hand".
fn where_x_draws(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains(" a card for each card in "))
        .map(|text| {
            text.replacen(
                " a card for each card in ",
                " X cards, where X is the number of cards in ",
                1,
            )
        })
        .collect()
}

/// The oracle wording of an ability the readers produce, in the first way its effects can be
/// written (see [`variants`]); `None` for any other ability.
pub fn ability(ability: &Ability) -> Option<String> {
    variants(ability).into_iter().next()
}

/// Every wording of an ability the readers produce. The effects of one player in a row are one
/// sentence ("you draw a card and gain 3 life") or several ("draw a card. You gain 3 life"), and
/// the data does not say which a card used, so each way is written. Empty for any other ability.
#[allow(clippy::too_many_lines)]
pub fn variants(ability: &Ability) -> Vec<String> {
    let mut texts = ability_texts(ability);
    let singulars: Vec<String> = texts
        .iter()
        .filter_map(|text| spend::singular(text))
        .collect();
    texts.extend(singulars);
    texts = texts.into_iter().flat_map(cast::impulse_wordings).collect();
    let controller = create::other_wordings(&texts);
    texts.extend(controller);
    let searches = library::other_wordings(&texts);
    texts.extend(searches);
    let counted = where_x_draws(&texts);
    texts.extend(counted);
    // A card may count the casts of "a commander" for those of "your commander" (Jirina Kudro).
    let commanders: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("you've cast your commander from"))
        .map(|text| {
            text.replace(
                "you've cast your commander from",
                "you've cast a commander from",
            )
        })
        .collect();
    texts.extend(commanders);
    // A card may call the token of a copy "the token" for "it" in its exceptions (Helm of the Host).
    let tokens: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("except it isn't legendary"))
        .map(|text| {
            text.replace(
                "except it isn't legendary",
                "except the token isn't legendary",
            )
        })
        .collect();
    texts.extend(tokens);
    // A card that becomes a target may call itself "it" in the counters it puts on itself (Seedglaive Mentor).
    if matches!(ability, Ability::Triggered { when, .. } if targeted_itself(when)) {
        let pronouns: Vec<String> = texts
            .iter()
            .filter(|text| text.ends_with(" counter on ~.") || text.ends_with(" counters on ~."))
            .map(|text| text.replacen(" on ~.", " on it.", 1))
            .collect();
        texts.extend(pronouns);
    }
    // A creature that attacks enchanted player is "it", and the player who controls it "its controller".
    if matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::AttacksPlayer { .. },
            ..
        }
    ) {
        let pronouns: Vec<String> = texts
            .iter()
            .filter(|text| text.contains("that permanent's controller"))
            .map(|text| text.replace("that permanent's controller", "its controller"))
            .collect();
        texts.extend(pronouns);
    }
    // The player attacked is "that player" in a trigger on an attack at the monarch.
    if matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::AttacksPlayer {
                at: Ref::Monarch,
                ..
            },
            ..
        }
    ) {
        let pronouns: Vec<String> = texts
            .iter()
            .filter(|text| text.contains("the monarch controls"))
            .map(|text| text.replace("the monarch controls", "that player controls"))
            .collect();
        texts.extend(pronouns);
    }
    // The creature that attacks a player is "it" or "that creature" (M'Baku).
    if matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::AttacksPlayer { .. },
            ..
        }
    ) {
        let named: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(", it gets ") || text.contains(", it gains "))
            .map(|text| {
                text.replace(", it gets ", ", that creature gets ")
                    .replace(", it gains ", ", that creature gains ")
            })
            .collect();
        texts.extend(named);
    }
    // The monarch held as the event happens may follow it: "Whenever you tap a land for mana while you're the monarch".
    if let Ability::Triggered {
        when,
        intervening_if: Some(held),
        ..
    } = ability
        && while_held(when)
        && let Some(held) = super::triggers::monarch_text(held)
    {
        let after: Vec<String> = texts
            .iter()
            .map(|text| text.replacen(&format!(", if {held}, "), &format!(" while {held}, "), 1))
            .filter(|text| !texts.contains(text))
            .collect();
        texts.extend(after);
    }
    // A card may open with "When" for the damage its commander deals (Archivist of Gondor).
    if matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::DealsDamage { source, .. },
            ..
        } if source.0.contains(&Pred::Commander) && source.0.contains(&Pred::OwnedBy(Ref::You))
    ) {
        let once: Vec<String> = texts
            .iter()
            .filter_map(|text| text.strip_prefix("Whenever your commander "))
            .map(|rest| format!("When your commander {rest}"))
            .collect();
        texts.extend(once);
    }
    texts.extend(attackers::named_draws(ability, &texts));
    texts.extend(counters::pronouns_after_none(ability, &texts));
    // The counters counted by "for each" may be those "on it" for the card.
    let pronouns: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (head, tail) = text.split_at(text.find("for each ")?);
            tail.contains(" counter on ~").then(|| {
                format!(
                    "{head}{}",
                    tail.replacen(" counter on ~", " counter on it", 1)
                )
            })
        })
        .collect();
    texts.extend(pronouns);
    // An activated ability that opens with damage may say "It deals" for the source (Walking Ballista).
    if matches!(ability, Ability::Activated { .. }) {
        let pronouns: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(": ~ deals "))
            .map(|text| text.replacen(": ~ deals ", ": It deals ", 1))
            .collect();
        texts.extend(pronouns);
    }
    // A trigger about the card itself entering, attacking or dying may say "it deals" for the card (Skysovereign).
    if matches!(ability, Ability::Triggered { when, .. } if itself_acts(when) || leaves_itself(when))
    {
        let pronouns: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(", ~ deals "))
            .map(|text| text.replacen(", ~ deals ", ", it deals ", 1))
            .collect();
        texts.extend(pronouns);
    }
    // A trigger about the card itself may say "it explores" or "it connives" for the card (Seekers' Squire).
    if matches!(ability, Ability::Triggered { when, .. } if itself_acts(when)) {
        for verb in ["explores", "connives"] {
            let (named, pronoun) = (format!(", ~ {verb}"), format!(", it {verb}"));
            let pronouns: Vec<String> = texts
                .iter()
                .filter(|text| text.contains(&named))
                .map(|text| text.replacen(&named, &pronoun, 1))
                .collect();
            texts.extend(pronouns);
        }
    }
    pronouns::of_the_card(ability, &mut texts);
    pronouns::you_after_the_attached(ability, &mut texts);
    pronouns::of_the_group(ability, &mut texts);
    // A trigger about damage the source deals may say "it deals" for the source (Questing Beast).
    if matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::DealsDamage { .. },
            ..
        }
    ) {
        let pronouns: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(", ~ deals "))
            .map(|text| text.replacen(", ~ deals ", ", it deals ", 1))
            .collect();
        texts.extend(pronouns);
    }
    let more = draws::wordings(ability, &texts);
    texts.extend(more);
    let more = becomes::wordings(ability, &texts);
    texts.extend(more);
    let more = group_pump::wordings(ability, &texts);
    texts.extend(more);
    texts.extend(where_x::wordings(ability));
    event_wordings::add(ability, &mut texts);
    graveyard_wordings(ability, &mut texts);
    texts
}

/// The wordings of an ability the card has in its graveyard: it returns itself "from your graveyard to your hand", and
/// a card that goes to a graveyard from the battlefield "is put into a graveyard from the battlefield" as well as dies.
fn graveyard_wordings(ability: &Ability, texts: &mut Vec<String>) {
    match ability {
        Ability::Activated {
            from: ZoneKind::Graveyard,
            ..
        } => {
            for text in texts.iter_mut() {
                *text = text.replace(
                    ": Return ~ to its owner's hand",
                    ": Return ~ from your graveyard to your hand",
                );
            }
        }
        Ability::Triggered {
            when:
                TriggerPattern::ZoneChange {
                    what,
                    from: Some(ZoneKind::Battlefield),
                    to: Some(ZoneKind::Graveyard),
                },
            ..
        } if what.0 == [Pred::This] => {
            let said: Vec<String> = texts
                .iter()
                .filter(|text| text.contains(" ~ dies, "))
                .map(|text| {
                    text.replacen(
                        " ~ dies, ",
                        " ~ is put into a graveyard from the battlefield, ",
                        1,
                    )
                })
                .collect();
            texts.extend(said);
        }
        _ => {}
    }
}

/// The line of a Saga chapter: its roman numeral, a dash and the sentences of its effects.
fn chapter_texts(number: u32, targets: &[TargetSpec], effects: &[Effect]) -> Vec<String> {
    let Some(numeral) = classify::numeral(number) else {
        return Vec::new();
    };
    alternatives(targets, effects)
        .iter()
        .map(|sentences| format!("{numeral} \u{2014} {}", spell(sentences)))
        .collect()
}

/// The ways the event of a triggered ability is worded.
fn head_wordings(head: Vec<String>) -> Vec<Vec<String>> {
    // An Equipment says "equipped creature" and an Aura "enchanted creature"; the data does not say which.
    let mut heads = vec![head.clone()];
    if head[0].contains("equipped creature") {
        heads.push(
            head.iter()
                .map(|part| part.replace("equipped creature", "enchanted creature"))
                .collect(),
        );
    }
    // A card that is targeted once says "When" ("When this creature becomes the target of a spell or ability").
    if head[0].starts_with("Whenever ~ becomes the target of a spell or ability")
        && !head[0].contains(" each turn")
    {
        let mut when = head;
        when[0] = when[0].replacen("Whenever", "When", 1);
        heads.insert(0, when);
    }
    heads
}

fn mana_trigger_texts(ability: &Ability) -> Vec<String> {
    let (
        Ability::Triggered {
            when,
            intervening_if,
            ..
        },
        Some(shape),
    ) = (ability, crate::mana_trigger::mana_trigger(ability))
    else {
        return Vec::new();
    };
    match (
        trigger_head(when, intervening_if.as_ref()),
        super::mana_trigger::text(&shape),
    ) {
        (Some(head), Some(effect)) => vec![[head, vec![format!("{effect}.")]].concat().join(" ")],
        _ => Vec::new(),
    }
}

/// The zone a triggered ability works from, as the wording of its event and effects implies.
fn written_from(when: &TriggerPattern, effects: &[Effect]) -> ZoneKind {
    if cast_itself(when) {
        ZoneKind::Stack
    } else if cycled_itself(when) || graveyard_itself(when) || returns_itself(effects) {
        ZoneKind::Graveyard
    } else {
        ZoneKind::Battlefield
    }
}

fn ability_texts(ability: &Ability) -> Vec<String> {
    match ability {
        Ability::Spell { targets, effects } => alternatives(targets, effects)
            .iter()
            .map(|sentences| spell(sentences))
            .collect(),
        Ability::Triggered {
            when: TriggerPattern::Chapter { number },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => chapter_texts(*number, targets, effects),
        Ability::Triggered {
            when: TriggerPattern::TappedForMana { .. },
            ..
        } if crate::mana_trigger::mana_trigger(ability).is_some() => mana_trigger_texts(ability),
        Ability::Triggered {
            when,
            intervening_if,
            limit: limit @ (None | Some(Limit::OncePerTurn)),
            from,
            targets,
            effects,
        } if *from == written_from(when, effects) => {
            let Some(head) = trigger_head(when, intervening_if.as_ref()) else {
                return Vec::new();
            };
            let heads = head_wordings(head);
            alternatives(targets, effects)
                .iter()
                .flat_map(|sentences| {
                    let tail = triggered_tail(sentences, *limit);
                    heads
                        .iter()
                        .map(move |head| [head.clone(), tail.clone()].concat().join(" "))
                        .collect::<Vec<_>>()
                })
                .flat_map(|text| {
                    // An Equipment copies "equipped creature", an Aura "enchanted creature".
                    let equipped =
                        text.replace("copy of enchanted creature", "copy of equipped creature");
                    let others = (equipped != text).then_some(equipped);
                    std::iter::once(text).chain(others)
                })
                .collect()
        }
        Ability::Activated {
            cost,
            timing,
            condition,
            limit,
            from: from @ (ZoneKind::Battlefield | ZoneKind::Graveyard),
            targets,
            effects,
        } => {
            let Some(head) = costs::text(cost) else {
                return Vec::new();
            };
            // Exiling the card from the battlefield is "Exile ~", from the graveyard "Exile ~ from your graveyard".
            let head = if *from == ZoneKind::Battlefield {
                head.replace("Exile ~ from your graveyard", "Exile ~")
            } else {
                head
            };
            let tails = match condition {
                None => restriction_texts(*timing, *limit),
                // "Activate only if you control a Swamp." stands alone.
                Some(condition) if *timing == Timing::Instant && limit.is_none() => {
                    statics::control_texts(condition)
                        .into_iter()
                        .map(|held| format!("Activate only if {held}."))
                        .collect()
                }
                Some(_) => return Vec::new(),
            };
            alternatives(targets, effects)
                .iter()
                .flat_map(|sentences| {
                    let body = format!("{head}: {}", closed(sentences, true).join(" "));
                    tails.iter().map(move |tail| {
                        if tail.is_empty() {
                            body.clone()
                        } else {
                            format!("{body} {tail}")
                        }
                    })
                })
                .collect()
        }
        Ability::AdditionalCost { cost, required } => costs::additional_text(cost, *required)
            .into_iter()
            .collect(),
        _ => statics::texts(ability),
    }
}

/// The oracle wording of one mode of a modal spell as its bullet reads, with a capital and no
/// closing period; `None` when the readers do not write the mode.
pub fn mode(mode: &Mode) -> Option<String> {
    let sentences = alternatives(&mode.targets, &mode.effects)
        .into_iter()
        .next()?;
    Some(
        sentences
            .iter()
            .map(|sentence| capitalize(sentence))
            .collect::<Vec<_>>()
            .join(". "),
    )
}

pub(super) fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The ways the effects can be written as sentences, without their first capital and period: the
/// effects of one player in a row may be joined into one sentence, in any run of them.
fn alternatives(targets: &[TargetSpec], effects: &[Effect]) -> Vec<Vec<String>> {
    if targets.len() > 1 || effects.is_empty() {
        return Vec::new();
    }
    if let [
        Effect::Modal {
            choose,
            repeat: false,
            modes,
        },
    ] = effects
    {
        if !targets.is_empty() || !choose.is_one() || modes.len() < 2 {
            return Vec::new();
        }
        return modal_texts(modes)
            .into_iter()
            .map(|text| vec![text])
            .collect();
    }
    // Bit `at` of a mask joins the effect after the one at `at` to it; a long list is tried whole
    // and apart only.
    let joints = effects.len() - 1;
    let masks: Vec<u32> = if joints <= 8 {
        (0..1u32 << joints).collect()
    } else {
        vec![0, u32::MAX >> 1]
    };
    let wordings = wording_choices(effects);
    let mut found: Vec<Vec<String>> = masks
        .into_iter()
        .flat_map(|mask| wordings.iter().map(move |wording| (mask, *wording)))
        .filter_map(|(mask, wording)| written(targets, effects, mask, wording))
        .flat_map(|sentences| conditional::wordings(&sentences))
        .collect();
    // A joint that is not closed words the effects the same way with or without a repeated subject.
    found.dedup();
    found
}

/// A way the data can be worded when the same effects read in more than one way.
#[derive(Clone, Copy)]
struct Wording {
    /// Tokens counted by things are worded "a token for each ...".
    per: bool,
    /// Each verb of "you" has its own subject: "you draw a card and you lose 1 life".
    again: bool,
    /// Which of the wordings of a condition is written ("three or more" or "3 or more").
    condition: usize,
}

/// The wordings to try for the effects: tokens counted by things say "for each" or "where X is", and two verbs of
/// "you" in a row share the subject or say it again.
fn wording_choices(effects: &[Effect]) -> Vec<Wording> {
    let counts_tokens = effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::CreateToken {
                count: Amount::Count(_)
                    | Amount::Counters { .. }
                    | Amount::Players(_)
                    | Amount::Times { .. }
                    | Amount::CommanderCasts(_),
                ..
            }
        )
    });
    let repeats = effects.windows(2).any(|pair| {
        pair.iter()
            .all(|effect| player_of(effect) == Some(&Ref::You))
    });
    let pers: &[bool] = if counts_tokens {
        &[false, true]
    } else {
        &[false]
    };
    let agains: &[bool] = if repeats { &[false, true] } else { &[false] };
    let conditions = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::If { condition, .. } => Some(conditional::condition_texts(condition).len()),
            _ => None,
        })
        .max()
        .unwrap_or(1)
        .max(1);
    pers.iter()
        .flat_map(|per| agains.iter().map(move |again| (*per, *again)))
        .flat_map(|(per, again)| {
            (0..conditions).map(move |condition| Wording {
                per,
                again,
                condition,
            })
        })
        .collect()
}

/// The effects of an attack at the enchanted player and the same again for the opponents who attack: one sentence
/// more, which no joint splits.
fn repeating(
    targets: &[TargetSpec],
    head: &[Effect],
    mask: u32,
    wording: Wording,
) -> Option<Vec<String>> {
    if mask >> (head.len() - 1) != 0 {
        return None;
    }
    let mut sentences = written(targets, head, mask, wording)?;
    sentences.push(attackers::sentence(head).to_owned());
    Some(sentences)
}

/// Whether no joint of the mask falls inside the `used` effects from `start`, which are one sentence.
fn unsplit(mask: u32, start: usize, used: usize) -> Option<()> {
    ((mask >> start) & ((1 << used) - 1) == 0).then_some(())
}

/// The sentences of the effects with the joints of the mask closed into compound sentences.
fn written(
    targets: &[TargetSpec],
    effects: &[Effect],
    mask: u32,
    wording: Wording,
) -> Option<Vec<String>> {
    if let Some(head) = attackers::repeated(effects) {
        return repeating(targets, head, mask, wording);
    }
    let mut writer = Writer {
        targets,
        named: false,
        per: wording.per,
        again: wording.again,
        condition: wording.condition,
    };
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0;
    while start < effects.len() {
        // A search and what follows it, or a look at the top cards and the choice from them, are
        // one sentence, which no joint splits.
        if let Some((text, used)) = cast::sentence_text(&effects[start..])
            .or_else(|| library::sequence(&effects[start..]))
            .or_else(|| writer.explore_twice(&effects[start..]))
        {
            unsplit(mask, start, used)?;
            sentences.push(text);
            start += used;
            continue;
        }
        // A look at the top card, the reveal and what follows it are three sentences, which no joint splits.
        if let Some((condition, then, used)) = reveal::shape(&effects[start..]) {
            unsplit(mask, start, used)?;
            let tail = writer.consequence(condition, then)?;
            sentences.push(reveal::text(condition, &tail)?);
            start += used;
            continue;
        }
        // An exile and the return of what was exiled are one sentence, which no joint splits.
        if let Some((text, used)) = flicker::text(&effects[start..]) {
            unsplit(mask, start, used)?;
            sentences.push(text);
            start += used;
            continue;
        }
        // Populate is two steps and one word; a copy that is exiled or sacrificed at the end step is two sentences.
        // No joint splits either.
        if copies::is_populate(&effects[start..]) {
            unsplit(mask, start, 2)?;
            sentences.push("populate".to_owned());
            start += 2;
            continue;
        }
        if let [created @ Effect::CreateToken { .. }, delayed, ..] = &effects[start..]
            && let Some(delay) = copies::delay_text(created, delayed)
        {
            unsplit(mask, start, 2)?;
            sentences.push(writer.copy_token(created, true)?);
            sentences.push(delay);
            start += 2;
            continue;
        }
        // A permanent chosen and the target that becomes a copy of it are two sentences, which no joint splits.
        if let [
            choose @ Effect::Choose { .. },
            copy @ Effect::BecomeCopy { .. },
            ..,
        ] = &effects[start..]
            && let Some(texts) = writer.become_copy(choose, copy)
        {
            unsplit(mask, start, 2)?;
            sentences.extend(texts);
            start += 2;
            continue;
        }
        // A choice of cards and the move of what was chosen are one sentence, which no joint splits.
        if let [
            choose @ Effect::Choose { .. },
            moved @ Effect::Move { .. },
            ..,
        ] = &effects[start..]
            && let Some(text) =
                put_back_text(choose, moved).or_else(|| returned_text(choose, moved))
        {
            unsplit(mask, start, 2)?;
            sentences.push(text);
            start += 2;
            continue;
        }
        let mut end = start + 1;
        while end < effects.len() && mask & (1 << (end - 1)) != 0 {
            end += 1;
        }
        match &effects[start..end] {
            [effect] => {
                sentences.push(writer.effect(effect)?);
                if let Effect::Destroy {
                    what,
                    regenerate: false,
                } = effect
                {
                    let pronoun = if writer.single(what) { "it" } else { "they" };
                    sentences.push(format!("{pronoun} can't be regenerated"));
                }
            }
            group => sentences.push(
                writer
                    .damage_group(group)
                    .or_else(|| writer.compound(group))?,
            ),
        }
        start = end;
    }
    (targets.is_empty() || writer.named).then_some(sentences)
}

/// "choose one —" and a bullet line for each mode, in every wording of each mode; the last bullet
/// has no period, which the sentence that holds the text adds.
fn modal_texts(modes: &[Mode]) -> Vec<String> {
    let mut texts = vec![format!("choose one {DASH}")];
    for (at, mode) in modes.iter().enumerate() {
        let bullets: Vec<String> = alternatives(&mode.targets, &mode.effects)
            .iter()
            .map(|sentences| {
                let capitalized: Vec<String> = sentences.iter().map(|s| capitalize(s)).collect();
                capitalized.join(". ")
            })
            .collect();
        let stop = if at + 1 == modes.len() { "" } else { "." };
        texts = texts
            .iter()
            .flat_map(|so_far| {
                bullets
                    .iter()
                    .map(move |bullet| format!("{so_far}\n{BULLET} {bullet}{stop}"))
            })
            .take(64)
            .collect();
    }
    texts
}

/// "put two cards from your hand on top of your library in any order": the choice of cards of the
/// hand, bound as `back`, and the move of them.
fn put_back_text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to:
                Destination::Library {
                    place: Place::Top,
                    order,
                },
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != BACK || name != BACK || from.0 != [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]
    {
        return None;
    }
    let Some(Amount::Fixed(most)) = &count.max else {
        return None;
    };
    let number = u32::try_from(*most)
        .ok()
        .filter(|most| *most == count.min)?;
    let tail = match (number, order) {
        (1, None) => String::new(),
        (2.., Some(Order::Chosen)) => " in any order".to_owned(),
        _ => return None,
    };
    let cards = if number == 1 {
        "a card".to_owned()
    } else {
        format!("{} cards", count_word(number)?)
    };
    Some(format!(
        "put {cards} from your hand on top of your library{tail}"
    ))
}

/// A choice of one permanent the player controls, bound as `returned`, and the move of it to its
/// owner's hand: "return a land you control to its owner's hand".
fn returned_text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to: Destination::Hand,
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != RETURNED || name != RETURNED || !count.is_one() {
        return None;
    }
    let phrase = phrase_text(from, Form::Subject)?;
    Some(format!(
        "return {} {phrase} to its owner's hand",
        article(&phrase)
    ))
}

/// The sentences with their capitals and periods; the first of a triggered ability follows the
/// comma of its event and keeps its lowercase.
fn closed(sentences: &[String], capital_first: bool) -> Vec<String> {
    sentences
        .iter()
        .enumerate()
        .map(|(at, sentence)| {
            if at == 0 && !capital_first {
                format!("{sentence}.")
            } else {
                format!("{}.", capitalize(sentence))
            }
        })
        .collect()
}

/// The sentences after the event of a triggered ability, and the one about how often it triggers.
fn triggered_tail(sentences: &[String], limit: Option<Limit>) -> Vec<String> {
    let mut tail = closed(sentences, false);
    tail.extend(limit.map(|_| "This ability triggers only once each turn.".to_owned()));
    tail
}

/// The sentences of a spell.
fn spell(sentences: &[String]) -> String {
    closed(sentences, true).join(" ")
}

/// The event and an intervening "if" of a triggered ability, each ending in a comma.
fn trigger_head(when: &TriggerPattern, condition: Option<&Condition>) -> Option<Vec<String>> {
    if condition == Some(&super::triggers::saddled())
        && matches!(when, TriggerPattern::Attacks { .. })
    {
        return Some(vec![format!("{} while saddled,", pattern_text(when)?)]);
    }
    let mut parts = vec![format!("{},", pattern_text(when)?)];
    if let Some(condition) = condition {
        parts.push(format!("if {},", condition_text(condition)?));
    }
    Some(parts)
}

/// The player a verb of the compound sentences is about.
fn player_of(effect: &Effect) -> Option<&Ref> {
    match effect {
        Effect::Draw { who, .. }
        | Effect::Mill { who, .. }
        | Effect::Discard { who, .. }
        | Effect::GainLife { who, .. }
        | Effect::LoseLife { who, .. }
        | Effect::Shuffle { who }
        | Effect::Sacrifice { who, .. }
        | Effect::CreateToken { who, .. } => Some(who),
        _ => None,
    }
}

/// A scry, draw, mill or discard of "you" as a verb with "you" understood: "draw a card", "discard your hand".
fn your_cards(effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::Scry {
            who: Ref::You,
            count,
        } => format!("scry {}", points_text(count)?),
        Effect::Draw {
            who: Ref::You,
            count,
        } => format!("draw {}", cards_text(count)?),
        Effect::Mill {
            who: Ref::You,
            count,
        } => format!("mill {}", cards_text(count)?),
        Effect::Discard {
            who: Ref::You,
            count,
            random,
        } => format!(
            "discard {}{}",
            draws::discarded_text(&Ref::You, count)?,
            at_random(*random)
        ),
        _ => return None,
    })
}

/// "a", "a and b", "a, b, and c".
fn join_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// "a creature or planeswalker", "an artifact": what a player sacrifices.
fn sacrifice_text(filter: &Selector) -> Option<String> {
    let text = phrase_text(filter, Form::Subject)?;
    Some(format!("{} {text}", article(&text)))
}

/// Whether the effect puts energy counters on you, which a card writes "you get {E}{E}".
fn gets_energy(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::PutCounters { on: Ref::You, counter, .. }
            if *counter == CounterKind::Other("energy".to_owned())
    )
}

/// Whether the effect is written as a verb with "you" understood ("draw a card"), which "you may" can precede.
fn is_imperative(effect: &Effect) -> bool {
    !gets_energy(effect)
        && matches!(
            effect,
            Effect::Scry { who: Ref::You, .. }
                | Effect::Surveil { who: Ref::You, .. }
                | Effect::Proliferate { who: Ref::You }
                | Effect::Draw { who: Ref::You, .. }
                | Effect::Mill { who: Ref::You, .. }
                | Effect::Discard { who: Ref::You, .. }
                | Effect::Shuffle { who: Ref::You }
                | Effect::Destroy {
                    regenerate: true,
                    ..
                }
                | Effect::Exile { .. }
                | Effect::Tap { .. }
                | Effect::Untap { .. }
                | Effect::Goad { .. }
                | Effect::Regenerate { .. }
                | Effect::Counter { .. }
                | Effect::Move { .. }
                | Effect::CreateToken { who: Ref::You, .. }
                | Effect::PutCounters { .. }
                | Effect::RemoveCounters { .. }
                | Effect::MoveCounters { .. }
                | Effect::Transform { .. }
                | Effect::Flip { .. }
                | Effect::CopySpell { .. }
        )
}

/// "{G}{G}", "one mana of any color": the mana an effect adds.
/// "add {C}{C}" and, when the mana is restricted, the sentence that says what it may be spent on.
fn add_text(mana: &Produce, times: &Amount, only: &[SpendOn]) -> Option<String> {
    let mut added = format!("add {}", mana_text(mana)?);
    // "Add {R} for each Goblin you control.": once for each thing counted, never a number of times.
    if !times.is_one() {
        let (Amount::Fixed(1), tail) = counted_parts(times)? else {
            return None;
        };
        added = format!("{added} {tail}");
    }
    if only.is_empty() {
        Some(added)
    } else {
        Some(format!("{added}. {}", spend::text(only)?))
    }
}

/// "{W}{U}": the symbols of mana added together.
pub(super) fn symbols_text(list: &[mtg_core::mana::ManaType]) -> String {
    let mut symbols = String::new();
    for mana in list {
        symbols.push('{');
        symbols.push(mana.symbol());
        symbols.push('}');
    }
    symbols
}

/// "two mana in any combination of colors", "three mana in any combination of {U}, {B}, and/or {R}".
pub(super) fn combination_text(
    count: &Amount,
    among: &[mtg_core::mana::ManaType],
) -> Option<String> {
    let number = match count {
        Amount::X => "X",
        Amount::Fixed(number) if *number >= 2 => count_word(u32::try_from(*number).ok()?)?,
        _ => return None,
    };
    let symbols: Vec<String> = among
        .iter()
        .map(|mana| format!("{{{}}}", mana.symbol()))
        .collect();
    let types = match symbols.as_slice() {
        [] => "colors".to_owned(),
        [_] => return None,
        [first, second] => format!("{first} and/or {second}"),
        [init @ .., last] => format!("{}, and/or {last}", init.join(", ")),
    };
    Some(format!("{number} mana in any combination of {types}"))
}

fn mana_text(mana: &Produce) -> Option<String> {
    Some(match mana {
        Produce::Fixed(list) if !list.is_empty() => symbols_text(list),
        Produce::AnyColor(1) => "one mana of any color".to_owned(),
        Produce::AnyColor(count @ 2..) => {
            format!("{} mana of any one color", count_word(*count)?)
        }
        Produce::AnyCombination { count, among } => combination_text(count, among)?,
        Produce::ChosenColor(1) => "one mana of the chosen color".to_owned(),
        Produce::OrChosenColor(mana) => {
            format!("{{{}}} or one mana of the chosen color", mana.symbol())
        }
        Produce::CommanderIdentity(1) => {
            "one mana of any color in your commander's color identity".to_owned()
        }
        Produce::ProducedBy {
            among,
            count: 1,
            colorless,
        } => {
            let kind = if *colorless { "type" } else { "color" };
            let who = match among.0.as_slice() {
                [
                    Pred::Type(mtg_core::card::CardType::Land),
                    Pred::ControlledBy(Ref::Opponents),
                ] => "an opponent controls",
                [
                    Pred::Type(mtg_core::card::CardType::Land),
                    Pred::ControlledBy(Ref::You),
                ] => "you control",
                _ => return None,
            };
            format!("one mana of any {kind} that a land {who} could produce")
        }
        Produce::OneOf(list) if list.len() > 1 => {
            let symbols: Vec<String> = list
                .iter()
                .map(|mana| format!("{{{}}}", mana.symbol()))
                .collect();
            match symbols.as_slice() {
                [first, second] => format!("{first} or {second}"),
                [init @ .., final_one] => format!("{}, or {final_one}", init.join(", ")),
                [] => return None,
            }
        }
        Produce::OneOfLists(lists) if lists.len() > 1 => {
            let texts: Vec<String> = lists.iter().map(|list| symbols_text(list)).collect();
            match texts.as_slice() {
                [first, second] => format!("{first} or {second}"),
                [init @ .., final_one] => format!("{}, or {final_one}", init.join(", ")),
                [] => return None,
            }
        }
        _ => return None,
    })
}

/// The sentence that restricts when an ability may be activated, in every wording of it; empty
/// text for none.
fn restriction_texts(timing: Timing, limit: Option<Limit>) -> Vec<String> {
    let words = |text: &str| vec![text.to_owned()];
    match (timing, limit) {
        (Timing::Instant, None) => words(""),
        (Timing::Sorcery, None) => words("Activate only as a sorcery."),
        (Timing::Instant, Some(Limit::OncePerTurn)) => words("Activate only once each turn."),
        (Timing::Sorcery, Some(Limit::OncePerTurn)) => vec![
            "Activate only as a sorcery and only once each turn.".to_owned(),
            "Activate only once each turn and only as a sorcery.".to_owned(),
        ],
    }
}

/// "+1/+1" and the words that count it, " for each creature you control" (empty for a fixed change): a change of power
/// and toughness. One side may be counted when the other is zero.
pub(super) fn stats_text(power: &Amount, toughness: &Amount) -> Option<(String, String)> {
    let part = |amount: &Amount| match counted_parts(amount) {
        Some((each, tail)) => Some((signed_text(&each)?, Some(tail))),
        None => Some((signed_text(amount)?, None)),
    };
    let ((power, power_tail), (toughness, toughness_tail)) = (part(power)?, part(toughness)?);
    let tail = match (power_tail, toughness_tail) {
        (None, None) => String::new(),
        (Some(tail), None) if toughness == "+0" => format!(" {tail}"),
        (None, Some(tail)) if power == "+0" => format!(" {tail}"),
        (Some(tail), Some(other)) if tail == other => format!(" {tail}"),
        _ => return None,
    };
    Some((format!("{power}/{toughness}"), tail))
}

/// Whether the change of power and toughness leading the modifications is counted: "for each ...", X with the count
/// named after it, or a count the card says as it is.
pub(super) fn counts_stats(mods: &[Modification]) -> bool {
    let counted = |amount: &Amount| {
        counted_parts(amount).is_some()
            || *amount == Amount::X
            || defining::value_text(amount).is_some()
    };
    matches!(
        mods.first(),
        Some(Modification::ModifyPT { power, toughness }) if counted(power) || counted(toughness)
    )
}

/// "+2", "-3", "+X": a change of power or toughness.
pub(super) fn signed_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(number) if *number >= 0 => Some(format!("+{number}")),
        Amount::Fixed(number) => Some(number.to_string()),
        Amount::X => Some("+X".to_owned()),
        _ => None,
    }
}

/// Whether the predicate names card types and nothing else.
pub(super) fn is_a_type(pred: &Pred) -> bool {
    match pred {
        Pred::Type(_) => true,
        Pred::AnyOf(types) => types.iter().all(is_a_type),
        _ => false,
    }
}

fn at_random(random: bool) -> &'static str {
    if random { " at random" } else { "" }
}

/// Writes effects in order, remembering whether the clause's target has been named yet.
struct Writer<'a> {
    targets: &'a [TargetSpec],
    named: bool,
    /// Tokens counted by things are worded "a token for each ..." and not "X tokens, where X is the number of ...".
    per: bool,
    /// The verbs of "you" in one compound sentence each say "you": "you draw a card and you lose 1 life".
    again: bool,
    /// Which of the wordings of a condition is written.
    condition: usize,
}

impl Writer<'_> {
    /// The target, named in full the first time and never again.
    fn name_target(&mut self) -> Option<String> {
        if self.named {
            return None;
        }
        self.named = true;
        spec_text(self.targets.first()?)
    }

    /// Whether the reference is the one target and it is a single object.
    fn single(&self, what: &Ref) -> bool {
        matches!(what, Ref::Target(_))
            && self
                .targets
                .first()
                .is_some_and(|spec| spec.bounds.is_one())
    }

    /// Whether the target is of this kind.
    fn target_is(&self, wanted: impl Fn(&Targetable) -> bool) -> bool {
        self.targets.first().is_some_and(|spec| wanted(&spec.what))
    }

    /// Whether the reference is the one target and it is a card in the controller's graveyard.
    fn in_your_graveyard(&self, what: &Ref) -> bool {
        *what == Ref::Target(0)
            && self.target_is(|kind| {
                matches!(kind, Targetable::Object(selector)
                    if selector.0.contains(&Pred::In(ZoneKind::Graveyard))
                        && selector.0.contains(&Pred::OwnedBy(Ref::You)))
            })
    }

    /// Whether the reference is the one target and it is a permanent: not a spell, not a card in a zone.
    fn is_a_permanent(&self, what: &Ref) -> bool {
        self.single(what)
            && self.target_is(|kind| {
                matches!(kind, Targetable::Object(selector)
                    if !selector.0.iter().any(|pred| matches!(pred, Pred::In(_) | Pred::Spell)))
            })
    }

    /// The object counters are put on, taken from or moved: "~", one target, or (with `several`) "each
    /// creature you control".
    fn counter_place(&mut self, place: &Ref, several: bool) -> Option<String> {
        match place {
            Ref::This => Some("~".to_owned()),
            // The creature that attacks enchanted player.
            Ref::Triggering => Some("it".to_owned()),
            Ref::Target(0)
                if self.single(place)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                self.name_target()
            }
            Ref::Each(selector) if several => {
                Some(format!("each {}", phrase_text(selector, Form::Each)?))
            }
            _ => None,
        }
    }

    /// "put a +1/+1 counter on ~", "remove two charge counters from target permanent", "move a
    /// +1/+1 counter from target creature onto ~".
    fn counters(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::PutCounters { on, counter, count } => {
                // "put a +1/+1 counter on target creature for each Elf you control"
                let (each, tail) = match counted_parts(count) {
                    Some((each, tail)) => (each, format!(" {tail}")),
                    None => (count.clone(), String::new()),
                };
                let counters = counters::text(counter, &each)?;
                format!("put {counters} on {}{tail}", self.counter_place(on, true)?)
            }
            Effect::RemoveCounters {
                from,
                counter,
                count,
            } => {
                let counters = counters::text(counter, count)?;
                format!(
                    "remove {counters} from {}",
                    self.counter_place(from, false)?
                )
            }
            Effect::MoveCounters {
                from,
                to,
                counter,
                count,
            } => {
                let counters = counters::text(counter, count)?;
                let from = self.counter_place(from, false)?;
                format!(
                    "move {counters} from {from} onto {}",
                    self.counter_place(to, false)?
                )
            }
            _ => return None,
        })
    }

    /// An effect on a library: "shuffle your library", "look at the top card of your library",
    /// "put target card from your graveyard on the bottom of your library".
    fn library(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::Shuffle { who: Ref::You } => "shuffle your library".to_owned(),
            Effect::LookAtTop {
                who: Ref::You,
                count,
                bind,
            } => library::look_text(count, bind)?,
            Effect::Move {
                what,
                to: Destination::Library { place, order: None },
            } => {
                let whose = if self.in_your_graveyard(what) {
                    "your"
                } else if self.is_a_permanent(what) {
                    "its owner's"
                } else {
                    return None;
                };
                format!(
                    "put {} {} {whose} library",
                    self.object(what)?,
                    library::place_text(*place)?
                )
            }
            _ => return None,
        })
    }

    /// "return target card from your graveyard to your hand", "return target creature to its owner's hand".
    fn returned_to_hand(&mut self, what: &Ref) -> Option<String> {
        if *what == Ref::This || *what == dead_card() {
            Some("return ~ to its owner's hand".to_owned())
        } else if self.in_your_graveyard(what) {
            Some(format!("return {} to your hand", self.object(what)?))
        } else if self.is_a_permanent(what) {
            Some(format!("return {} to its owner's hand", self.object(what)?))
        } else {
            None
        }
    }

    /// A token that is a copy: "create a token that's a copy of target creature you control", of "it" once the target
    /// has been named; `delayed` says a later sentence acts on the copy.
    fn copy_token(&mut self, effect: &Effect, delayed: bool) -> Option<String> {
        let Effect::CreateToken {
            token: TokenRef::CopyOf { of, .. },
            ..
        } = effect
        else {
            return None;
        };
        let copied = match of {
            Ref::This => "~".to_owned(),
            Ref::Attached => "enchanted creature".to_owned(),
            Ref::Target(0) if self.is_a_permanent(of) => {
                if self.named {
                    "it".to_owned()
                } else {
                    self.name_target()?
                }
            }
            _ => return None,
        };
        copies::token_text(effect, &copied, delayed)
    }

    /// A copy of a spell or an ability, or the card that becomes a copy of the target, naming the one target the first
    /// time.
    fn copy_effect(&mut self, effect: &Effect) -> Option<String> {
        if let Effect::BecomeCopy { .. } = effect {
            if !self.is_a_permanent(&Ref::Target(0)) {
                return None;
            }
            let target = self.name_target()?;
            return copy_spells::becomes_text(effect, &target);
        }
        // The spell a trigger is about has no target to name.
        if let Effect::CopySpell {
            what: Ref::Triggering | Ref::This,
            ..
        } = effect
        {
            return copy_spells::cast_text(effect);
        }
        if self.named {
            return None;
        }
        let text = copy_spells::copy_text(effect, self.targets.first()?)?;
        self.named = true;
        Some(text)
    }

    /// The two sentences of a permanent chosen and a target that becomes a copy of it.
    fn become_copy(&mut self, choose: &Effect, copy: &Effect) -> Option<[String; 2]> {
        if !matches!(
            copy,
            Effect::BecomeCopy {
                what: Ref::Target(0),
                ..
            }
        ) || !self.is_a_permanent(&Ref::Target(0))
        {
            return None;
        }
        let target = self.name_target()?;
        copy_spells::become_texts(choose, copy, &target)
    }

    /// An effect as a sentence without its first capital and its period.
    fn effect(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::CopySpell { .. } | Effect::CopyAbility { .. } | Effect::BecomeCopy { .. } => {
                self.copy_effect(effect)?
            }
            Effect::Scry { who: Ref::You, .. }
            | Effect::Draw { who: Ref::You, .. }
            | Effect::Mill { who: Ref::You, .. }
            | Effect::Discard { who: Ref::You, .. } => your_cards(effect)?,
            Effect::Shuffle { who: Ref::You }
            | Effect::LookAtTop { .. }
            | Effect::Move {
                to: Destination::Library { .. },
                ..
            } => self
                .library(effect)
                .or_else(|| cast::text(effect))
                .or_else(|| self.keyword_action(effect))?,
            Effect::Sacrifice {
                who: Ref::You,
                filter,
                count,
            } if filter.0 == [Pred::This] && count.is_one() => "sacrifice ~".to_owned(),
            Effect::Draw { who, .. }
            | Effect::Mill { who, .. }
            | Effect::Discard { who, .. }
            | Effect::GainLife { who, .. }
            | Effect::LoseLife { who, .. }
            | Effect::Shuffle { who }
            | Effect::Sacrifice { who, .. } => {
                let subject = self.player(who)?;
                format!("{subject} {}", self.predicate(effect, *who != Ref::You)?)
            }
            Effect::Move {
                what,
                to: Destination::Hand,
            } => self
                .returned_to_hand(what)
                .or_else(|| cast::text(effect))
                .or_else(|| self.keyword_action(effect))?,
            Effect::AddMana {
                who: Ref::You,
                mana,
                times,
                only,
            } => add_text(mana, times, only)?,
            // "you get {E}{E}": energy is a counter on the player, written with its symbol.
            Effect::PutCounters { count, .. } if gets_energy(effect) => {
                format!("you get {}", costs::energy_text(count)?)
            }
            Effect::PutCounters { .. }
            | Effect::RemoveCounters { .. }
            | Effect::MoveCounters { .. } => self.counters(effect)?,
            Effect::CreateToken { .. } => self.created(effect)?,
            Effect::DealDamage {
                amount: Amount::Divided,
                ..
            } => self.divided(effect)?,
            Effect::DealDamage {
                source: Some(_), ..
            } => self.damage_by(effect)?,
            Effect::DealDamage {
                source: None,
                to,
                amount,
            } => format!(
                "~ deals {} damage to {}",
                points_text(amount)?,
                self.recipient(to)?
            ),
            Effect::Destroy { what, .. } => format!("destroy {}", self.object(what)?),
            Effect::Tap { what } => format!("tap {}", self.object(what)?),
            Effect::Untap { what } => format!("untap {}", self.object(what)?),
            Effect::Goad { what } if self.named && self.single(what) => "goad it".to_owned(),
            Effect::Goad { what } => format!("goad {}", self.object(what)?),
            Effect::Regenerate { what } => format!("regenerate {}", self.object(what)?),
            Effect::Counter { what } => format!("counter {}", self.object(what)?),
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::UntilEndOfTurn,
            } => self.pump(affected, mods)?,
            Effect::May {
                who: Ref::You,
                effects,
            } => match effects.as_slice() {
                [inner] if is_imperative(inner) => format!("you may {}", self.effect(inner)?),
                _ => self.may_have(effects)?,
            },
            Effect::Pay {
                who,
                cost,
                times,
                paid,
                unpaid,
            } if times.is_one() => self.payment(who, cost, paid, unpaid)?,
            _ => return self.other_effect(effect),
        })
    }

    /// An exile with no binding: "exile target creature", and "... until ~ leaves the battlefield" with the duration.
    fn exiled(&mut self, effect: &Effect) -> Option<String> {
        match effect {
            Effect::Exile {
                what,
                bind: None,
                duration: None,
            } => Some(format!("exile {}", self.object(what)?)),
            Effect::Exile {
                what,
                bind: None,
                duration: Some(Duration::WhileSourceOnBattlefield),
            } => Some(format!(
                "exile {} until ~ leaves the battlefield",
                self.object(what)?
            )),
            Effect::Exile {
                what,
                bind: None,
                duration: Some(Duration::UntilOpponentBecomesMonarch),
            } => Some(format!(
                "exile {} until an opponent becomes the monarch",
                self.object(what)?
            )),
            _ => None,
        }
    }

    /// The effects with a wording of their own: a card returned to the battlefield, the casting and the keyword actions.
    fn other_effect(&mut self, effect: &Effect) -> Option<String> {
        self.exiled(effect)
            .or_else(|| self.graveyard_exile(effect))
            .or_else(|| self.reanimated(effect))
            .or_else(|| cast::text(effect))
            .or_else(|| self.keyword_action(effect))
            .or_else(|| self.controller_search(effect))
    }

    /// A search another player may make: "its controller may search their library for a basic land card, put it onto
    /// the battlefield, then shuffle".
    fn controller_search(&mut self, effect: &Effect) -> Option<String> {
        let Effect::May { who, effects } = effect else {
            return None;
        };
        let owner = self.player(who)?;
        library::may_search_text(&owner, who, effects)
    }

    /// A card of your graveyard that returns to the battlefield: "return target creature card from your graveyard to the
    /// battlefield".
    fn reanimated(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Move {
            what,
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters,
                },
        } = effect
        else {
            return None;
        };
        (counters.is_empty() && self.in_your_graveyard(what)).then(|| {
            self.object(what)
                .map(|card| format!("return {card} to the battlefield"))
        })?
    }

    /// The effects after a condition, as steps joined by "and": "remove all of them and transform ~".
    fn consequence(&mut self, condition: &Condition, then: &[Effect]) -> Option<String> {
        let mut steps = Vec::with_capacity(then.len());
        for effect in then {
            steps.push(match conditional::remove_all_text(condition, effect) {
                Some(text) => text,
                None => self.effect(effect)?,
            });
        }
        (!steps.is_empty()).then(|| join_list(&steps))
    }

    /// "surveil 2", "proliferate", "transform ~", "you become the monarch", "return ~ from your graveyard to the
    /// battlefield", "if there are five or more charge counters on ~, transform ~": a keyword action, a designation,
    /// the card returning itself or an effect on a condition.
    fn keyword_action(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::WinGame { who } => format!(
                "{} {} the game",
                self.player(who)?,
                if *who == Ref::You { "win" } else { "wins" }
            ),
            Effect::Move {
                what: Ref::This,
                to:
                    Destination::Battlefield {
                        tapped,
                        under: None,
                        transformed: false,
                        counters,
                    },
            } if counters.is_empty() => format!(
                "return ~ from your graveyard to the battlefield{}",
                if *tapped { " tapped" } else { "" }
            ),
            Effect::Move {
                what,
                to:
                    Destination::Library {
                        place: Place::Shuffled,
                        order: None,
                    },
            } if *what == Ref::This || *what == dead_card() => {
                "shuffle ~ into its owner's library".to_owned()
            }
            Effect::Surveil {
                who: Ref::You,
                count,
            } => format!("surveil {}", points_text(count)?),
            Effect::Proliferate { who: Ref::You } => "proliferate".to_owned(),
            Effect::Learn { who: Ref::You } => "learn".to_owned(),
            Effect::Monstrosity { count } => format!("monstrosity {}", points_text(count)?),
            Effect::Adapt { count } => format!("adapt {}", points_text(count)?),
            Effect::Bolster { count } => format!("bolster {}", points_text(count)?),
            Effect::Amass { subtype, count } => format!(
                "amass {} {}",
                plural_subtype(&subtype.0),
                points_text(count)?
            ),
            Effect::Explore {
                what: what @ (Ref::This | Ref::Target(0)),
            } => format!("{} explores", self.object(what)?),
            Effect::Connive {
                what: what @ (Ref::This | Ref::Target(0)),
            } => format!("{} connives", self.object(what)?),
            Effect::Transform { what } => format!("transform {}", self.object(what)?),
            Effect::Flip { what } => format!("flip {}", self.object(what)?),
            Effect::If {
                condition: Condition::Not(_),
                ..
            } if spent::unless_text(effect).is_some() => spent::unless_text(effect)?,
            Effect::If {
                condition,
                then,
                otherwise,
            } if otherwise.is_empty() => {
                let texts = conditional::condition_texts(condition);
                let text = texts.get(self.condition).or(texts.first())?;
                format!("if {text}, {}", self.consequence(condition, then)?)
            }
            Effect::BecomeMonarch { who } => format!(
                "{} {} the monarch",
                self.player(who)?,
                if *who == Ref::You {
                    "become"
                } else {
                    "becomes"
                }
            ),
            _ => {
                return prevention::effect_text(effect)
                    .or_else(|| lands::effect_text(effect))
                    .or_else(|| super::attack_tax::effect_text(effect));
            }
        })
    }

    /// "counter target spell unless its controller pays {2}", "you may pay {1}. If you do, draw a card".
    fn payment(
        &mut self,
        who: &Ref,
        cost: &Cost,
        paid: &[Effect],
        unpaid: &[Effect],
    ) -> Option<String> {
        match (paid, unpaid) {
            ([], [inner]) => {
                let happens = self.effect(inner)?;
                let payer = self.player(who)?;
                Some(format!(
                    "{happens} {}",
                    payments::unless_text(&payer, who, cost)?
                ))
            }
            ([_, ..], []) if *who == Ref::You => {
                let mut texts = Vec::new();
                for effect in paid {
                    texts.push(self.effect(effect)?);
                }
                payments::may_pay_text(cost, &texts.join(", then "))
            }
            _ => None,
        }
    }

    /// "~ explores, then it explores again": one creature that explores twice is one sentence, which no joint splits.
    fn explore_twice(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Explore { what: first },
            Effect::Explore { what: second },
            ..,
        ] = effects
        else {
            return None;
        };
        if first != second || !matches!(first, Ref::This | Ref::Target(0)) {
            return None;
        }
        let subject = self.object(first)?;
        Some((format!("{subject} explores, then it explores again"), 2))
    }

    /// Tokens created: "create a Treasure token" for you, "target opponent creates a Treasure token" for another player.
    fn created(&mut self, effect: &Effect) -> Option<String> {
        match effect {
            Effect::CreateToken { who, .. } if *who != Ref::You => Some(format!(
                "{} {}",
                self.player(who)?,
                create::text(effect, self.per)?
            )),
            _ => create::text(effect, self.per).or_else(|| self.copy_token(effect, false)),
        }
    }

    /// Who does something: "you", "target player", "each opponent", "its controller".
    fn player(&mut self, who: &Ref) -> Option<String> {
        Some(match who {
            Ref::You => "you".to_owned(),
            Ref::Opponents => "each opponent".to_owned(),
            Ref::EachPlayer => "each player".to_owned(),
            Ref::Target(0)
                if self.target_is(|what| {
                    matches!(what, Targetable::Player | Targetable::Opponent)
                }) =>
            {
                self.name_target()?
            }
            Ref::ControllerOf(inner)
                if **inner == Ref::Target(0)
                    && self.named
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                "its controller".to_owned()
            }
            Ref::Triggering => "that player".to_owned(),
            Ref::ControllerOf(inner) if **inner == Ref::Triggering => {
                "that permanent's controller".to_owned()
            }
            _ => return None,
        })
    }

    /// "3 life", "X life", "life equal to its power": what is gained or lost.
    fn life(&self, amount: &Amount) -> Option<String> {
        Some(match amount {
            Amount::Stat { of, stat } if *of == Ref::Target(0) && self.named => {
                format!("life equal to its {}", stat_name(*stat)?)
            }
            _ => match counted_parts(amount) {
                Some((each, tail)) => format!("{} life {tail}", points_text(&each)?),
                None => format!("{} life", points_text(amount)?),
            },
        })
    }

    /// What a player does, without the subject: "draws a card", "sacrifice a creature"; `third` is
    /// the verb that follows "target opponent" rather than "you".
    fn predicate(&mut self, effect: &Effect, third: bool) -> Option<String> {
        let s = if third { "s" } else { "" };
        Some(match effect {
            Effect::Draw { count, .. } => format!("draw{s} {}", cards_text(count)?),
            Effect::Mill { count, .. } => format!("mill{s} {}", cards_text(count)?),
            Effect::Discard { who, count, random } => {
                let cards = draws::discarded_text(who, count)?;
                format!("discard{s} {cards}{}", at_random(*random))
            }
            Effect::GainLife { amount, .. } => format!("gain{s} {}", self.life(amount)?),
            Effect::LoseLife { amount, .. } => format!("lose{s} {}", self.life(amount)?),
            Effect::Sacrifice { filter, count, .. } if count.is_one() => {
                format!("sacrifice{s} {}", sacrifice_text(filter)?)
            }
            Effect::CreateToken { .. } => create::text(effect, self.per)?,
            Effect::Shuffle { who } => {
                format!(
                    "shuffle{s} {} library",
                    if *who == Ref::You { "your" } else { "their" }
                )
            }
            _ => return None,
        })
    }

    /// Several effects as one sentence, one clause for each run of effects of one player: "target opponent discards a
    /// card and loses 3 life", "you create a Wolf token and that player mills ten cards".
    fn compound(&mut self, group: &[Effect]) -> Option<String> {
        let mut clauses = Vec::new();
        let mut rest = group;
        while let Some(first) = rest.first() {
            let who = player_of(first)?;
            let run = rest
                .iter()
                .take_while(|effect| player_of(effect) == Some(who))
                .count();
            let subject = self.player(who)?;
            let mut phrases = Vec::with_capacity(run);
            for effect in &rest[..run] {
                phrases.push(self.predicate(effect, *who != Ref::You)?);
            }
            if self.again && *who == Ref::You {
                clauses.extend(phrases.iter().map(|phrase| format!("{subject} {phrase}")));
            } else {
                clauses.push(format!("{subject} {}", join_list(&phrases)));
            }
            rest = &rest[run..];
        }
        Some(clauses.join(" and "))
    }

    /// Damage to two recipients that are no targets and the life gained with it as one sentence: "~ deals 2 damage to each
    /// creature and each player", "~ deals 3 damage to any target and you gain 3 life".
    fn damage_group(&mut self, group: &[Effect]) -> Option<String> {
        let (damage, life) = match group.split_last() {
            Some((
                Effect::GainLife {
                    who: Ref::You,
                    amount,
                },
                rest,
            )) => (rest, Some(amount)),
            _ => (group, None),
        };
        let mut dealt = None;
        let mut recipients = Vec::with_capacity(damage.len());
        for effect in damage {
            let Effect::DealDamage {
                source: None,
                to,
                amount,
            } = effect
            else {
                return None;
            };
            if *dealt.get_or_insert(amount) != amount {
                return None;
            }
            recipients.push(to);
        }
        // One group of objects and one of players, as the reader has them.
        let objects = recipients
            .iter()
            .filter(|to| matches!(to, Ref::Each(_)))
            .count();
        if recipients.len() > 2 || (recipients.len() == 2 && objects != 1) {
            return None;
        }
        let mut to = Vec::with_capacity(2);
        for recipient in recipients {
            to.push(self.recipient(recipient)?);
        }
        let gained = match life {
            Some(life) => format!(" and you gain {}", self.life(life)?),
            None => String::new(),
        };
        Some(format!(
            "~ deals {} damage to {}{gained}",
            points_text(dealt?)?,
            to.join(" and ")
        ))
    }

    /// The objects a verb acts on: the target in full, or "all creatures".
    fn object(&mut self, what: &Ref) -> Option<String> {
        match what {
            Ref::Target(0) if self.target_is(|what| matches!(what, Targetable::Object(_))) => {
                self.name_target()
            }
            Ref::Each(selector) => Some(format!("all {}", phrase_text(selector, Form::All)?)),
            Ref::This => Some("~".to_owned()),
            _ => None,
        }
    }

    /// Who or what is dealt damage.
    fn recipient(&mut self, to: &Ref) -> Option<String> {
        match to {
            Ref::Target(0) => self.name_target(),
            Ref::Opponents => Some("each opponent".to_owned()),
            Ref::EachPlayer => Some("each player".to_owned()),
            Ref::You => Some("you".to_owned()),
            Ref::Triggering => Some("that player".to_owned()),
            Ref::Each(selector) => Some(format!("each {}", phrase_text(selector, Form::Each)?)),
            _ => None,
        }
    }

    /// "target creature gets +2/+2 and gains flying until end of turn".
    fn pump(&mut self, affected: &Ref, mods: &[Modification]) -> Option<String> {
        if let Some(sentence) = becomes::text(affected, mods) {
            return Some(sentence);
        }
        let (subject, plural) = match affected {
            Ref::This => ("~".to_owned(), false),
            Ref::Triggering => ("it".to_owned(), false),
            Ref::Target(0)
                if self.single(affected)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                (self.name_target()?, false)
            }
            Ref::Each(selector) => {
                let phrase = phrase_text(selector, Form::All)?;
                // "All creatures get", but "Creatures you control get" and "Attacking creatures get".
                let all = selector.0.iter().all(is_a_type);
                (if all { format!("all {phrase}") } else { phrase }, true)
            }
            _ => return None,
        };
        if let [Modification::CantBeBlocked] = mods {
            return Some(format!("{subject} can't be blocked this turn"));
        }
        if !plural && let Some(sentence) = becomes::animate_text(&subject, mods) {
            return Some(sentence);
        }
        let (gets, gains) = if plural {
            ("get", "gain")
        } else {
            ("gets", "gains")
        };
        let (stats, tail, rest) = match mods.split_first() {
            Some((Modification::ModifyPT { power, toughness }, rest)) => {
                let (stats, tail) = stats_text(power, toughness)?;
                (Some(format!("{gets} {stats}")), tail, rest)
            }
            _ => (None, String::new(), mods),
        };
        let mut parts: Vec<String> = stats.into_iter().collect();
        if !rest.is_empty() {
            parts.push(format!("{gains} {}", granted_text(rest)?));
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!(
            "{subject} {} until end of turn{tail}",
            parts.join(" and ")
        ))
    }
}

/// Whether a trigger may say "while you're the monarch" after its event: not one about damage, which can itself pass the
/// designation on, so that the state after it is not the one the event happened in.
pub(super) fn while_held(when: &TriggerPattern) -> bool {
    !matches!(
        when,
        TriggerPattern::DealsDamage { .. } | TriggerPattern::Batch(_) | TriggerPattern::AnyOf(_)
    )
}
