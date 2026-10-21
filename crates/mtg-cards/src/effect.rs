//! Effects: the verbs of the DSL. One variant per engine primitive, so a new mechanic is one variant.

use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::card::TokenSpec;
use crate::cost::Cost;
use crate::modify::{Duration, GameRule, Modification};
use crate::refs::{Bounds, Ref, Selector, TargetSpec, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Condition, CounterKind};

// serde's `skip_serializing_if` hands out a reference, so these take one.
#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) const fn is_false(value: &bool) -> bool {
    !*value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) const fn is_true(value: &bool) -> bool {
    *value
}

pub(crate) const fn yes() -> bool {
    true
}

/// Where in a library cards go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Place {
    /// On top.
    Top,
    /// On the bottom.
    Bottom,
    /// Into the library, which is then shuffled.
    Shuffled,
}

/// Who orders several cards put into one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Order {
    /// The player who puts them chooses ("in any order").
    Chosen,
    /// Random order.
    Random,
}

/// Where a moved object goes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Destination {
    /// Its owner's hand.
    Hand,
    /// Its owner's graveyard.
    Graveyard,
    /// The battlefield.
    Battlefield {
        /// Enters tapped.
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
        /// Enters under this player's control; its owner's when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        under: Option<Ref>,
    },
    /// Its owner's library.
    Library {
        /// Top, bottom or shuffled in.
        place: Place,
        /// Who orders several cards.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        order: Option<Order>,
    },
}

/// Mana an effect adds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Produce {
    /// This mana, as printed (`{C}{C}`).
    Fixed(Vec<ManaType>),
    /// This many mana of any one color, chosen when added.
    AnyColor(u32),
}

/// What a created token is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TokenRef {
    /// A predefined token by its id in `cards/tokens/` (`treasure`).
    Named(String),
    /// A token described by the effect.
    Custom(Box<TokenSpec>),
    /// A token that is a copy of the referenced object, with exceptions.
    CopyOf {
        /// The object copied.
        of: Ref,
        /// Changes the copy has ("except it's a Zombie in addition").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        except: Vec<Modification>,
    },
}

/// One option of a modal spell or ability, with its own targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    /// A short name for prompts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The mode's targets; `Target(i)` in its effects refers to these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TargetSpec>,
    /// What the mode does.
    pub effects: Vec<Effect>,
}

