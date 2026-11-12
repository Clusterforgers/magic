//! The actions a player or object takes that `TriggerPattern::Did` waits for: one entry here and one event mapping in the
//! engine add an action to every card.

use serde::{Deserialize, Serialize};

/// What happened, for `TriggerPattern::Did { action, who, what }`. `who` is the player who did it (the controller of the
/// object for an action of a permanent) and `what` the object it was done to or by, when the action has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    /// An ability that isn't a mana ability was activated; `what` is its source.
    Activated,
    /// A loyalty ability was activated (CR 606.4); `what` is its source.
    ActivatedLoyalty,
    /// A triggered ability triggered; `what` is its source.
    AbilityTriggered,
    /// A land was played (CR 305.1), never one put onto the battlefield by an effect; `what` is the land.
    PlayedLand,
    /// A double-faced permanent transformed (CR 701.28).
    Transformed,
    /// A flip card was flipped (CR 710).
    Flipped,
    /// A permanent was turned face up.
    TurnedFaceUp,
    /// A player scried (CR 701.22).
    Scried,
    /// A player surveilled (CR 701.25).
    Surveilled,
    /// A player searched their library and found cards (CR 701.23).
    SearchedLibrary,
    /// A player searched their own library ("whenever an opponent searches their library"); the search is also a
    /// `SearchedLibrary`.
    SearchedOwnLibrary,
    /// A library was shuffled; `who` is its owner.
    Shuffled,
    /// A door of a Room was unlocked (CR 709.5).
    Unlocked,
    /// The last locked door of a Room was unlocked.
    FullyUnlocked,
    /// A player lost the game.
    LostGame,
    /// A spell was countered; `what` is the spell.
    Countered,
    /// A permanent was destroyed.
    Destroyed,
    /// An object was exiled.
    Exiled,
    /// A permanent was sacrificed.
    Sacrificed,
    /// A card was milled.
    Milled,
    /// A card was foretold.
    Foretold,
    /// A token was created; `who` controls it.
    TokenCreated,
    /// An Aura, Equipment or Fortification stopped being attached.
    Unattached,
    /// Two creatures became paired (CR 702.95).
    Paired,
    /// A permanent regenerated.
    Regenerated,
    /// A permanent changed controller.
    ChangedController,
    /// A spell was copied.
    CopiedSpell,
    /// A player proliferated.
    Proliferated,
    /// A creature explored.
    Explored,
    /// A creature connived.
    Connived,
    /// A player amassed.
    Amassed,
    /// A player bolstered.
    Bolstered,
    /// A creature adapted.
    Adapted,
    /// A creature evolved (CR 702.100d).
    Evolved,
    /// A creature became monstrous.
    BecameMonstrous,
    /// A creature became renowned.
    BecameRenowned,
    /// A creature exploited a creature.
    Exploited,
    /// A permanent was exerted.
    Exerted,
    /// A creature mutated.
    Mutated,
    /// A player learned.
    Learned,
    /// A player rolled a die.
    Rolled,
    /// A player collected evidence (CR 701.59).
    CollectedEvidence,
    /// A player clashed (CR 701.23), won or lost; `Amount::Triggering` is 1 when they won and 0 when they lost.
    Clashed,
}

impl Action {
    /// The verb phrase of the action for the subject "you" (first) and for a third person (second).
    #[must_use]
    pub const fn verbs(self) -> (&'static str, &'static str) {
        match self {
            // The engine logs no activation of a mana ability (CR 605.3b), so the words say so.
            Action::Activated => (
                "activate an ability that isn't a mana ability",
                "activates an ability that isn't a mana ability",
            ),
            Action::ActivatedLoyalty => {
                ("activate a loyalty ability", "activates a loyalty ability")
            }
            Action::AbilityTriggered => ("trigger an ability", "triggers an ability"),
            Action::PlayedLand => ("play a land", "plays a land"),
            Action::Transformed => ("transform a permanent", "transforms a permanent"),
            Action::Flipped => ("flip a permanent", "flips a permanent"),
            Action::TurnedFaceUp => ("turn a permanent face up", "turns a permanent face up"),
            Action::Scried => ("scry", "scries"),
            Action::Surveilled => ("surveil", "surveils"),
            Action::SearchedLibrary => ("search a library", "searches a library"),
            Action::SearchedOwnLibrary => ("search your library", "searches their library"),
            Action::Shuffled => ("shuffle a library", "shuffles a library"),
            Action::Unlocked => ("unlock a door", "unlocks a door"),
            Action::FullyUnlocked => ("fully unlock a Room", "fully unlocks a Room"),
            Action::LostGame => ("lose the game", "loses the game"),
            Action::Countered => ("counter a spell", "counters a spell"),
            Action::Destroyed => ("destroy a permanent", "destroys a permanent"),
            Action::Exiled => ("exile an object", "exiles an object"),
            Action::Sacrificed => ("sacrifice a permanent", "sacrifices a permanent"),
            Action::Milled => ("mill a card", "mills a card"),
            Action::Foretold => ("foretell a card", "foretells a card"),
            Action::TokenCreated => ("create a token", "creates a token"),
            Action::Unattached => ("unattach an attachment", "unattaches an attachment"),
            Action::Paired => ("pair creatures", "pairs creatures"),
            Action::Regenerated => ("regenerate a permanent", "regenerates a permanent"),
            Action::ChangedController => (
                "gain control of a permanent",
                "gains control of a permanent",
            ),
            Action::CopiedSpell => ("copy a spell", "copies a spell"),
            Action::Proliferated => ("proliferate", "proliferates"),
            Action::Explored => ("explore", "explores"),
            Action::Connived => ("connive", "connives"),
            Action::Amassed => ("amass", "amasses"),
            Action::Bolstered => ("bolster", "bolsters"),
            Action::Adapted => ("adapt", "adapts"),
            Action::Evolved => ("evolve", "evolves"),
            Action::BecameMonstrous => ("become monstrous", "becomes monstrous"),
            Action::BecameRenowned => ("become renowned", "becomes renowned"),
            Action::Exploited => ("exploit a creature", "exploits a creature"),
            Action::Exerted => ("exert a creature", "exerts a creature"),
            Action::Mutated => ("mutate", "mutates"),
            Action::Learned => ("learn", "learns"),
            Action::Rolled => ("roll a die", "rolls a die"),
            Action::CollectedEvidence => ("collect evidence", "collects evidence"),
            Action::Clashed => ("clash", "clashes"),
        }
    }

    /// Whether the abilities and objects it concerns are those of the state before it (CR 603.10a): what left the    /// battlefield, or the player who lost the game, is judged as it was before.
    #[must_use]
    pub const fn looks_back(self) -> bool {
        matches!(
            self,
            Action::Activated
                | Action::ActivatedLoyalty
                | Action::Destroyed
                | Action::Exiled
                | Action::Sacrificed
                | Action::Exploited
                | Action::LostGame
        )
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the words of an action say what the engine counts: an activation is never of a mana ability (CR 605.3b).
    use super::Action;
    use rstest::rstest;

    #[rstest]
    #[case::activated(Action::Activated, true)]
    #[case::loyalty(Action::ActivatedLoyalty, false)]
    fn an_activation_says_it_is_no_mana_ability(#[case] action: Action, #[case] says: bool) {
        let (you, they) = action.verbs();
        for words in [you, they] {
            assert_eq!(
                words.ends_with("that isn't a mana ability"),
                says,
                "{words}"
            );
        }
    }
}
