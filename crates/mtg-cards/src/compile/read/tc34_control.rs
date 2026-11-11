//! A player controlled during their next turn (CR 722): "You control target player during that player's next turn.",
//! "you gain control of target opponent during that player's next turn. After that turn, that player takes an extra
//! turn."; and a text change (CR 612): "Change the text of target spell or permanent by replacing all instances of one
//! color word with another[ or one basic land type with another][ until end of turn]." with "The new creature type
//! can't be Wall." after a creature type; and a spell taken: "Gain control of target spell. You may choose new targets
//! for it." The writer's side is `render/tc34_control.rs`.

use super::selectors::target;
use super::tokens::Reader;
use crate::effect::{Effect, NewTargets, ValueKind};
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};

/// "gain control of target spell. You may choose new targets for it." (Aethersnatch, Commandeer): the spell changes
/// controller for good and its new controller may choose new targets.
pub(super) fn tc34_take_spell(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("gain control of")?;
    let what = target(r)?;
    let Targetable::Object(selector) = &r.targets.first()?.what else {
        return None;
    };
    selector.0.contains(&Pred::Spell).then_some(())?;
    r.punct('.').then_some(())?;
    r.need("you may choose new targets for it")?;
    Some(vec![
        Effect::Continuous {
            affected: what.clone(),
            mods: vec![Modification::SetController(Ref::You)],
            duration: Duration::Forever,
        },
        Effect::ChangeTargets {
            what,
            to: NewTargets::Choose,
            onto: None,
        },
    ])
}

/// "target spell or permanent": a spell on the stack or a permanent, as the Forge translator writes it.
pub(super) fn spell_or_permanent() -> Targetable {
    Targetable::Object(Selector(vec![Pred::AnyOf(vec![
        Pred::Spell,
        Pred::In(ZoneKind::Battlefield),
    ])]))
}

/// The name the word replaced is chosen under.
pub(super) const FROM: &str = "from";
/// The name the word written is chosen under.
pub(super) const TO: &str = "to";

/// "you control target player during that player's next turn", or "you gain control of target opponent during that
/// player's next turn. After that turn, that player takes an extra turn".
pub(super) fn tc34_control_player(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you")?;
    let gain = r.words("gain control of");
    if !gain {
        r.need("control")?;
    }
    let who = target(r)?;
    matches!(
        r.targets.first()?.what,
        Targetable::Player | Targetable::Opponent
    )
    .then_some(())?;
    r.need("during that player's next turn")?;
    if gain {
        r.punct('.').then_some(())?;
        r.need("after that turn")?;
        r.punct(',').then_some(())?;
        r.need("that player takes an extra turn")?;
    }
    Some(vec![Effect::ControlPlayer {
        who,
        extra_turn: gain,
    }])
}

/// The kind of word after "one": a color word, a basic land type or a creature type.
fn kind(r: &mut Reader<'_>) -> Option<ValueKind> {
    if r.words("color word") {
        Some(ValueKind::Color)
    } else if r.words("basic land type") {
        Some(ValueKind::BasicLandType)
    } else if r.words("creature type") {
        Some(ValueKind::CreatureType)
    } else {
        None
    }
}

/// "Change the text of <target> by replacing all instances of one <kind> with another[ or one basic land type with
/// another][ until end of turn][. The new creature type can't be <type>]": the two words chosen and the change.
pub(super) fn tc34_text_change(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("change the text of")?;
    let what = if r.targets.is_empty() && r.words("target spell or permanent") {
        r.targets.push(TargetSpec {
            what: spell_or_permanent(),
            bounds: Bounds::default(),
            divide: None,
        });
        Ref::Target(0)
    } else {
        let what = target(r)?;
        matches!(r.targets.first()?.what, Targetable::Object(_)).then_some(())?;
        what
    };
    r.need("by replacing all instances of one")?;
    let mut from = kind(r)?;
    r.need("with another")?;
    if from == ValueKind::Color && r.words("or one basic land type with another") {
        from = ValueKind::ColorOrBasicLandType;
    }
    let duration = if r.words("until end of turn") {
        Duration::UntilEndOfTurn
    } else {
        Duration::Forever
    };
    let mut to = ValueKind::LikeChosen(FROM.to_owned());
    if from == ValueKind::CreatureType
        && let Some(left_out) = r.attempt(|r| {
            r.punct('.').then_some(())?;
            r.need("the new creature type can't be")?;
            let name = r.name()?;
            super::subtypes::is_subtype(name).then(|| name.to_owned())
        })
    {
        to = ValueKind::CreatureTypeExcept(vec![left_out]);
    }
    Some(vec![
        Effect::ChooseValue {
            who: Ref::You,
            kind: from,
            bind: FROM.to_owned(),
        },
        Effect::ChooseValue {
            who: Ref::You,
            kind: to,
            bind: TO.to_owned(),
        },
        Effect::Continuous {
            affected: what,
            mods: vec![Modification::ChangeTextChosen {
                from: FROM.to_owned(),
                to: TO.to_owned(),
            }],
            duration,
        },
    ])
}