/// One instruction of a spell or ability. Optional fields have defaults: `who` is the
/// controller, an `Amount` is one, `source` is the ability's source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Effect {
    /// The player draws cards.
    Draw {
        /// Who draws.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The player discards cards of their choice, or at random.
    Discard {
        /// Who discards.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// Cards are chosen at random.
        #[serde(default, skip_serializing_if = "is_false")]
        random: bool,
    },
    /// The player mills cards.
    Mill {
        /// Who mills.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The player scries (CR 701.22).
    Scry {
        /// Who scries.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The controller looks at the top cards of a library and binds them.
    LookAtTop {
        /// Whose library.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many cards.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// The name the cards are bound to.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The objects are revealed to every player.
    Reveal {
        /// What is revealed.
        what: Ref,
    },
    /// The player searches a zone for cards and binds those they find (CR 701.23).
    Search {
        /// Who searches.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Whose zone: their library unless the zone says otherwise.
        #[serde(
            default = "ZoneKind::library",
            skip_serializing_if = "ZoneKind::is_library"
        )]
        zone: ZoneKind,
        /// What the cards must be.
        filter: Selector,
        /// How many may be found.
        #[serde(default, skip_serializing_if = "Bounds::is_one")]
        count: Bounds,
        /// The cards are revealed.
        #[serde(default, skip_serializing_if = "is_false")]
        reveal: bool,
        /// The name the found cards are bound to.
        #[serde(rename = "as")]
        bind: String,
    },
    /// The player chooses some of a bound set and binds the choice (and optionally the rest).
    Choose {
        /// Who chooses.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The objects chosen from.
        from: Ref,
        /// How many.
        #[serde(default, skip_serializing_if = "Bounds::is_one")]
        count: Bounds,
        /// The name the chosen objects are bound to.
        #[serde(rename = "as")]
        bind: String,
        /// The name the objects not chosen are bound to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rest_as: Option<String>,
    },
    /// The player shuffles their library.
    Shuffle {
        /// Who shuffles.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// Objects change zones.
    Move {
        /// What moves.
        what: Ref,
        /// Where it goes.
        to: Destination,
    },
    /// Destroys permanents (CR 701.8).
    Destroy {
        /// What is destroyed.
        what: Ref,
        /// They can be regenerated; `false` for "can't be regenerated".
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        regenerate: bool,
    },
    /// Regenerates permanents (CR 701.19): until end of turn, the next time each would be destroyed it is tapped,
    /// loses its damage and leaves combat instead.
    Regenerate {
        /// The permanents given a regeneration shield.
        what: Ref,
    },
    /// Exiles objects.
    Exile {
        /// What is exiled.
        what: Ref,
    },
    /// The player sacrifices permanents they control that match, of their choice.
    Sacrifice {
        /// Who sacrifices.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Which permanents may be chosen.
        filter: Selector,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Counters spells or abilities (CR 701.6).
    Counter {
        /// What is countered.
        what: Ref,
    },
    /// Deals damage.
    DealDamage {
        /// The source of the damage; the ability's source when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Ref>,
        /// Who or what is dealt damage.
        to: Ref,
        /// How much.
        amount: Amount,
    },
    /// The player gains life.
    GainLife {
        /// Who gains.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How much.
        amount: Amount,
    },
    /// The player loses life.
    LoseLife {
        /// Who loses.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How much.
        amount: Amount,
    },
    /// Adds mana to a pool.
    AddMana {
        /// Whose pool.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The mana.
        mana: Produce,
        /// How many times it is added.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        times: Amount,
    },
    /// Puts counters on an object or player.
    PutCounters {
        /// Where they go.
        on: Ref,
        /// The kind.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Removes counters from an object or player; one that has fewer loses what it has.
    RemoveCounters {
        /// Where they come from.
        from: Ref,
        /// The kind.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Moves counters from one object onto another (CR 122.5); the first object gives up what it has, up to the count.
    MoveCounters {
        /// The object they leave.
        from: Ref,
        /// The object they go onto.
        to: Ref,
        /// The kind.
        counter: CounterKind,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// Taps permanents.
    Tap {
        /// What is tapped.
        what: Ref,
    },
    /// Untaps permanents.
    Untap {
        /// What is untapped.
        what: Ref,
    },
    /// Attaches Auras, Equipment or Fortifications to an object (CR 701.3).
    Attach {
        /// The permanents attached; each moves to `to` when it can be attached there.
        what: Ref,
        /// The object they are attached to.
        to: Ref,
    },
    /// Unattaches Auras, Equipment or Fortifications from what they are attached to (CR 701.3d).
    Unattach {
        /// The permanents unattached; one attached to nothing stays as it is.
        what: Ref,
    },
    /// Transforms permanents.
    Transform {
        /// What transforms.
        what: Ref,
    },
    /// The player wins the game.
    WinGame {
        /// Who wins.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
    },
    /// Creates tokens.
    CreateToken {
        /// Who controls them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// What they are.
        token: TokenRef,
        /// How many.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
        /// They enter tapped.
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
        /// They enter attacking.
        #[serde(default, skip_serializing_if = "is_false")]
        attacking: bool,
        /// Names the tokens for the effects that follow ("then attach this to it").
        #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
    },
    /// Creates a continuous effect from this resolution; the objects are fixed now (CR 611.2c).
    Continuous {
        /// The objects changed.
        affected: Ref,
        /// The changes.
        mods: Vec<Modification>,
        /// How long they last.
        duration: Duration,
    },
    /// Changes a rule of the game for a while.
    Rule {
        /// The rule.
        rule: GameRule,
        /// How long.
        duration: Duration,
    },
    /// Creates a shield (CR 615): a replacement effect for a while, whose references name what they do now
    /// (CR 611.2c). A damage shield prevents damage ("prevent the next 3 damage that would be dealt to target creature
    /// this turn").
    Shield {
        /// The event it replaces.
        when: ReplaceWhen,
        /// What happens instead.
        by: ReplaceBy,
        /// How long it lasts.
        duration: Duration,
    },
    /// Creates a delayed triggered ability (CR 603.7); bound names are captured as they are now.
    DelayedTrigger {
        /// The event it waits for.
        when: TriggerPattern,
        /// It triggers once and is gone (the usual case).
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        once: bool,
        /// What it does.
        effects: Vec<Effect>,
    },
    /// Does `then` if the condition holds, `otherwise` if not.
    If {
        /// The test.
        condition: Condition,
        /// Done when true.
        then: Vec<Effect>,
        /// Done when false.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        otherwise: Vec<Effect>,
    },
    /// The player may do these effects.
    May {
        /// Who decides.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// What they may do.
        effects: Vec<Effect>,
    },
    /// The player may pay; `paid` follows if they do, `unpaid` if they do not ("unless pays").
    Pay {
        /// Who may pay.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The cost.
        cost: Cost,
        /// Done when the cost is paid.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        paid: Vec<Effect>,
        /// Done when it is not.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unpaid: Vec<Effect>,
    },
    /// The controller chooses modes when the spell or ability is put on the stack (CR 700.2).
    Modal {
        /// How many modes are chosen.
        #[serde(default, skip_serializing_if = "Bounds::is_one")]
        choose: Bounds,
        /// The same mode may be chosen more than once.
        #[serde(default, skip_serializing_if = "is_false")]
        repeat: bool,
        /// The modes, in printed order.
        modes: Vec<Mode>,
    },
}

#[cfg(test)]
mod tests {
    //! Goal: effects read and write the compact forms card files use, with defaults left out, and refuse what is not a field.
    use super::*;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use crate::refs::Pred;

    #[rstest]
    #[case::draw_one(Effect::Draw { who: Ref::You, count: Amount::one() }, r#"{"Draw":{}}"#)]
    #[case::opponent_draws(
        Effect::Draw { who: Ref::Target(0), count: Amount::Fixed(2) },
        r#"{"Draw":{"who":{"Target":0},"count":2}}"#
    )]
    #[case::bolt(
        Effect::DealDamage { source: None, to: Ref::Target(0), amount: Amount::Fixed(3) },
        r#"{"DealDamage":{"to":{"Target":0},"amount":3}}"#
    )]
    #[case::wrath(
        Effect::Destroy { what: Ref::Each(Selector(vec![Pred::Type(CardType::Creature)])), regenerate: false },
        r#"{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":false}}"#
    )]
    #[case::destroy_default(
        Effect::Destroy { what: Ref::Target(0), regenerate: true },
        r#"{"Destroy":{"what":{"Target":0}}}"#
    )]
    #[case::move_to_battlefield_tapped(
        Effect::Move { what: Ref::Bound("land".into()), to: Destination::Battlefield { tapped: true, under: None } },
        r#"{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}"#
    )]
    #[case::search(
        Effect::Search { who: Ref::You, zone: ZoneKind::Library, filter: Selector(vec![Pred::Supertype(mtg_core::card::Supertype::Basic)]), count: Bounds::default(), reveal: false, bind: "land".into() },
        r#"{"Search":{"filter":[{"Supertype":"Basic"}],"as":"land"}}"#
    )]
    #[case::pay(
        Effect::Pay { who: Ref::Triggering, cost: Cost::mana("{1}".parse().unwrap()), paid: vec![], unpaid: vec![Effect::Draw { who: Ref::You, count: Amount::one() }] },
        r#"{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"Draw":{}}]}}"#
    )]
    #[case::put_a_poison_counter_on_a_player(
        Effect::PutCounters { on: Ref::Target(0), counter: CounterKind::Poison, count: Amount::one() },
        r#"{"PutCounters":{"on":{"Target":0},"counter":"poison"}}"#
    )]
    #[case::remove_all_the_counters_of_a_kind(
        Effect::RemoveCounters { from: Ref::This, counter: CounterKind::Other("energy".into()), count: Amount::Counters { on: Ref::This, counter: CounterKind::Other("energy".into()) } },
        r#"{"RemoveCounters":{"from":"This","counter":{"Other":"energy"},"count":{"Counters":{"on":"This","counter":{"Other":"energy"}}}}}"#
    )]
    #[case::attach_the_source_to_a_target(
        Effect::Attach { what: Ref::This, to: Ref::Target(0) },
        r#"{"Attach":{"what":"This","to":{"Target":0}}}"#
    )]
    #[case::unattach_the_source(Effect::Unattach { what: Ref::This }, r#"{"Unattach":{"what":"This"}}"#)]
    #[case::move_two_counters(
        Effect::MoveCounters { from: Ref::Target(0), to: Ref::Target(1), counter: CounterKind::PlusOnePlusOne, count: Amount::Fixed(2) },
        r#"{"MoveCounters":{"from":{"Target":0},"to":{"Target":1},"counter":"+1/+1","count":2}}"#
    )]
    fn effects_have_stable_json(#[case] value: Effect, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Effect>(json).unwrap(), value);
    }

    #[rstest]
    #[case::unknown_field(r#"{"Draw":{"cards":2}}"#, "unknown field `cards`")]
    #[case::unknown_variant(r#"{"Teleport":{}}"#, "unknown variant `Teleport`")]
    #[case::missing_field(r#"{"DealDamage":{"amount":3}}"#, "missing field `to`")]
    #[case::bad_bind(r#"{"LookAtTop":{}}"#, "missing field `as`")]
    fn bad_effects_say_why(#[case] json: &str, #[case] expected: &str) {
        let error = serde_json::from_str::<Effect>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}
