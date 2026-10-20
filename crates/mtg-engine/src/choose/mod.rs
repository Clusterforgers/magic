//! What players choose while a spell or ability is announced and resolved.
//!
//! Each decision the effects need has its own file with its options, its answer and the check that turns
//! the answer into what the engine does with it (`docs/cards-dsl.md`, "Decisions the effects need"). Targets
//! and cards and their order are built; the other kinds hold their types and refuse their answers until their task.

mod ask;
mod cards;
mod costs;
mod divide;
mod mode;
mod number;
mod option;
mod order;
mod replacement;
mod targets;
mod triggers;
mod yes_no;

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::object::ObjectRef;

pub use cards::{CardsChoice, CardsOptions};
pub use costs::{CostsChoice, CostsOptions, OptionalCost};
pub use divide::{DivideChoice, DivideOptions};
pub use mode::{ModeChoice, ModeOption, ModeOptions};
pub use number::{NumberChoice, NumberOptions};
pub use option::{ChoiceOption, OptionChoice, OptionOptions};
pub use order::{OrderChoice, OrderOptions};
pub use replacement::{ReplacementCandidate, ReplacementChoice, ReplacementOptions};
pub use targets::{TargetChoice, TargetOptions, is_legal, legal_targets};
pub use triggers::{TriggerEntry, TriggersChoice, TriggersOptions};
pub use yes_no::{YesNoChoice, YesNoOptions};

pub(crate) use ask::{Questions, Stop};
pub(crate) use cards::{Chooser, Picked, pick_cards};
pub(crate) use targets::{asks, legal_for, options_for};

pub(crate) use cards::answer as cards_answer;
pub(crate) use costs::answer as costs_answer;
pub(crate) use divide::answer as divide_answer;
pub(crate) use mode::answer as mode_answer;
pub(crate) use number::answer as number_answer;
pub(crate) use option::answer as option_answer;
pub(crate) use order::answer as order_answer;
pub(crate) use replacement::answer as replacement_answer;
pub(crate) use targets::answer as targets_answer;
pub(crate) use triggers::answer as triggers_answer;
pub(crate) use yes_no::answer as yes_no_answer;

/// A player or an object, as a spell or ability names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Target {
    /// A player.
    Player(PlayerId),
    /// An object, as it was when named: if it has changed zones since, it is a new object (CR 400.7).
    Object(ObjectRef),
}

impl Target {
    /// The player, if this is one.
    pub const fn player(self) -> Option<PlayerId> {
        match self {
            Target::Player(player) => Some(player),
            Target::Object(_) => None,
        }
    }

    /// The object, if this is one.
    pub const fn object(self) -> Option<ObjectRef> {
        match self {
            Target::Object(object) => Some(object),
            Target::Player(_) => None,
        }
    }
}

/// What a player announced for a spell or ability as it went on the stack, in the order it was asked
/// (CR 601.2b to 601.2f).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Choices {
    /// The modes chosen, as indices into the ability's modes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<usize>>", optional))]
    pub modes: Vec<usize>,
    /// The value of X, which outlives the spell (CR 107.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub x: Option<u32>,
    /// The targets chosen for each target specification, in the order of the specifications.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Vec<Target>>>", optional))]
    pub targets: Vec<Vec<Target>>,
    /// How damage or counters are divided among the targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<(Target, u32)>>", optional))]
    pub division: Vec<(Target, u32)>,
    /// The optional costs paid, as indices into the ability's optional costs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<usize>>", optional))]
    pub optional_costs: Vec<usize>,
}

impl Choices {
    /// Whether nothing was announced, so serialization can leave it out.
    pub fn is_empty(&self) -> bool {
        *self == Choices::default()
    }
}

/// One answer a player gave during a resolution, kept in the journal until the step that asked consumes it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Choice {
    /// A number (`ChooseNumber`).
    Number(u32),
    /// Cards chosen (`ChooseCards`).
    Cards(Vec<ObjectId>),
    /// Cards put in an order, first to last (`OrderCards`).
    Order(Vec<ObjectId>),
    /// Yes or no (`YesNo`).
    YesNo(bool),
    /// One of the offered options, by index (`ChooseOption`).
    Option(usize),
    /// One of the applicable replacement effects, by index (`ChooseReplacement`).
    Replacement(usize),
}

impl Choice {
    /// The objects the choice names, for deciding who may see it.
    pub fn objects(&self) -> &[ObjectId] {
        match self {
            Choice::Cards(cards) | Choice::Order(cards) => cards,
            Choice::Number(_) | Choice::YesNo(_) | Choice::Option(_) | Choice::Replacement(_) => {
                &[]
            }
        }
    }
}
