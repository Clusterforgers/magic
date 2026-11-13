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
    /// Any ability was activated, a mana ability (CR 605.3b) included; `what` is its source.
    ActivatedAny,
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
    /// A creature enlisted a creature (CR 702.154a); `what` is the creature that enlisted.
    Enlisted,
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
    /// A player foraged (CR 701.61): exiled three cards from their graveyard or sacrificed a Food.
    Foraged,
    /// A card became plotted (CR 702.170a): its owner exiled it face up with plot.
    Plotted,
    /// An exhaust ability was activated (CR 702.177); `what` is its source.
    ActivatedExhaust,
    /// A boast ability was activated (CR 702.142); `what` is its source.
    ActivatedBoast,
    /// A player clashed (CR 701.23), won or lost; `Amount::Triggering` is 1 when they won and 0 when they lost.
    Clashed,
    /// A player investigated (CR 701.16a), once for each Clue token the investigation made.
    Investigated,
    /// A player discovered (CR 701.57); `what` is the card discovered and the amount the discover value.
    Discovered,
    /// A player manifested dread (CR 701.62); `what` is the card they put into their graveyard this way.
    ManifestedDread,
    /// A player gave a gift (CR 702.174); `what` is nothing, the player given it is the second subject.
    GaveGift,
    /// A player paid life (CR 119.4), as a cost or for an effect; the amount is the life paid.
    PaidLife,
    /// A creature exploited a creature that was not a token (CR 702.110b), judged as the battlefield was before the
    /// sacrifice; `what` is the creature that exploited.
    ExploitedNontoken,
    /// An opponent of a player gained control of a permanent that player controlled (CR 613.1b); the player is the one
    /// who lost control, `what` the permanent.
    ControlTakenByOpponent,
    /// A creature explored and revealed a land card (CR 701.44); `what` is the creature.
    ExploredLand,
    /// A creature explored and revealed a card that is not a land card (CR 701.44); `what` is the creature.
    ExploredNonland,
    /// A player paid the cumulative upkeep of a permanent (CR 702.24a); `what` is the permanent.
    PaidCumulativeUpkeep,
    /// A player didn't pay the cumulative upkeep of a permanent (CR 702.24a); `what` is the permanent.
    DeclinedCumulativeUpkeep,
    /// A player paid the echo cost of a permanent (CR 702.30a); `what` is the permanent.
    PaidEcho,
}

impl Action {
    /// The keyword kinds an activation of the ability also is: exhaust, once each game (CR 702.177), and boast, only
    /// if the source attacked this turn and only once each turn (CR 702.142).
    #[must_use]
    pub fn activation_kinds(ability: &crate::ability::Ability) -> Vec<Action> {
        use crate::ability::{Ability, Limit};
        use crate::refs::{Pred, Ref, Selector};
        use crate::value::Condition;
        let Ability::Activated {
            limit, condition, ..
        } = ability
        else {
            return Vec::new();
        };
        let attacked = Condition::Matches {
            subject: Ref::This,
            selector: Selector(vec![Pred::AttackedThisTurn]),
        };
        match (limit, condition) {
            (Some(Limit::OncePerGame), _) => vec![Action::ActivatedExhaust],
            (Some(Limit::OncePerTurn), Some(held)) if *held == attacked => {
                vec![Action::ActivatedBoast]
            }
            _ => Vec::new(),
        }
    }

    /// The verb phrase of the action for the subject "you" (first) and for a third person (second).
    #[must_use]
    pub const fn verbs(self) -> (&'static str, &'static str) {
        match self {
            // `Activated` leaves out the mana abilities (CR 605.3b), so the words say so; `ActivatedAny` counts them.
            Action::Activated => (
                "activate an ability that isn't a mana ability",
                "activates an ability that isn't a mana ability",
            ),
            Action::ActivatedLoyalty => {
                ("activate a loyalty ability", "activates a loyalty ability")
            }
            Action::ActivatedAny => ("activate an ability", "activates an ability"),
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
            Action::Enlisted => ("enlist a creature", "enlists a creature"),
            Action::Exerted => ("exert a creature", "exerts a creature"),
            Action::Mutated => ("mutate", "mutates"),
            Action::Learned => ("learn", "learns"),
            Action::Rolled => ("roll a die", "rolls a die"),
            Action::CollectedEvidence => ("collect evidence", "collects evidence"),
            Action::Foraged => ("forage", "forages"),
            Action::Plotted => ("plot a card", "plots a card"),
            Action::ActivatedExhaust => (
                "activate an exhaust ability",
                "activates an exhaust ability",
            ),
            Action::ActivatedBoast => ("activate a boast ability", "activates a boast ability"),
            Action::Clashed => ("clash", "clashes"),
            Action::Investigated => ("investigate", "investigates"),
            Action::Discovered => ("discover", "discovers"),
            Action::ManifestedDread => ("manifest dread", "manifests dread"),
            Action::GaveGift => ("give a gift", "gives a gift"),
            Action::PaidLife => ("pay life", "pays life"),
            Action::ExploitedNontoken => (
                "exploit a nontoken creature",
                "exploits a nontoken creature",
            ),
            Action::ExploredLand => ("explore a land card", "explores a land card"),
            Action::ExploredNonland => ("explore a nonland card", "explores a nonland card"),
            Action::PaidCumulativeUpkeep => ("pay a cumulative upkeep", "pays a cumulative upkeep"),
            Action::DeclinedCumulativeUpkeep => (
                "don't pay a cumulative upkeep",
                "doesn't pay a cumulative upkeep",
            ),
            Action::PaidEcho => ("pay an echo cost", "pays an echo cost"),
            Action::ControlTakenByOpponent => (
                "have an opponent gain control of a permanent from you",
                "has an opponent gain control of a permanent from them",
            ),
        }
    }

    /// Whether the abilities and objects it concerns are those of the state before it (CR 603.10a): what left the    /// battlefield, or the player who lost the game, is judged as it was before.
    #[must_use]
    pub const fn looks_back(self) -> bool {
        matches!(
            self,
            Action::Activated
                | Action::ActivatedLoyalty
                | Action::ActivatedExhaust
                | Action::ActivatedBoast
                | Action::ActivatedAny
                | Action::Destroyed
                | Action::Exiled
                | Action::Sacrificed
                | Action::Exploited
                | Action::ExploitedNontoken
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
    #[case::any(Action::ActivatedAny, false)]
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
