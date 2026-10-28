//! What players choose while a spell or ability is announced and resolved.
//!
//! Each decision the effects need has its own file with its options, its answer and the check that turns
//! the answer into what the engine does with it (`docs/cards-dsl.md`, "Decisions the effects need"). The questions of
//! the announcement (modes, X, optional costs, targets, division) are asked by `announce`.

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

use mtg_cards::value::CastWay;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::object::ObjectRef;
use crate::snapshot::Snapshot;

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
pub(crate) use cards::check as check_cards;
pub(crate) use cards::{Chooser, Picked, Selection, pick_cards, pick_selections};
pub(crate) use mode::{
    C11dOnce, Modal, announceable, announced_specs, enough_modes, every_mode_is_legal, modal_of,
    mode_options, targets_before,
};
pub(crate) use option::{OptionChooser, Optioned, pick_options};
pub(crate) use targets::{
    asks, legal_for, legal_from, options_for, reads_x, targets_met, without_earlier,
};
pub(crate) use yes_no::{Said, pick_yes_no};

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
    /// The game picked the modes with the next substream of its random source ("choose one at random"): the event
    /// that carries these choices spends the substream.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub random_modes: bool,
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
    /// The alternative cost the spell is cast for; none for its mana cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub way: Option<CastWay>,
    /// The face of a card with several faces the spell is cast as (CR 601.2a): the half of a split card, the Adventure or the
    /// back face of a modal double-faced card; none for a card with one face, the front face of an adventure card or a
    /// modal double-faced card. An ability on the stack keeps the face of its source here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub face: Option<u8>,
    /// The mana spent to cast the spell, as it was paid (CR 601.2h): "if {R} was spent to cast it" reads it, from the
    /// spell and from the permanent it becomes; empty for a spell cast without paying mana and for an ability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<Vec<mtg_core::mana::ManaType>>", optional)
    )]
    pub spent: Option<Box<Vec<mtg_core::mana::ManaType>>>,
    /// What the cost of an ability or a spell took: the objects picked and how the ones that left were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cost_taken: Option<Box<CostTaken>>,
}

/// What the cost of an ability or a spell took (CR 601.2h, 608.2h).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CostTaken {
    /// The permanents and cards picked for the parts of an ability's cost that take them (a sacrifice, a discard, an
    /// exile from a graveyard, a tap, a return to hand), one list for each such part in the order the cost prints them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Vec<ObjectId>>>", optional))]
    pub picks: Vec<Vec<ObjectId>>,
    /// The permanents and cards the cost took out of their zones (a sacrifice, a discard, an exile), as they were just
    /// before: "the sacrificed creature's power" reads them (CR 608.2h). Public, as every such object is shown as it leaves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Snapshot>>", optional))]
    pub shots: Vec<Snapshot>,
    /// Which of the costs a card lets its caster choose between ("discard a card or pay 3 life") was chosen, by position
    /// among the combinations of the choices the card prints; 0 for a card with no such choice.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u8>", optional))]
    pub variant: u8,
}

/// Whether a count is 0, so serialization can leave it out.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(count: &u8) -> bool {
    *count == 0
}

impl Choices {
    /// The mana spent to cast the spell, as it was paid (CR 601.2h).
    pub fn spent(&self) -> &[mtg_core::mana::ManaType] {
        self.spent.as_deref().map_or(&[], Vec::as_slice)
    }

    /// The permanents and cards picked for the parts of the cost that take them, one list for each such part.
    pub fn cost_picks(&self) -> &[Vec<ObjectId>] {
        self.cost_taken.as_deref().map_or(&[], |taken| &taken.picks)
    }

    /// Which of the costs the card lets its caster choose between was chosen (`CostTaken::variant`).
    pub fn cost_variant(&self) -> u8 {
        self.cost_taken.as_deref().map_or(0, |taken| taken.variant)
    }

    /// Records the cost chosen between the costs of the card; nothing for 0 on choices that took nothing.
    pub(crate) fn choose_cost(&mut self, variant: u8) {
        if variant != 0 || self.cost_taken.is_some() {
            self.cost_taken.get_or_insert_with(Box::default).variant = variant;
        }
    }

    /// What the cost took out of its zones, as it was just before (`parts::photographed`).
    pub fn cost_shots(&self) -> &[Snapshot] {
        self.cost_taken.as_deref().map_or(&[], |taken| &taken.shots)
    }

    /// These choices with `shots` as the objects the cost took.
    pub(crate) fn with_cost_shots(mut self, shots: Vec<Snapshot>) -> Choices {
        if !shots.is_empty() || self.cost_taken.is_some() {
            self.cost_taken.get_or_insert_with(Box::default).shots = shots;
        }
        self
    }

    /// Forgets the objects picked, as a copy of a spell pays nothing; what the cost of the original took stays named.
    pub(crate) fn forget_cost_picks(&mut self) {
        if let Some(taken) = self.cost_taken.as_deref_mut() {
            taken.picks.clear();
        }
    }

    /// Adds the objects picked for the next part of the cost that takes some.
    pub(crate) fn pick_for_cost(&mut self, objects: Vec<ObjectId>) {
        self.cost_taken
            .get_or_insert_with(Box::default)
            .picks
            .push(objects);
    }

    /// Whether nothing was announced, so serialization can leave it out.
    pub fn is_empty(&self) -> bool {
        *self == Choices::default()
    }

    /// The objects chosen as targets, each once however many specifications name it: it becomes a target once.
    pub(crate) fn targeted_objects(&self) -> Vec<ObjectRef> {
        let mut found = Vec::new();
        for object in self.targets.iter().flatten().filter_map(|t| t.object()) {
            if !found.contains(&object) {
                found.push(object);
            }
        }
        found
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
    /// One of the options of a secret vote, by index (`ChoiceOption::Secret`); the other seats do not see which.
    Vote(usize),
    /// One of the applicable replacement effects, by index (`ChooseReplacement`).
    Replacement(usize),
    /// The targets chosen for a copy of a spell or ability (`ChooseTargets`); targets are public.
    Targets(Vec<Target>),
}

impl Choice {
    /// The objects the choice names, for deciding who may see it.
    pub fn objects(&self) -> &[ObjectId] {
        match self {
            Choice::Cards(cards) | Choice::Order(cards) => cards,
            Choice::Number(_)
            | Choice::YesNo(_)
            | Choice::Option(_)
            | Choice::Vote(_)
            | Choice::Replacement(_)
            | Choice::Targets(_) => &[],
        }
    }
}
