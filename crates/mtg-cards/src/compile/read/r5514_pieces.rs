//! r55-14 sentences read and written whole: "Simultaneously, all phased-out creatures phase in and all creatures with
//! phasing phase out." (`Effect::Rephase`); "An opponent chooses one —" with its bullets, where "that player" is the
//! opponent who chose (`Ref::ModesChooser`); "Until that player's next turn, they gain protection from everything and
//! their life total can't change." and "All nonland permanents they control phase out." after "Choose target opponent."
//! (`Duration::UntilTheirNextTurn`).

use super::amounts::{count_word, number_word};
use super::modal::{BULLET, DASH};
use super::tokens::Reader;
use crate::effect::{Effect, Mode, ValueKind};
use crate::keyword::Keyword;
use crate::modify::{Duration, GameRule};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use crate::value::Amount;
use mtg_core::card::CardType;

/// The sentence of `Effect::Rephase`, without its period.
const REPHASE: &str =
    "simultaneously, all phased-out creatures phase in and all creatures with phasing phase out";

/// The rules of the target player until their next turn, without the period.
const THEIR_TURN: &str = "until that player's next turn, they gain protection from everything and their life total can't change";

/// The phase-out of the target player's nonland permanents, without the period.
const THEIR_PHASE: &str = "all nonland permanents they control phase out";

/// The header of a choice of one mode made by an opponent.
const OPPONENT_CHOOSES: &str = "an opponent chooses one";

/// The name the number of cards the mode's chooser draws is kept under.
const DRAWN: &str = "drawn";

/// Reads a sentence of this module, with its period, into `effects`.
pub(super) fn r5514_sentence(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    if r.attempt(|r| (r.r376_text(REPHASE) && r.sentence_end()).then_some(()))
        .is_some()
    {
        effects.push(Effect::Rephase);
        return Some(());
    }
    if r.attempt(r5514_chosen_player).is_some() {
        return Some(());
    }
    if let Some(found) = r.attempt(r5514_energy_sized) {
        effects.extend(found);
        return Some(());
    }
    if let Some(found) = r.attempt(opponent_modal) {
        effects.extend(found);
        return Some(());
    }
    if one_player_target(&r.targets) {
        if r.attempt(|r| (r.r376_text(THEIR_TURN) && r.sentence_end()).then_some(()))
            .is_some()
        {
            effects.extend(their_turn_rules());
            return Some(());
        }
        if r.attempt(|r| (r.r376_text(THEIR_PHASE) && r.sentence_end()).then_some(()))
            .is_some()
        {
            effects.push(their_phase_out());
            return Some(());
        }
    }
    None
}

/// Whether the targets are one player ("Choose target opponent."): what "that player" and "they" name.
fn one_player_target(targets: &[TargetSpec]) -> bool {
    matches!(
        targets,
        [TargetSpec {
            what: Targetable::Player | Targetable::Opponent,
            bounds,
            divide: None,
        }] if bounds.is_one()
    )
}

/// The two rules of the target player until their next turn, in printed order.
fn their_turn_rules() -> Vec<Effect> {
    vec![
        Effect::Rule {
            rule: GameRule::PlayersHave {
                who: Ref::Target(0),
                keyword: Keyword::Protection(Selector::default()),
            },
            duration: Duration::UntilTheirNextTurn,
        },
        Effect::Rule {
            rule: GameRule::LifeTotalCantChange(Ref::Target(0)),
            duration: Duration::UntilTheirNextTurn,
        },
    ]
}

/// The phase-out of every nonland permanent the target player controls.
fn their_phase_out() -> Effect {
    Effect::PhaseOut {
        what: Ref::Each(Selector(vec![
            Pred::ControlledBy(Ref::Target(0)),
            Pred::Not(Box::new(Pred::Type(CardType::Land))),
        ])),
        duration: None,
        tapped: false,
        held: None,
    }
}

/// "An opponent chooses one —" and its bullets: the mode is chosen by the next opponent as the spell is cast.
fn opponent_modal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(OPPONENT_CHOOSES)?;
    r.punct(DASH).then_some(())?;
    let mut modes = Vec::new();
    while r.punct(BULLET) {
        modes.push(r.attempt(mode_shape).or_else(|| super::modal::mode(r))?);
    }
    (modes.len() >= 2).then(|| {
        vec![Effect::Modal {
            who: Ref::Opponents,
            choose: Bounds::default(),
            repeat: false,
            at_random: false,
            different_players: false,
            once_each_turn: false,
            once_ever: false,
            all_if: None,
            mode_costs: Vec::new(),
            extra_mode_cost: None,
            pawprints: Vec::new(),
            modes,
        }]
    })
}

