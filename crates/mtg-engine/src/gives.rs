//! What an attachment gives its host as the layers apply it now: the one reading the views show beside the host.
//!
//! It is read from the record of the effects that changed the host in the pass of `layers.rs` (`Held::applied`), so a
//! condition that fails, an ability the attachment lost or an effect that ended leaves nothing, and no card text is
//! read a second time (`docs/ui-design.md`, Attachments).

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use serde::{Deserialize, Serialize};

use crate::abilities::Grant;
use crate::effects::Modification;
use crate::keywords::keywords_of;
use crate::layers::Applied;
use crate::state::GameState;

/// A change an attachment makes to its host beyond power, toughness and keywords, named for a mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum GivenRule {
    /// The host can't attack.
    CantAttack,
    /// The host can't attack the attachment's controller or their planeswalkers.
    CantAttackYou,
    /// The host can't block.
    CantBlock,
    /// The host can block only some attackers.
    CanBlockOnly,
    /// The host can't be blocked.
    CantBeBlocked,
    /// The host can't be blocked by some creatures.
    CantBeBlockedBy,
    /// The host needs at least, or allows at most, some number of blockers.
    BlockerLimit,
    /// The host can block additional creatures.
    BlocksAdditional,
    /// The host doesn't untap in its controller's untap step.
    DoesntUntap,
    /// The host can't be turned face up.
    CantBeTurnedFaceUp,
    /// The host loses all abilities.
    LoseAbilities,
    /// The host loses a keyword.
    LoseKeyword,
    /// The host gains an ability that is not a keyword.
    GainAbility,
    /// The attachment's controller gains control of the host.
    SetController,
    /// The host's power and toughness are switched.
    SwitchPowerToughness,
    /// The host's card types, subtypes or creature types change.
    ChangeTypes,
    /// The host's colours change.
    SetColors,
    /// The host is goaded.
    Goaded,
    /// The attachment has umbra armor: the host is not destroyed while it can go instead (CR 702.89).
    UmbraArmor,
}

/// What an attachment gives its host now; [`Gives::is_empty`] while it gives nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gives {
    /// The net change to the host's power (layer 7c).
    pub power: i32,
    /// The net change to the host's toughness (layer 7c).
    pub toughness: i32,
    /// The power and toughness it sets (layer 7b), the last it sets.
    pub set: Option<(i32, i32)>,
    /// The keywords it grants (layer 6), each once, in the order applied.
    pub keywords: Vec<Keyword>,
    /// The other changes it makes, each once, in the order applied.
    pub rules: Vec<GivenRule>,
}

impl Gives {
    /// Whether it gives nothing.
    pub fn is_empty(&self) -> bool {
        *self == Gives::default()
    }

    /// Adds what one applied modification of the attachment does.
    fn add(&mut self, modification: &Modification) {
        let rule = match modification {
            Modification::ModifyPowerToughness { power, toughness } => {
                self.power += power;
                self.toughness += toughness;
                return;
            }
            Modification::SetPowerToughness { power, toughness } => {
                self.set = Some((*power, *toughness));
                return;
            }
            Modification::AddKeyword(keyword) => {
                if !self.keywords.contains(keyword) {
                    self.keywords.push(keyword.clone());
                }
                return;
            }
            Modification::CantAttack => GivenRule::CantAttack,
            Modification::CantAttackYou { .. } => GivenRule::CantAttackYou,
            Modification::CantBlock => GivenRule::CantBlock,
            Modification::CanBlockOnly { .. } => GivenRule::CanBlockOnly,
            Modification::CantBeBlocked => GivenRule::CantBeBlocked,
            Modification::CantBeBlockedBy { .. } => GivenRule::CantBeBlockedBy,
            Modification::MinBlockers(_) | Modification::MaxBlockers(_) => GivenRule::BlockerLimit,
            Modification::BlocksAdditional(_) => GivenRule::BlocksAdditional,
            Modification::DoesntUntap => GivenRule::DoesntUntap,
            Modification::CantBeTurnedFaceUp => GivenRule::CantBeTurnedFaceUp,
            Modification::LoseAbilities => GivenRule::LoseAbilities,
            Modification::RemoveKeyword(_) => GivenRule::LoseKeyword,
            Modification::GrantAbility(_) => GivenRule::GainAbility,
            Modification::SetController(_) => GivenRule::SetController,
            Modification::SwitchPowerToughness => GivenRule::SwitchPowerToughness,
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::AddAllCreatureTypes => GivenRule::ChangeTypes,
            Modification::SetColors(_) => GivenRule::SetColors,
            Modification::Goaded { .. } => GivenRule::Goaded,
        };
        self.rule(rule);
    }

    /// Adds a rule once.
    fn rule(&mut self, rule: GivenRule) {
        if !self.rules.contains(&rule) {
            self.rules.push(rule);
        }
    }
}

/// What the attachment `id` gives the permanent or player it is attached to, or none while it is attached to neither
/// or gives nothing: the effects of its static abilities and of the effects whose source it is, as the layers applied
/// them to the host in this state. An Aura on a player gives keywords or takes them (`GivenRule::LoseKeyword`).
pub fn gives(state: &GameState, id: ObjectId) -> Option<Gives> {
    let layers = state.layers();
    let applied: &[Applied] = if let Some(host) = state.host_of(id) {
        layers.get(host).map_or(&[], |held| held.applied.as_slice())
    } else {
        let player = state.player_host_of(id)?;
        layers
            .player(player)
            .map_or(&[], |held| held.applied.as_slice())
    };
    let mut gives = Gives::default();
    for applied in applied.iter().filter(|applied| from(state, applied, id)) {
        gives.add(&applied.modification);
    }
    if keywords_of(state, id).contains(&&Keyword::UmbraArmor) {
        gives.rule(GivenRule::UmbraArmor);
    }
    (!gives.is_empty()).then_some(gives)
}

/// Whether `applied` comes from the object `id`: a static ability it has, or an effect whose source it is.
fn from(state: &GameState, applied: &Applied, id: ObjectId) -> bool {
    match applied.origin {
        Grant::Static { source, .. } => source == id,
        Grant::Effect(effect) => state
            .effects()
            .iter()
            .any(|e| e.id == effect && e.source.as_ref().is_some_and(|source| source.id == id)),
    }
}
