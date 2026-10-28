//! Abilities: a clause's sentences become a spell ability, an event, a comma and sentences a
//! triggered ability, and a cost, a colon, sentences and the restrictions on when it may be
//! activated an activated ability, and a loyalty cost, a colon and sentences a loyalty ability.

use super::tokens::Reader;
use super::{
    costs, effects, exiled_kind, instead_pairs, mana_trigger, modal, render, restrictions, triggers,
};
use crate::ability::{Ability, Limit, LoyaltyCost};
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// Whether the next sentence is a restriction that ends the effects: "Activate only ..." or "This ability triggers only
/// once each turn."
fn at_restriction(r: &mut Reader<'_>) -> bool {
    r.peek_word() == Some("activate")
        || r.lookahead(|r| {
            r.words(super::restrictions::W6E_NOT_IN_COMBAT)
                .then_some(())
        })
        .is_some()
        || super::c11a_ability_less::c11a_at_less(r)
        || r.lookahead(|r| r.words(ONCE_EACH_TURN).then_some(()))
            .is_some()
}

/// "This ability triggers only once each turn." after the effects of a triggered ability.
fn once_each_turn(r: &mut Reader<'_>) -> Option<Limit> {
    r.attempt(|r| {
        r.words(ONCE_EACH_TURN).then_some(())?;
        r.sentence_end().then_some(Limit::OncePerTurn)
    })
}

const ONCE_EACH_TURN: &str = "this ability triggers only once each turn";

/// The sentences of an ability, in order, every one read; `None` when one is not, or when a `Modal`
/// is not the only effect (the modes keep their own targets, so the ability has none). With
/// `restricted` the sentences end where an "Activate only ..." or a "This ability triggers only once each turn."
/// sentence begins.
fn sentences(r: &mut Reader<'_>, restricted: bool) -> Option<Vec<Effect>> {
    let mut effects = Vec::new();
    while !(r.end() || restricted && at_restriction(r)) {
        let before = effects.len();
        if !effects::sentence(r, &mut effects) {
            return None;
        }
        // ". If C, instead S2." replaces the sentence just read.
        if let Some(replaced) = instead_pairs::tail(r, &effects[before..]) {
            effects.truncate(before);
            effects.push(replaced);
        } else if effects.len() > before {
            // ". If it was a creature card, S." after the exile of a target.
            exiled_kind::tail(r, &mut effects);
        }
    }
    let alone = effects.len() == 1 || !effects.iter().any(|e| matches!(e, Effect::Modal { .. }));
    // A name an effect binds is bound once in an ability.
    let binds: Vec<&str> = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Choose { bind, .. } | Effect::ChooseAtRandom { bind, .. } => {
                Some(bind.as_str())
            }
            _ => None,
        })
        .collect();
    let unique = binds
        .iter()
        .enumerate()
        .all(|(at, name)| !binds[..at].contains(name));
    (!effects.is_empty() && alone && unique).then_some(effects)
}