/// A number word read as a count of at least one.
fn count(r: &mut Reader<'_>) -> Option<u32> {
    let word = r.peek_word()?;
    let n = number_word(word).filter(|n| *n >= 1 && word != "a")?;
    r.word(word).then_some(n)
}

/// A bullet of a choice made by an opponent that the effect templates do not word: "You draw three cards." and "You
/// destroy up to two target creatures that player controls. They can't be regenerated. That player draws up to three
/// cards."
fn mode_shape(r: &mut Reader<'_>) -> Option<Mode> {
    r.need("you")?;
    if r.word("draw") {
        let n = count(r)?;
        r.need(if n == 1 { "card" } else { "cards" })?;
        r.sentence_end().then_some(())?;
        return Some(drawing_mode(n));
    }
    r.need("destroy up to")?;
    let most = count(r)?;
    r.need("target creatures that player controls")?;
    r.sentence_end().then_some(())?;
    r.need("they can't be regenerated")?;
    r.sentence_end().then_some(())?;
    r.need("that player draws up to")?;
    let drawn = count(r)?;
    r.need("cards")?;
    r.sentence_end().then_some(())?;
    Some(destroying_mode(most, drawn))
}

/// "You draw <n> cards."
fn drawing_mode(n: u32) -> Mode {
    Mode {
        label: None,
        targets: Vec::new(),
        effects: vec![Effect::Draw {
            who: Ref::You,
            count: Amount::Fixed(i32::try_from(n).unwrap_or(i32::MAX)),
            bind: None,
        }],
    }
}

/// "You destroy up to <most> target creatures that player controls. They can't be regenerated. That player draws up to
/// <drawn> cards."
fn destroying_mode(most: u32, drawn: u32) -> Mode {
    let fixed = |n: u32| Amount::Fixed(i32::try_from(n).unwrap_or(i32::MAX));
    Mode {
        label: None,
        targets: vec![TargetSpec {
            what: Targetable::Object(Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::ModesChooser),
            ])),
            bounds: Bounds {
                min: Amount::Fixed(0),
                max: Some(fixed(most)),
            },
            divide: None,
        }],
        effects: vec![
            Effect::Destroy {
                what: Ref::Target(0),
                regenerate: false,
                bind: None,
            },
            Effect::ChooseValue {
                who: Ref::ModesChooser,
                kind: ValueKind::Number(Amount::Fixed(0), fixed(drawn)),
                bind: DRAWN.to_owned(),
            },
            Effect::Draw {
                who: Ref::ModesChooser,
                count: Amount::ChosenAs(DRAWN.to_owned()),
                bind: None,
            },
        ],
    }
}

/// The bullet of a mode this module words, without its last period.
fn mode_text(mode: &Mode) -> Option<String> {
    let fixed = |amount: &Amount| match amount {
        Amount::Fixed(n) => u32::try_from(*n).ok().filter(|n| *n >= 1),
        _ => None,
    };
    if let [Effect::Draw { count, .. }] = mode.effects.as_slice() {
        let n = fixed(count)?;
        (*mode == drawing_mode(n)).then_some(())?;
        let noun = if n == 1 { "card" } else { "cards" };
        return Some(format!("You draw {} {noun}", count_word(n)?));
    }
    let [spec] = mode.targets.as_slice() else {
        return None;
    };
    let most = fixed(spec.bounds.max.as_ref()?)?;
    let Some(Effect::ChooseValue {
        kind: ValueKind::Number(_, drawn),
        ..
    }) = mode.effects.get(1)
    else {
        return None;
    };
    let drawn = fixed(drawn)?;
    (*mode == destroying_mode(most, drawn) && most >= 2 && drawn >= 2).then_some(())?;
    Some(format!(
        "You destroy up to {} target creatures that player controls. They can't be regenerated. That player draws up to {} cards",
        count_word(most)?,
        count_word(drawn)?
    ))
}

