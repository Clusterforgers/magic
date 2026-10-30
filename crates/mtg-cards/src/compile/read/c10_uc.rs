//! Taps that hold permanents down (CR 502.3): "Tap target creature an opponent controls. That creature doesn't untap
//! during its controller's untap step for as long as you control this creature." over one or several targets, "Tap all
//! creatures target player controls. Those creatures don't untap during that player's next untap step.", and "~ enters
//! tapped and doesn't untap during your untap step."

use mtg_core::card::CardType;

use super::effects::object;
use super::selectors::{Form, phrase};
use super::statics::statics;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The tails of the hold that lasts while you control the source, for one target and for several.
pub(super) const C10_UC_CONTROL_TAILS: [&str; 2] = [
    "That creature doesn't untap during its controller's untap step for as long as you control ~",
    "Those creatures don't untap during their controllers' untap steps for as long as you control ~",
];

/// The sentence of a tap of every creature the target player controls, held through that player's next untap step.
pub(super) const C10_UC_SLEEP: &str = "tap all creatures target player controls. Those creatures don't untap during that player's next untap step";

/// The group of "all creatures target player controls".
pub(super) fn c10_uc_players_creatures() -> Ref {
    Ref::Each(Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::Target(0)),
    ]))
}

/// The tap and the rule that keeps the same permanents tapped for `duration`.
fn c10_uc_pair(what: Ref, duration: Duration) -> Vec<Effect> {
    vec![
        Effect::Tap {
            what: what.clone(),
            bind: None,
        },
        Effect::Continuous {
            affected: what,
            mods: vec![Modification::DoesntUntap],
            duration,
        },
    ]
}

/// "tap <targets>. <they> don't untap during <their controllers'> untap step(s) for as long as you control ~": the tap
/// and a rule over the same permanents that lasts while you control the source.
pub(super) fn c10_uc_held_control(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("tap")?;
    let what = object(r)?;
    matches!(what, Ref::Target(0)).then_some(())?;
    r.punct('.').then_some(())?;
    if r.targets.first()?.bounds.is_one() {
        r.need("that creature doesn't untap during its controller's untap step")?;
    } else {
        r.need("those creatures don't untap during their controllers")?;
        r.punct('\'').then_some(())?;
        r.need("untap steps")?;
    }
    r.need("for as long as you control")?;
    r.source().then_some(())?;
    Some(c10_uc_pair(what, Duration::WhileYouControlSource))
}

/// "tap up to X target creatures, where X is <a count>. Those creatures don't untap during their controller's next
/// untap step": the bounds of the targets are the count, and the tap and the hold are over them.
pub(super) fn c10_uc_tap_counted(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need("tap up to")?;
    r.x().then_some(())?;
    r.need("target")?;
    let selector = phrase(r, Form::Targets)?;
    let count = super::where_x::counted(r)?;
    // The sentence's where clause is read: none is left for `where_x::bind` to find.
    r.where_x = false;
    r.punct('.').then_some(())?;
    r.targets.push(TargetSpec {
        what: Targetable::Object(selector),
        bounds: Bounds::up_to(Some(count)),
        divide: None,
    });
    let what = Ref::Target(0);
    r.need("those creatures don't untap during their controller's next untap step")?;
    Some(c10_uc_pair(what, Duration::NextUntapStep))
}

/// "tap all creatures target player controls. Those creatures don't untap during that player's next untap step": the
/// tap of the group and the rule that holds the same creatures through their controller's next untap step.
pub(super) fn c10_uc_sleep(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need("tap all creatures target player controls")?;
    r.punct('.').then_some(())?;
    r.need("those creatures don't untap during that player's next untap step")?;
    r.targets.push(TargetSpec {
        what: Targetable::Player,
        bounds: Bounds::default(),
        divide: None,
    });
    Some(c10_uc_pair(
        c10_uc_players_creatures(),
        Duration::NextUntapStep,
    ))
}

/// The sentence of a card that enters tapped and stays tapped through your untap steps.
const C10_UC_STAYS: &str = "~ enters tapped and doesn't untap during your untap step.";

/// The two abilities of "~ enters tapped and doesn't untap during your untap step.": the replacement of its entering
/// and the static that keeps it tapped.
fn c10_uc_stays() -> Vec<Ability> {
    vec![
        Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector::this(),
            },
            by: ReplaceBy::Tapped,
            condition: None,
            from: ZoneKind::Battlefield,
        },
        statics(Ref::This, vec![Modification::DoesntUntap], None),
    ]
}

/// "~ enters tapped and doesn't untap during your untap step." (Traxos) read into the two abilities in order.
pub(super) fn c10_uc_enters_held(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    r.source().then_some(())?;
    r.need("enters tapped and doesn't untap during your untap step")?;
    r.sentence_end().then_some(())?;
    Some(c10_uc_stays())
}

/// The sentence of the two abilities `c10_uc_enters_held` reads, or nothing for any other pair.
pub(super) fn c10_uc_enters_held_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let stays = c10_uc_stays();
    if stays.first() == Some(first) && stays.get(1) == Some(second) {
        vec![C10_UC_STAYS.to_owned()]
    } else {
        Vec::new()
    }
}