/// The sentences of an instant or sorcery, in order; every one must be read.
pub fn spell(r: &mut Reader<'_>) -> Option<Ability> {
    let effects = sentences(r, false)?;
    Some(Ability::Spell {
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}

/// The modes of an instant or sorcery: "Choose one —" and its bullets, as the only effect.
pub fn modal_spell(r: &mut Reader<'_>) -> Option<Ability> {
    let effects = modal::modal(r)?;
    r.end().then_some(Ability::Spell {
        targets: Vec::new(),
        effects,
    })
}

/// A triggered ability: an event, a comma, an optional intervening "if" and then sentences, every
/// one of which must be read.
pub fn triggered(r: &mut Reader<'_>) -> Option<Ability> {
    let when = triggers::pattern(r)?;
    r.triggered = true;
    let saddled = r.words("while saddled");
    if saddled && !matches!(when, TriggerPattern::Attacks { .. }) {
        return None;
    }
    r.that_player = about_a_player(&when);
    r.acts_itself = triggers::itself_acts(&when);
    r.that_permanent = matches!(
        when,
        TriggerPattern::Tapped { .. } | TriggerPattern::Untapped { .. }
    );
    if triggers::targeted_itself(&when) {
        r.it = Some(Ref::This);
    }
    note_attacks_and_casts(r, &when);
    // The creature that attacks is "it" and "its controller" is the player attacking.
    if matches!(
        when,
        TriggerPattern::AttacksPlayer { .. } | TriggerPattern::AttacksPlayerOrPlaneswalker { .. }
    ) {
        r.it = Some(Ref::Triggering);
    }
    // The creature of the event is "it" in the effects.
    if r.it.is_none() {
        r.it = triggers::event_creature(&when);
    }
    // The card that dies, or is put into a graveyard, is a new object there, which "it" names in what the card does.
    r.died = triggers::event_creature(&when) == Some(Ref::This)
        && matches!(
            &when,
            TriggerPattern::ZoneChange {
                to: Some(ZoneKind::Graveyard),
                ..
            }
        );
    // A permanent entering is not the card itself, which may wait in the graveyard for it.
    r.from_graveyard = matches!(
        &when,
        TriggerPattern::ZoneChange { what, to: Some(ZoneKind::Battlefield), .. } if what.0 != [Pred::This]
    );
    note_other_creatures(r, &when);
    let held = if render::while_held(&when) {
        r.attempt(|r| {
            r.need("while")?;
            triggers::monarch_condition(r)
        })
    } else {
        None
    };
    r.punct(',').then_some(())?;
    let intervening_if = r.attempt(|r| {
        r.word("if").then_some(())?;
        let condition = triggers::condition(r)?;
        r.punct(',').then_some(condition)
    });
    let intervening_if = match (saddled, intervening_if) {
        (false, None) => held,
        (false, Some(_)) if held.is_some() => return None,
        (false, other) => other,
        (true, None) => Some(triggers::saddled()),
        (true, Some(_)) => return None,
    };
    let monstrous = super::c10_pa_monstrous::c10_pa_x_of(r, &when);
    let effects = match r.attempt(|r| mana_trigger::effects(r, &when)) {
        Some(mana) => mana,
        None => sentences(r, true)?,
    };
    let effects = super::c10_pa_monstrous::c10_pa_put_in(r, effects, monstrous);
    let limit = once_each_turn(r);
    // A card cycled is in its owner's graveyard when its own ability triggers, and a card that returns itself from the
    // graveyard has its ability there.
    let from = if triggers::cast_itself(&when) {
        ZoneKind::Stack
    } else if triggers::cycled_itself(&when)
        || triggers::graveyard_itself(&when)
        || effects::returns_itself(&effects)
    {
        ZoneKind::Graveyard
    } else {
        ZoneKind::Battlefield
    };
    Some(Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        targets: std::mem::take(&mut r.targets),
        effects,
    })
}

/// What the event says about who attacks whom and which spell is cast, which the pronouns of the effects name.
fn note_attacks_and_casts(r: &mut Reader<'_>, when: &TriggerPattern) {
    r.at_monarch = matches!(
        when,
        TriggerPattern::AttacksPlayer {
            at: Ref::Monarch,
            ..
        }
    );
    r.attacked_player = matches!(
        when,
        TriggerPattern::AttacksPlayer {
            at: Ref::Opponents,
            ..
        }
    );
    r.cast_spell = casts_a_spell(when);
    r.cast_itself = triggers::cast_itself(when);
    r.subject = triggers::event_subject(when);
    r.attacker = matches!(when, TriggerPattern::Attacks { what } if *what == Selector::this());
    r.attacked = matches!(
        when,
        TriggerPattern::PlayerAttacked {
            who: Ref::AttachedPlayer
        }
    );
}

/// What the event says about the other creatures it is about: whose permanent entered, which card went to a graveyard,
/// and where a card that returns itself waits.
fn note_other_creatures(r: &mut Reader<'_>, when: &TriggerPattern) {
    r.that_controller = matches!(
        when,
        TriggerPattern::ZoneChange { what, to: Some(ZoneKind::Battlefield), .. } if !what.0.contains(&Pred::This)
    );
    r.others_died = matches!(
        when,
        TriggerPattern::ZoneChange { what, from: Some(ZoneKind::Battlefield), to: Some(ZoneKind::Graveyard) } if !what.0.contains(&Pred::This)
    );
    // An ability that triggers at a step works from the graveyard when its card returns itself.
    r.from_graveyard |= matches!(when, TriggerPattern::StepBegins { .. });
}

/// Whether the event is a spell being cast, in every one of the events joined by "or", also the nth in a turn.
fn casts_a_spell(when: &TriggerPattern) -> bool {
    match when {
        TriggerPattern::AnyOf(events) => events.iter().all(casts_a_spell),
        TriggerPattern::Nth { event, .. } => casts_a_spell(event),
        _ => matches!(
            when,
            TriggerPattern::SpellCast { .. } | TriggerPattern::SpellCopied { .. }
        ),
    }
}

/// Whether the event is about a player ("that player" in the effects is who cast the spell, drew the card, lost the
/// life or was dealt the damage), in every one of the events joined by "or".
fn about_a_player(when: &TriggerPattern) -> bool {
    if let TriggerPattern::AnyOf(events) = when {
        return events.iter().all(about_a_player);
    }
    matches!(
        when,
        TriggerPattern::SpellCast { .. }
            | TriggerPattern::SpellCopied { .. }
            | TriggerPattern::Drew { .. }
            | TriggerPattern::Discarded { .. }
            | TriggerPattern::LifeGained { .. }
            | TriggerPattern::LifeLost { .. }
            | TriggerPattern::BecomesMonarch { .. }
            | TriggerPattern::Proliferated { .. }
            | TriggerPattern::CoinFlipWon { .. }
            | TriggerPattern::DiceRolled { .. }
            | TriggerPattern::Amassed { .. }
            | TriggerPattern::Bolstered { .. }
            | TriggerPattern::Surveilled { .. }
            | TriggerPattern::Cycled { .. }
            | TriggerPattern::TappedForMana { .. }
            | TriggerPattern::StepBegins {
                whose: Ref::EachPlayer | Ref::Opponents | Ref::Monarch,
                ..
            }
            | TriggerPattern::PlayerAttacks { .. }
            | TriggerPattern::DealsDamage {
                to: Some(Ref::EachPlayer | Ref::Opponents),
                ..
            }
    )
}

/// An activated ability: a cost, a colon, sentences and the restrictions on when it may be
/// activated: `{T}: Add {G}.`, `{2}, {T}, Sacrifice ~: Draw a card.`
pub fn activated(r: &mut Reader<'_>) -> Option<Ability> {
    let mut cost = costs::cost(r)?;
    r.punct(':').then_some(())?;
    // An X in the cost is the payer's to choose, so the effects may use it.
    r.x_free |= cost.has_x();
    r.paid |= costs::takes_objects(&cost);
    r.subject = Some(Ref::This);
    // A card that exiles itself from the graveyard or returns itself from there has its ability in the graveyard.
    let in_graveyard = r.from_graveyard || effects::returns_from_graveyard(r);
    r.from_graveyard = in_graveyard;
    let effects = sentences(r, true)?;
    if let Some(less) = r.attempt(super::c11a_ability_less::c11a_less) {
        cost.0.push(less);
    }
    let (timing, limit, condition) = restrictions::read(r)?;
    let targets = std::mem::take(&mut r.targets);
    // An ability without targets that adds mana is a mana ability (CR 605.1a) whatever else it does and plays
    // without the stack; the data holds it whole, and the engine plays the shapes it reads (`mana_shape`).
    Some(Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        from: if in_graveyard {
            ZoneKind::Graveyard
        } else {
            ZoneKind::Battlefield
        },
        targets,
        effects,
    })
}

/// A loyalty cost: `+N`, `−N`, `0` or `−X`.
fn loyalty_cost(r: &mut Reader<'_>) -> Option<LoyaltyCost> {
    if r.punct('+') {
        return r.number().map(LoyaltyCost::Plus);
    }
    if r.punct('\u{2212}') {
        if r.word("x") {
            return Some(LoyaltyCost::MinusX);
        }
        return r.number().map(LoyaltyCost::Minus);
    }
    (r.number()? == 0).then_some(LoyaltyCost::Zero)
}

/// A planeswalker's loyalty ability: its loyalty cost, a colon and sentences, every one read. The X of a `−X` is the
/// number of counters the player removes, which the effects may use.
pub fn loyalty(r: &mut Reader<'_>) -> Option<Ability> {
    let cost = loyalty_cost(r)?;
    r.punct(':').then_some(())?;
    r.x_free |= cost == LoyaltyCost::MinusX;
    r.subject = Some(Ref::This);
    let effects = sentences(r, false)?;
    let targets = std::mem::take(&mut r.targets);
    Some(Ability::Loyalty {
        cost,
        targets,
        effects,
    })
}