/// The wording of a choice of one mode made by an opponent, every bullet a shape this module words; the last bullet has
/// no period, which the sentence that holds the text adds.
pub(super) fn r5514_modal_texts(effects: &[Effect]) -> Option<Vec<Vec<String>>> {
    let [
        Effect::Modal {
            who: Ref::Opponents,
            choose,
            repeat: false,
            at_random: false,
            different_players: false,
            once_each_turn: false,
            once_ever: false,
            all_if: None,
            mode_costs,
            extra_mode_cost: None,
            pawprints,
            modes,
        },
    ] = effects
    else {
        return None;
    };
    (choose.is_one() && mode_costs.is_empty() && pawprints.is_empty() && modes.len() >= 2)
        .then_some(())?;
    let mut text = format!("{OPPONENT_CHOOSES} {DASH}");
    for (at, mode) in modes.iter().enumerate() {
        let stop = if at + 1 == modes.len() { "" } else { "." };
        text = format!("{text}\n{BULLET} {}{stop}", mode_text(mode)?);
    }
    Some(vec![vec![text]])
}

/// The words of an effect of this module, without its period.
pub(super) fn r5514_effect_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::Rephase => Some(REPHASE.to_owned()),
        _ => (*effect == their_phase_out()).then(|| THEIR_PHASE.to_owned()),
    }
}

/// The words of the two rules of the target player until their next turn and the effects they take.
pub(super) fn r5514_steps_text(effects: &[Effect]) -> Option<(String, usize)> {
    (effects.get(..2)? == their_turn_rules().as_slice()).then(|| (THEIR_TURN.to_owned(), 2))
}

/// The sentences of a spell that chooses a target player in a sentence of its own and then holds them until their next
/// turn and phases out their nonland permanents ("Choose target opponent. Until that player's next turn, ..."), with
/// the wordings of the steps after them.
pub(super) fn r5514_their_turn_sentences(
    targets: &[TargetSpec],
    effects: &[Effect],
    rest: impl Fn(&[Effect]) -> Vec<Vec<String>>,
) -> Option<Vec<Vec<String>>> {
    one_player_target(targets).then_some(())?;
    let noun = match targets.first()?.what {
        Targetable::Opponent => "opponent",
        _ => "player",
    };
    let (_, taken) = r5514_steps_text(effects)?;
    (effects.get(taken)? == &their_phase_out()).then_some(())?;
    let head = vec![
        format!("choose target {noun}"),
        THEIR_TURN.to_owned(),
        THEIR_PHASE.to_owned(),
    ];
    let after = &effects[taken + 1..];
    if after.is_empty() {
        return Some(vec![head]);
    }
    Some(
        rest(after)
            .into_iter()
            .map(|tail| head.iter().cloned().chain(tail).collect())
            .collect(),
    )
}

/// "Choose target opponent." before the rules of that player until their next turn: the target, with no step.
pub(super) fn r5514_chosen_player(r: &mut Reader<'_>) -> Option<()> {
    r.targets.is_empty().then_some(())?;
    r.need("choose target")?;
    let what = if r.word("opponent") {
        Targetable::Opponent
    } else {
        r.need("player")?;
        Targetable::Player
    };
    r.sentence_end().then_some(())?;
    r.lookahead(|r| r.r376_text(THEIR_TURN).then_some(()))?;
    r.targets.push(TargetSpec {
        what,
        bounds: Bounds::default(),
        divide: None,
    });
    Some(())
}

/// The cost modifier of "Mana abilities of this land cost an additional <n> life to activate."
fn mana_life(life: u32) -> crate::ability::Ability {
    crate::ability::Ability::CostModifier {
        applies_to: Selector::this(),
        change: crate::ability::CostChange::Increase,
        mana: crate::mana::ManaCostX::new(0, mtg_core::mana::ManaCost::ZERO),
        per: Amount::Fixed(1),
        condition: None,
        activating: Some(crate::ability::ActivationKind::Mana),
        targeting: None,
        least: 0,
        life,
    }
}

/// The nouns a land names itself by in "Mana abilities of <noun> cost ...".
const SELF_NOUNS: [&str; 2] = ["this land", "~"];

/// "Mana abilities of this land cost an additional 1 life to activate.": life added to each activation of the source's
/// mana abilities.
pub(super) fn r5514_read(text: &str) -> Option<crate::ability::Ability> {
    let rest = text.strip_prefix("Mana abilities of ")?;
    let rest = SELF_NOUNS
        .iter()
        .find_map(|noun| rest.strip_prefix(noun))?
        .strip_prefix(" cost an additional ")?;
    let (life, tail) = rest.split_once(' ')?;
    (tail == "life to activate.").then_some(())?;
    let life: u32 = life.parse().ok().filter(|life| *life >= 1)?;
    Some(mana_life(life))
}

/// The sentences of an ability of this module read whole.
pub(super) fn r5514_wordings(ability: &crate::ability::Ability, texts: &mut Vec<String>) {
    let crate::ability::Ability::CostModifier { life, .. } = ability else {
        return;
    };
    if *life == 0 || *ability != mana_life(*life) {
        return;
    }
    for noun in SELF_NOUNS {
        let text = format!("Mana abilities of {noun} cost an additional {life} life to activate.");
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The name the target of an energy payment sized by it is kept under for the reflexive trigger after it.
const PAID_FOR: &str = "paid_for";

/// The targets an energy payment may be sized by, as printed after "target", with the noun "that ..." names them by.
const SIZED_BY: [(&str, &str); 1] = [("nonland permanent", "permanent")];

/// The selector of a target an energy payment may be sized by.
fn sized_by(phrase: &str) -> Option<Selector> {
    match phrase {
        "nonland permanent" => Some(Selector(vec![Pred::Not(Box::new(Pred::Type(
            CardType::Land,
        )))])),
        _ => None,
    }
}

/// The steps of "you may pay an amount of {E} equal to target <x>'s mana value. When you do, destroy that <noun>.": the
/// target kept by name, the payment, and the reflexive trigger that destroys what the name holds.
fn energy_sized_steps() -> Vec<Effect> {
    vec![
        Effect::Bind {
            what: Ref::Target(0),
            bind: PAID_FOR.to_owned(),
            adds: false,
        },
        Effect::Pay {
            who: Ref::You,
            cost: crate::cost::Cost(vec![crate::cost::CostItem::PayEnergy(Amount::Stat {
                of: Ref::Target(0),
                stat: crate::value::StatKind::ManaValue,
            })]),
            times: Amount::Fixed(1),
            paid: vec![Effect::Reflexive {
                targets: Vec::new(),
                effects: vec![Effect::Destroy {
                    what: Ref::Bound(PAID_FOR.to_owned()),
                    regenerate: true,
                    bind: None,
                }],
            }],
            unpaid: Vec::new(),
        },
    ]
}

/// Reads "you may pay an amount of {E} equal to target <x>'s mana value. When you do, destroy that <noun>." into its
/// steps, declaring the target.
pub(super) fn r5514_energy_sized(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    let (phrase, noun) = SIZED_BY.iter().find(|(phrase, _)| {
        r.lookahead(|r| {
            r.r376_text(&format!(
                "you may pay an amount of {{E}} equal to target {phrase}'s mana value"
            ))
            .then_some(())
        })
        .is_some()
    })?;
    r.r376_text(&format!(
        "you may pay an amount of {{E}} equal to target {phrase}'s mana value"
    ))
    .then_some(())?;
    r.sentence_end().then_some(())?;
    r.r376_text(&format!("when you do, destroy that {noun}"))
        .then_some(())?;
    r.sentence_end().then_some(())?;
    r.targets.push(TargetSpec {
        what: Targetable::Object(sized_by(phrase)?),
        bounds: Bounds::default(),
        divide: None,
    });
    Some(energy_sized_steps())
}

/// Whether the steps of the ability are an energy payment sized by its target and the reflexive destruction of it.
pub(super) fn r5514_is_energy_sized(ability: &crate::ability::Ability) -> bool {
    ability.effects() == energy_sized_steps().as_slice()
}

/// The sentences of an energy payment sized by the one target, and the reflexive destruction of it.
pub(super) fn r5514_energy_sized_sentences(
    targets: &[TargetSpec],
    effects: &[Effect],
) -> Option<Vec<Vec<String>>> {
    (effects == energy_sized_steps().as_slice()).then_some(())?;
    let [
        TargetSpec {
            what: Targetable::Object(selector),
            bounds,
            divide: None,
        },
    ] = targets
    else {
        return None;
    };
    bounds.is_one().then_some(())?;
    let (phrase, noun) = SIZED_BY
        .iter()
        .find(|(phrase, _)| sized_by(phrase).as_ref() == Some(selector))?;
    Some(vec![vec![
        format!("you may pay an amount of {{E}} equal to target {phrase}'s mana value"),
        format!("when you do, destroy that {noun}"),
    ]])
}
