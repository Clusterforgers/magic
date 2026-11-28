//! The event log vocabulary: every change to game state is exactly one of these.

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::abilities::{AbilitySource, Cause, PutOnStack};
use crate::choose::{Choice, Choices, CoinSide, Target};
use crate::combat::{AttackTarget, DamageForm, DamageTarget};
use crate::copies::{AbilityCopy, BecameCopy, CardCopy, SpellCopy};
use crate::counters::CountersChanged;
use crate::day_night::DayNight;
use crate::decision::Unplayable;
use crate::effects::ContinuousEffect;
use crate::library::{CardsFound, HandLooked, LibraryLooked, Revealed, Scried, Surveilled};
use crate::manual::{ManualAction, ManualRoll};
use crate::object::{Chosen, FaceDown, ObjectRef};
use crate::player::LossReason;
use crate::resolve::LeftToTable;
use crate::retargeted::TargetsChanged;
use crate::rules::RuleEffect;
use crate::snapshot::Snapshot;
use crate::spend::Restricted;
use crate::state::GameResult;
use crate::tokens::TokenCreated;
use crate::triggers::{DelayedCreated, DelayedId, Triggered};
use crate::turn::Step;
use crate::zones::Zone;

/// Whether a flag is off, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// Whether a count is zero, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(count: &u32) -> bool {
    *count == 0
}

/// Why a player was not asked to decide something optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum NothingToDo {
    /// The player has no creature able to attack.
    Attack,
    /// The player has no creature able to block an attacker.
    Block,
}

/// The element a player bent (Avatar): the keyword actions "whenever you waterbend, earthbend, firebend, or airbend"
/// waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Bending {
    /// A waterbend cost was paid.
    Water,
    /// An earthbend was done.
    Earth,
    /// A firebending ability resolved.
    Fire,
    /// An airbend was done.
    Air,
}

/// Why a creature was put into its owner's graveyard by state-based actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DeathCause {
    /// Damage marked on it was at least its toughness (CR 704.5g).
    LethalDamage,
    /// A source with deathtouch dealt it damage (CR 704.5h).
    Deathtouch,
    /// Its toughness was 0 or less (CR 704.5f).
    ZeroToughness,
}

/// Which state-based action put a permanent into its owner's graveyard (CR 704.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum StateRule {
    /// A creature died, and why (CR 704.5f, 704.5g, 704.5h).
    Died(DeathCause),
    /// A planeswalker had no loyalty counters (CR 704.5i).
    ZeroLoyalty,
    /// Its controller kept another legendary permanent of the same name (CR 704.5j).
    LegendRule,
    /// An Aura was attached to nothing or to something it may not enchant (CR 704.5m).
    UnattachedAura,
    /// A battle had no defense counters (CR 704.5v).
    ZeroDefense,
    /// A newer Role of the same controller was on the same permanent (CR 704.5y).
    ExtraRole,
    /// A state-based action the event does not name.
    Unnamed,
}

/// Reads a zone cause, taking the bare `"StateBased"` of a log that names no rule as `StateBased(Unnamed)`.
fn zone_cause<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ZoneCause, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Cause(ZoneCause),
        Bare(Bare),
    }
    #[derive(Deserialize)]
    enum Bare {
        StateBased,
    }
    Ok(match Stored::deserialize(deserializer)? {
        Stored::Cause(cause) => cause,
        Stored::Bare(Bare::StateBased) => ZoneCause::StateBased(StateRule::Unnamed),
    })
}

/// A fact about the game; folding the full list over the initial setup yields the game state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Event {
    /// The starting player was picked at random.
    FirstPlayerChosen {
        /// Who goes first.
        player: PlayerId,
    },
    /// A library was shuffled into this order (bottom to top).
    LibraryShuffled {
        /// Owner of the library.
        player: PlayerId,
        /// The new order.
        order: Vec<ObjectId>,
    },
    /// The opening-hand procedure began.
    MulliganPhaseBegan,
    /// An opening hand was dealt from the top of the library.
    HandDealt {
        /// Who received it.
        player: PlayerId,
        /// Cards in draw order.
        cards: Vec<ObjectId>,
    },
    /// The player chose to mulligan, pending the rest of the table.
    MulliganDeclared {
        /// Who chose.
        player: PlayerId,
    },
    /// The hand was shuffled into the library.
    MulliganTaken {
        /// Who mulliganed.
        player: PlayerId,
        /// The shuffled library, bottom to top.
        library_order: Vec<ObjectId>,
    },
    /// The player kept their hand.
    MulliganKept {
        /// Who kept.
        player: PlayerId,
    },
    /// The player exiled their whole hand and drew as many cards from the top of their library by the ability of a card
    /// in it (Serum Powder, CR 103.5); they decide about the new hand as about the one before.
    R115HandExiled {
        /// Who exiled.
        player: PlayerId,
        /// The cards of the hand exiled.
        exiled: Vec<ObjectId>,
        /// The cards drawn, in draw order.
        drawn: Vec<ObjectId>,
    },
    /// Cards were put on the bottom of the library after keeping.
    CardsBottomed {
        /// Whose cards.
        player: PlayerId,
        /// Cards in the order placed (the last is the bottom).
        cards: Vec<ObjectId>,
    },
    /// Cards that begin the game on the battlefield were put there from the opening hand (CR 103.6).
    OpeningCardsPlayed {
        /// Whose cards.
        player: PlayerId,
        /// The cards, from the hand.
        cards: Vec<ObjectId>,
    },
    /// All players kept; the game proper begins.
    MulliganPhaseEnded,
    /// A new turn began.
    TurnBegan {
        /// Turn number.
        number: u32,
        /// The active player.
        active: PlayerId,
        /// Whether it is an extra turn: the latest extra turn added is taken, and the turn order goes on from the seat
        /// it was added after (CR 500.7).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        extra: bool,
    },
    /// A player gets an extra turn after the current one (CR 500.7); the latest added is taken first.
    ExtraTurnAdded {
        /// Who takes it.
        player: PlayerId,
        /// Whether its untap step is skipped.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        skip_untap: bool,
        /// Whether power-up abilities can't be activated during it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        no_power_up: bool,
        /// Whether damage can't be prevented during it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        r109_unpreventable: bool,
        /// The delayed trigger set up as it begins, for its end step; its id is given then.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        r6703_end: Option<Box<DelayedCreated>>,
    },
    /// The latest extra turn added is not taken, because its player has left the game or a rule skips it.
    ExtraTurnSkipped {
        /// Whose turn.
        player: PlayerId,
    },
    /// A player gains control of a spell or ability on the stack (CR 110.2): it resolves for them, and a permanent spell
    /// enters under their control.
    SpellControlGained {
        /// The spell or ability.
        object: ObjectId,
        /// Its new controller.
        player: PlayerId,
    },
    /// A player will control another during that player's next turn (CR 722): its decisions go to `by`.
    PlayerControlGained {
        /// The player controlled.
        player: PlayerId,
        /// The player who controls them.
        by: PlayerId,
        /// Whether the controlled player takes an extra turn after that turn.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        extra_turn: bool,
        /// The control lasts the player's next combat phase rather than their next turn.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        combat: bool,
    },
    /// The game restarted (CR 727): every card goes back to its owner's library (a commander to the command zone), tokens
    /// and other objects cease to exist, the players start over and the opening runs again with `by` as the starting
    /// player; `kept` stays in exile and enters the battlefield under `by`'s control once the opening is over.
    GameRestarted {
        /// The player whose effect restarted the game.
        by: PlayerId,
        /// The exiled cards left out of the new game.
        kept: Vec<ObjectId>,
    },
    /// Phases were added after the current one (CR 500.8): each is the step it begins with, in the order they begin,
    /// before any phase added earlier.
    PhasesAdded {
        /// The first step of each added phase.
        steps: Vec<Step>,
        /// Each entry is a step on its own, not a phase: it is followed by what comes after it, not by the rest of its
        /// phase ("additional upkeep steps after this phase").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
        lone: bool,
        /// The phases come after the first combat phase of the turn, which is still to begin or under way, instead of
        /// after the current phase ("After the first combat phase this turn, there's an additional combat phase").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
        r110_after_first_combat: bool,
    },
    /// A step was added right after the current one, before any step added earlier (`Effect::AddStep`).
    StepAdded {
        /// The step added.
        step: Step,
    },
    /// A step began.
    StepBegan {
        /// The step.
        step: Step,
    },
    /// A player had no legal option for an optional decision, so it was skipped.
    NothingToDo {
        /// The player.
        player: PlayerId,
        /// What they could not do.
        kind: NothingToDo,
    },
    /// A cast or activation a player began was taken back before it reached the stack: a step of its announcement could
    /// not be met. Nothing else changed.
    AnnouncementRewound {
        /// Who began it.
        player: PlayerId,
        /// The card or permanent it was of.
        object: ObjectId,
        /// Why it could not go on.
        reason: Unplayable,
        /// The detail of the reason, when the engine can name it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        why: Option<crate::decision::Why>,
        /// It was an activation of an ability of `object`, not a cast.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        activated: bool,
    },
    /// A player took back their last cast, activation or land play with the table's consent: the log before it is the
    /// game, and this note follows it. Nothing else changed.
    TookBack {
        /// Who took it back.
        player: PlayerId,
        /// What they took back.
        kind: crate::take_back::TakeBackKind,
        /// The card cast or played, or the source of the ability.
        name: String,
    },
    /// A player undid a mana ability they had just activated at priority, its mana unspent (`mana_undo.rs`): a note
    /// after the log was cut back to before it; the state is as if it never happened.
    ManaUndone {
        /// Who undid it.
        player: PlayerId,
        /// The source, untapped again.
        source: ObjectId,
    },
    /// A priority round started with this player.
    PriorityGranted {
        /// Who has priority.
        player: PlayerId,
    },
    /// A player passed priority.
    PriorityPassed {
        /// Who passed.
        player: PlayerId,
    },
    /// A land was played from a hand, or from exile where a permission lets its player.
    LandPlayed {
        /// Who played it.
        player: PlayerId,
        /// The land.
        object: ObjectId,
        /// The face of a card with several faces it was played as (the back face of a modal double-faced card).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        face: Option<u8>,
    },
    /// A spell was cast and put on the stack.
    SpellCast {
        /// The caster.
        player: PlayerId,
        /// The spell.
        object: ObjectId,
        /// What the caster announced: targets, modes, X and the like.
        #[serde(default, skip_serializing_if = "Choices::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Choices>", optional))]
        choices: Choices,
    },
    /// The top spell of the stack resolved.
    SpellResolved {
        /// The spell.
        object: ObjectId,
    },
    /// A permanent became tapped.
    PermanentTapped {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent became untapped.
    PermanentUntapped {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent phased out (CR 702.26): it is treated as though it does not exist until it phases in. Nothing
    /// leaves the battlefield and nothing triggers.
    PermanentPhasedOut {
        /// The permanent.
        object: ObjectId,
        /// The player who controlled it, who it phases in for during their untap step.
        controller: PlayerId,
        /// It went with the permanent it is attached to, and phases in with that permanent.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        indirect: bool,
        /// It phases out until this permanent leaves the battlefield, and phases in then and not in an untap step.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        until: Option<ObjectRef>,
        /// It phases in tapped ("Tap that creature as it phases in this way").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        tapped: bool,
        /// It phases out until this player rolls this number or less while rolling to visit their Attractions (Ferris
        /// Wheel), and phases in then and not in an untap step.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        r112_visit: Option<(PlayerId, u32)>,
    },
    /// A permanent phased in (CR 702.26): it exists again, as the same object, with everything it had.
    PermanentPhasedIn {
        /// The permanent.
        object: ObjectId,
    },
    /// Mana was added to a pool.
    ManaAdded {
        /// Whose pool.
        player: PlayerId,
        /// The mana.
        mana: ManaType,
        /// The producing permanent.
        source: Option<ObjectId>,
        /// What the mana may be spent on; left out means anything.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<SpendOn>>", optional))]
        only: Vec<SpendOn>,
        /// How long it stays in the pool; left out means until the step ends.
        #[serde(default, skip_serializing_if = "ManaLasts::is_step")]
        #[cfg_attr(feature = "ts", ts(as = "Option<ManaLasts>", optional))]
        lasts: ManaLasts,
    },
    /// Mana was spent from a pool.
    ManaSpent {
        /// Whose pool.
        player: PlayerId,
        /// The mana spent.
        mana: Vec<ManaType>,
        /// The part of it that was restricted, each also in `mana`; left out means none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Restricted>>", optional))]
        restricted: Vec<Restricted>,
    },
    /// Unspent mana disappeared at the end of a step, a phase or the turn.
    ManaPoolEmptied {
        /// Whose pool.
        player: PlayerId,
        /// The longest-lasting mana that went: all that lasts no longer than this; left out means the mana of the step.
        #[serde(default, skip_serializing_if = "ManaLasts::is_step")]
        #[cfg_attr(feature = "ts", ts(as = "Option<ManaLasts>", optional))]
        through: ManaLasts,
        /// The types a rule keeps in the pool ("you don't lose unspent red mana"); left out means none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ManaType>>", optional))]
        kept: Vec<ManaType>,
        /// The type the rest becomes instead of being lost ("that mana becomes colorless instead"); left out means it is lost.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        becomes: Option<ManaType>,
    },
    /// Unspent mana cost its owner life under the house rule of mana burn; the `LifeLost` of the same amount follows.
    ManaBurned {
        /// Whose pool.
        player: PlayerId,
        /// The mana that was left, which is the life lost.
        amount: u32,
    },
    /// Life was paid as (part of) a cost; it is lost, not damage (CR 119.4).
    LifePaid {
        /// Who paid.
        player: PlayerId,
        /// The life paid, at least 1.
        amount: u32,
    },
    /// A card was drawn.
    CardDrawn {
        /// Who drew.
        player: PlayerId,
        /// The card.
        object: ObjectId,
    },
    /// A draw was attempted from an empty library.
    DrewFromEmptyLibrary {
        /// Who tried.
        player: PlayerId,
    },
    /// A replacement effect took the place of a draw of the player (CR 614.1a): the draw counts as a time the player would
    /// have drawn a card ("the first time you would draw a card each turn").
    R109DrawReplaced {
        /// Who would have drawn.
        player: PlayerId,
    },
    /// A player's graveyard was shuffled into their library in place of losing to an empty library; the draws they
    /// could not make follow.
    LibraryRefilled {
        /// Owner of the library.
        player: PlayerId,
        /// The new order of the library, graveyard included (bottom to top).
        order: Vec<ObjectId>,
    },
    /// A card was discarded from hand.
    CardDiscarded {
        /// Who discarded.
        player: PlayerId,
        /// The card.
        object: ObjectId,
    },
    /// A card was cycled (CR 702.29): it is the discard that pays for cycling, logged after its `CardDiscarded`, so a
    /// cycling trigger and a discard trigger each read their own event.
    Cycled {
        /// Who cycled it.
        player: PlayerId,
        /// The card.
        object: ObjectId,
        /// The X of the cycling cost, which the trigger "when you cycle this card" reads.
        #[serde(default, skip_serializing_if = "is_zero")]
        x: u32,
    },
    /// Attackers were declared (possibly none).
    AttackersDeclared {
        /// The attacking player.
        player: PlayerId,
        /// Each attacker and its target.
        attackers: Vec<(ObjectId, AttackTarget)>,
    },
    /// A player of a team that shares its turn chose the attacks of their own creatures; the team declares every
    /// player's choice together once the last has chosen (CR 805, CR 508.1).
    AttackersChosen {
        /// The player who chose.
        player: PlayerId,
        /// Each of their attackers and its target.
        attackers: Vec<(ObjectId, AttackTarget)>,
        /// The attackers they exert.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exert: Vec<ObjectId>,
        /// The bands they form.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        bands: Vec<Vec<ObjectId>>,
        /// The creatures their attackers enlist, each (attacker, creature tapped) (CR 702.154a).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        r114_enlist: Vec<(ObjectId, ObjectId)>,
    },
    /// The attacks a team chose break a rule together, so each of its players chooses again.
    AttackChoicesRefused {
        /// Why the team's declaration is illegal.
        reason: String,
    },
    /// A seat of the boss's side aims its attack, or a spell or ability, at another hero than the one it eyes, for the
    /// reason given.
    FocusShifted {
        /// The seat of the boss's side.
        player: PlayerId,
        /// The hero it eyes.
        eyed: PlayerId,
        /// The hero its attack or targets go at instead.
        instead: PlayerId,
        /// Why.
        why: crate::spotlight_aim::AimWhy,
        /// The spell or ability whose targets go at `instead`; `None` for an attack.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        source: Option<ObjectId>,
    },
    /// Attackers declared together form a band (CR 702.22c).
    BandFormed {
        /// The attacking player.
        player: PlayerId,
        /// The members of the band.
        attackers: Vec<ObjectId>,
    },
    /// A defending player declared blockers (possibly none).
    BlockersDeclared {
        /// The defending player.
        player: PlayerId,
        /// Pairs of (blocker, attacker blocked).
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    /// An attacker's damage was divided among its blockers, or a blocker's among the attackers it blocks.
    DamageAssigned {
        /// The attacker.
        attacker: ObjectId,
        /// Damage per blocker.
        assignment: Vec<(ObjectId, u32)>,
    },
    /// Damage was dealt.
    DamageDealt {
        /// The source.
        source: ObjectId,
        /// The recipient.
        target: DamageTarget,
        /// Amount (never zero).
        amount: u32,
        /// The source had deathtouch as it dealt the damage (CR 702.2): a creature it hurts is marked for destruction.
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        deathtouch: bool,
        /// The source had wither or infect as it dealt the damage: a creature gets -1/-1 counters instead of damage, and with
        /// infect a player gets poison counters instead of losing life.
        #[serde(default, skip_serializing_if = "DamageForm::is_normal")]
        #[cfg_attr(feature = "ts", ts(as = "Option<DamageForm>", optional))]
        form: DamageForm,
        /// The poison counters the player dealt this combat damage also gets, from the toxic of the source (CR 702.164).
        #[serde(default, skip_serializing_if = "is_zero")]
        #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
        toxic: u32,
        /// The damage can't be prevented (CR 614.17): no prevention effect applies to it.
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        unpreventable: bool,
    },
    /// Combat ended and its state was cleared.
    CombatEnded,
    /// All damage marked on permanents was removed.
    DamageCleared,
    /// A creature was removed from combat (CR 506.4): it no longer attacks or blocks.
    RemovedFromCombat {
        /// The creature.
        object: ObjectId,
    },
    /// An attacking creature that no creature still blocks and only creatures that left combat had blocked this combat
    /// became unblocked (False Orders, Balduvian Warlord).
    R6400Unblocked {
        /// The attacker.
        attacker: ObjectId,
    },
    /// A player other than the active player chose which creatures attack this combat (CR 508.1a, "You choose which
    /// creatures attack this turn"); the active player then declares them, choosing what each attacks.
    R5081AttackersChosen {
        /// Who chose.
        chooser: PlayerId,
        /// The creatures that attack; none for no attack.
        attackers: Vec<ObjectId>,
    },
    /// A creature began to block an attacking creature without being declared (CR 509.1h): "then it blocks an
    /// attacking creature of your choice".
    R6400Blocks {
        /// The blocker.
        blocker: ObjectId,
        /// The attacker it blocks.
        attacker: ObjectId,
    },
    /// A creature began to block an attacking creature without blocking (CR 509.1h): put onto the battlefield blocking it,
    /// or moved there from the attacker it was blocking. Nothing triggers on it, and the attacker is blocked from then on.
    R115BlockMoved {
        /// The blocker.
        blocker: ObjectId,
        /// The attacker it stops blocking, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        from: Option<ObjectId>,
        /// The attacker it blocks.
        to: ObjectId,
    },
    /// Attacking creatures became blocked by an effect, with no creature blocking them (CR 509.1h).
    AttackersBlocked {
        /// The attackers, each not blocked before.
        attackers: Vec<ObjectId>,
    },
    /// A permanent regenerated instead of being destroyed (CR 701.19): it was tapped, lost its damage and left combat.
    Regenerated {
        /// The permanent.
        object: ObjectId,
    },
    /// All damage marked on a permanent was removed in place of its destruction (umbra armor, CR 702.89a).
    DamageRemoved {
        /// The permanent.
        object: ObjectId,
    },
    /// Damage a source would have dealt was prevented (CR 615): "whenever damage that would be dealt to you is prevented".
    DamagePrevented {
        /// The source of the damage.
        source: ObjectId,
        /// Who or what it would have been dealt to.
        target: DamageTarget,
        /// How much was prevented.
        amount: u32,
    },
    /// A shield that prevents a given amount of damage prevented some and has this much left (CR 615.7).
    ShieldUsed {
        /// The shield, one of the rule changes (they share the counter of continuous effects).
        id: EffectId,
        /// What it still prevents.
        left: u32,
    },
    /// A continuous effect was registered.
    EffectCreated {
        /// The effect.
        effect: Box<ContinuousEffect>,
    },
    /// A continuous effect ended and was removed.
    EffectExpired {
        /// The effect.
        id: EffectId,
    },
    /// A change to a rule of the game was registered.
    RuleCreated {
        /// The change.
        effect: Box<RuleEffect>,
    },
    /// A change to a rule of the game ended and was removed.
    RuleExpired {
        /// The change.
        id: EffectId,
    },
    /// A creature was put into its owner's graveyard by state-based actions.
    CreatureDied {
        /// The creature.
        object: ObjectId,
        /// Why.
        cause: DeathCause,
    },
    /// A player lost the game.
    PlayerLost {
        /// Who lost.
        player: PlayerId,
        /// Why.
        reason: LossReason,
    },
    /// A player at 0 life became a Spirit ([`crate::spirits`]) and got its Spirit emblem.
    BecameSpirit {
        /// Who.
        player: PlayerId,
        /// The Spirit emblem's new object.
        emblem: ObjectId,
    },
    /// A Spirit returned to life ([`crate::spirits`]): it is a living player again with this life total; its haunt
    /// emblem left the game just before.
    SpiritRevived {
        /// Who.
        player: PlayerId,
        /// The life total it returns with.
        life: i32,
    },
    /// A boss seat's next phase began ([`crate::phases`]): its emblem is a new object in their command zone.
    PhaseBegan {
        /// Whose phase.
        player: PlayerId,
        /// The phase emblem's new object.
        emblem: ObjectId,
    },
    /// A seat set its next scheme in motion as its turn began ([`crate::schemes`]): the scheme is a new emblem in their
    /// command zone (the one before it left the game just before).
    SchemeSetInMotion {
        /// Whose scheme.
        player: PlayerId,
        /// The scheme emblem's new object.
        emblem: ObjectId,
    },
    /// An object left the game because its owner did.
    ObjectLeftGame {
        /// The object.
        object: ObjectId,
    },
    /// The game ended.
    GameEnded {
        /// The result.
        result: GameResult,
    },
    /// A loop of mandatory actions would go on forever (CR 104.4b, 732.4): the game is a draw, and `GameEnded` follows.
    LoopDrawn {
        /// The names of the objects whose abilities make up the loop.
        cards: Vec<String>,
    },
    /// A loop ran as a shortcut (CR 732): the summary of the events right before it.
    LoopRepeated(Box<crate::loops::LoopSummary>),
    /// A loop was found and its driver is asked how often to repeat it (CR 732.2a); the question waits until the next
    /// event.
    LoopOffered(Box<crate::loops::LoopOffer>),
    /// The player asked about a loop plays on by hand instead (`LoopAnswer::Stop`).
    LoopStopped {
        /// Who was asked.
        player: PlayerId,
    },
    /// An ability was put on the stack, activated by its controller or triggered.
    AbilityPutOnStack(Box<PutOnStack>),
    /// A player activated an ability; its costs are paid and the ability goes on the stack next.
    AbilityActivated {
        /// Who activated it.
        player: PlayerId,
        /// The object it comes from.
        source: ObjectRef,
        /// Its index among the rules of the source's card; 0 for a granted ability; its place among those its keyword
        /// adds for one a keyword added.
        ability: usize,
        /// Where the ability comes from: the card, an effect that granted it, or a keyword.
        #[serde(default, skip_serializing_if = "AbilitySource::is_printed")]
        #[cfg_attr(feature = "ts", ts(as = "Option<AbilitySource>", optional))]
        origin: AbilitySource,
    },
    /// A player activated a mana ability (CR 605.3b); it uses no stack, so the log alone keeps the activation for the
    /// abilities that trigger on it.
    ManaAbilityActivated {
        /// Who activated it.
        player: PlayerId,
        /// The object it comes from.
        source: ObjectRef,
        /// The source was tapped to pay its cost ("tapped a land for mana").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
        tapped: bool,
        /// The sources of the mana spent on its cost, for one that costs mana ("if mana from a Treasure was spent to
        /// activate it"); empty otherwise.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(
            feature = "ts",
            ts(optional, as = "Option<Vec<mtg_cards::spend::SpentFrom>>")
        )]
        r113_spent_from: Vec<mtg_cards::spend::SpentFrom>,
        /// How many mana the source produced with it ("the amount of mana this creature produced").
        #[serde(default, skip_serializing_if = "is_zero")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<u32>"))]
        r114_made: u32,
    },
    /// A triggered ability triggered and waits to be put on the stack.
    AbilityTriggered(Box<Triggered>),
    /// A player put the triggered abilities they would put on the stack together in the order they go on in.
    TriggersOrdered {
        /// Who ordered them.
        player: PlayerId,
        /// For each place on the stack, bottom first, the position among the triggers waiting of the one that goes there.
        order: Vec<usize>,
    },
    /// A waiting triggered ability was removed without going on the stack: it had no legal target (CR 603.3d), or its
    /// controller or source had left the game.
    TriggerRemoved {
        /// The object with the ability, as it was.
        source: ObjectRef,
        /// Its index among the rules of the source's card; 0 for a granted ability; its place among those its keyword
        /// adds for one a keyword added.
        ability: usize,
        /// Where the ability comes from: the card, an effect that granted it, or a keyword.
        #[serde(default, skip_serializing_if = "AbilitySource::is_printed")]
        #[cfg_attr(feature = "ts", ts(as = "Option<AbilitySource>", optional))]
        origin: AbilitySource,
        /// The delayed trigger it is one of, for a delayed triggered ability.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        delayed: Option<DelayedId>,
        /// What it triggered on.
        cause: Box<Cause>,
    },
    /// A delayed triggered ability was set up (CR 603.7).
    DelayedTriggerCreated(Box<DelayedCreated>),
    /// A delayed triggered ability ran out without triggering.
    DelayedTriggerExpired {
        /// The delayed trigger.
        id: DelayedId,
    },
    /// The top object of the stack began to resolve.
    ResolutionBegan {
        /// The spell or ability.
        object: ObjectId,
    },
    /// A player answered a question put during the resolution.
    ChoiceMade {
        /// Who answered.
        player: PlayerId,
        /// The answer.
        choice: Choice,
    },
    /// One step of the resolution is done: the effect at the old position happened.
    StepDone {
        /// Where the next step runs.
        position: Vec<usize>,
        /// Names the step bound, with what they stand for.
        bound: Vec<(String, Vec<Target>)>,
        /// Objects the step moved away, as they were.
        snapshots: Vec<Snapshot>,
    },
    /// A replacement effect run in the middle of the resolution reached a spell cast (CR 608.2g): the cast and the steps
    /// after it go on as part of the resolving spell or ability once its step is done, before its next effect.
    R115Continued {
        /// The object with the replacement effect.
        source: ObjectRef,
        /// Who controls the replacement effect.
        controller: PlayerId,
        /// The effects of the replacement.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        effects: Vec<mtg_cards::effect::Effect>,
        /// Where the steps go on: the cast.
        position: Vec<usize>,
        /// Names the steps before it bound, with what they stand for.
        bound: Vec<(String, Vec<Target>)>,
        /// Objects the steps before it moved away, as they were.
        snapshots: Vec<Snapshot>,
    },
    /// A triggered ability whose effects all hang on conditions found every one of them false as it began to resolve, so
    /// it does nothing; `ResolutionEnded` follows.
    ConditionsFailed {
        /// The ability.
        object: ObjectId,
        /// The conditions in the card's words, after "if"; those the writer cannot word are left out.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
        conditions: Vec<String>,
    },
    /// The resolution of a spell or ability is over.
    ResolutionEnded {
        /// The spell or ability.
        object: ObjectId,
        /// How it ended.
        outcome: ResolutionOutcome,
    },
    /// A step of a spell or ability of a card that runs on a player's read failed, so the rest of it is left to the table: the
    /// steps before it stand and the resolution ends right after (`docs/engine.md`, Fail-soft resolution).
    AbilityLeftToTable(Box<LeftToTable>),
    /// The event a triggered line the table plays by hand waits for happened, so its controller applies it now
    /// (`docs/engine.md`, By-hand lines): a note, no state change.
    ByHandDue(Box<crate::triggers::ByHandDue>),
    /// An object moved from one zone to another.
    ZoneChanged {
        /// The object.
        object: ObjectId,
        /// Where it was.
        from: Zone,
        /// Where it went.
        to: Zone,
        /// Why.
        #[serde(deserialize_with = "zone_cause")]
        cause: ZoneCause,
    },
    /// A player gained life.
    LifeGained {
        /// Who gained it.
        player: PlayerId,
        /// How much, at least 1.
        amount: u32,
    },
    /// A player lost life (damage is its own event).
    LifeLost {
        /// Who lost it.
        player: PlayerId,
        /// How much, at least 1.
        amount: u32,
    },
    /// Counters were put on or removed from an object or player.
    CountersChanged(CountersChanged),
    /// A token was created.
    TokenCreated(TokenCreated),
    /// A player got an emblem (CR 114).
    EmblemCreated(crate::w6b_emblems::EmblemCreated),
    /// A player looked at cards in a library.
    LibraryLooked(LibraryLooked),
    /// A player looked at a hand.
    HandLooked(HandLooked),
    /// A player found cards by searching.
    CardsFound(CardsFound),
    /// A player scried.
    Scried(Scried),
    /// A player surveilled.
    Surveilled(Surveilled),
    /// A player revealed cards.
    Revealed(Revealed),
    /// Cards of a public zone, or the cards a discard at random takes from a hand (CR 701.9b), were picked at random
    /// with the next substream of the generator (CR 701.24 for a shuffle, CR 608.2c for the choice): the cards picked,
    /// in the order they were drawn. A discard names only the cards it then reveals.
    ChosenAtRandom {
        /// The cards picked.
        cards: Vec<ObjectId>,
    },
    /// Players were picked at random with the next substream of the generator ("choose an opponent at random"): the
    /// players picked, in the order they were drawn.
    PlayersChosenAtRandom {
        /// The players picked.
        players: Vec<PlayerId>,
    },
    /// A kind of counter was picked at random with the next substream of the generator ("choose a kind of counter at
    /// random"): the kind picked for the object it goes on.
    KindChosenAtRandom {
        /// The object the counter goes on.
        object: ObjectId,
        /// The kind picked.
        kind: mtg_cards::value::CounterKind,
    },
    /// A card was conjured into a zone (Alchemy).
    CardConjured(crate::conjure::CardConjured),
    /// A player seeks (Alchemy): `count` random cards of their library that match go to their hand, chosen with the
    /// next substream of the generator; the `ZoneChanged` events that move them follow in the same batch.
    Sought {
        /// Who seeks.
        player: PlayerId,
        /// How many cards were picked.
        count: u32,
    },
    /// Cards of a library were put on its bottom.
    LibraryBottomed {
        /// Whose library.
        player: PlayerId,
        /// The cards, top to bottom as they lie after the move: the last is the bottom card.
        cards: Vec<ObjectId>,
        /// The order was drawn at random, with the next substream of the generator (CR 702.85a).
        #[serde(default)]
        random: bool,
    },
    /// Cards of a library were put beneath its top `below` cards (on the bottom when it has fewer).
    LibraryInserted {
        /// Whose library.
        player: PlayerId,
        /// The cards, top to bottom as they lie after the move.
        cards: Vec<ObjectId>,
        /// How many cards of the library stay above them.
        below: u32,
    },
    /// A card an effect put onto the battlefield entered tapped or under another player's control; it follows the
    /// `ZoneChanged` that moved it, in the same batch.
    Entered {
        /// The permanent.
        object: ObjectId,
        /// Whether it entered tapped.
        tapped: bool,
        /// Who controls it.
        controller: PlayerId,
        /// What its controller chose as it entered, which the permanent remembers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        chosen: Option<Chosen>,
    },
    /// An Aura, Equipment or Fortification became attached to a permanent (CR 701.3); it takes a new timestamp (CR 613.7a).
    Attached {
        /// The attachment.
        object: ObjectId,
        /// The permanent it is attached to.
        to: ObjectId,
    },
    /// An attachment chose a value as it became attached ("As this Equipment becomes attached to a creature, choose a
    /// color."): it replaces any value it chose before.
    ChosenAsAttached {
        /// The attachment.
        object: ObjectId,
        /// What it chose.
        chosen: Chosen,
    },
    /// A resolving spell or ability made players guess or group cards or permanents into piles (The Celestial Toymaker's
    /// count of them this turn).
    R112Grouped {
        /// The spell or ability.
        object: ObjectId,
    },
    /// A permanent keeps a value its ability chose as its own choice, public ("II — Choose a card name.").
    R111ChoiceKept {
        /// The permanent.
        object: ObjectId,
        /// What it keeps.
        chosen: Chosen,
    },
    /// A permanent's controller chose a value secretly, which the permanent keeps (CR 706, "secretly choose an
    /// opponent"); only that player sees what it is until a cost reveals it.
    R110ChoseSecretly {
        /// The permanent.
        object: ObjectId,
        /// What was chosen.
        chosen: Chosen,
    },
    /// A permanent's secret choice was revealed to every player ("Reveal the player you chose").
    R110ChoiceRevealed {
        /// The permanent.
        object: ObjectId,
        /// The choice revealed.
        chosen: Chosen,
    },
    /// An Aura became attached to a player (CR 303.4); it takes a new timestamp (CR 613.7a).
    AttachedToPlayer {
        /// The Aura.
        object: ObjectId,
        /// The player it enchants.
        player: PlayerId,
    },
    /// A creature that was not declared as an attacker joined the attack, attacking this player or planeswalker (CR 508.4).
    JoinedAttack {
        /// The creature.
        object: ObjectId,
        /// What it attacks.
        target: AttackTarget,
    },
    /// An attacking creature now attacks this player, planeswalker or battle instead (an effect reselected it); it is
    /// still the creature that attacked as declared.
    AttackRedirected {
        /// The creature.
        object: ObjectId,
        /// What it attacks now.
        target: AttackTarget,
    },
    /// A permanent stopped being attached to what it was attached to.
    Unattached {
        /// The permanent.
        object: ObjectId,
    },
    /// Two creatures became paired (CR 702.95): each is the partner of the other until a `Unpaired`.
    Paired {
        /// The creature that was paired, a soulbond creature or the creature that entered.
        object: ObjectId,
        /// The creature it was paired with.
        with: ObjectId,
    },
    /// Two paired creatures stopped being paired (CR 702.95e): one left the battlefield, stopped being a creature or changed
    /// controller. `with` may have left the battlefield.
    Unpaired {
        /// One creature of the pair.
        object: ObjectId,
        /// The other.
        with: ObjectId,
    },
    /// A transforming double-faced permanent was turned over (CR 701.28): the same object with the other face up.
    Transformed {
        /// The permanent.
        object: ObjectId,
        /// The face now up: none for the front face, the position of the back face otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        face: Option<u8>,
    },
    /// A flip card on the battlefield was flipped (CR 710): the same object as its bottom half until it leaves.
    Flipped {
        /// The permanent.
        object: ObjectId,
    },
    /// A prepare card on the battlefield became prepared: while it is, its controller may cast a copy of its spell face.
    Prepared {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent stopped being prepared as its controller cast a copy of its spell face.
    Unprepared {
        /// The permanent.
        object: ObjectId,
    },
    /// A creature on the battlefield became suspected (CR 701.60): while it is, it has menace and can't block.
    Suspected {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent stopped being suspected (CR 701.60d).
    Unsuspected {
        /// The permanent.
        object: ObjectId,
    },
    /// A Case on the battlefield became solved (CR 719.3): it stays solved until it leaves.
    Solved {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent became harnessed: its ∞ ability works from now on.
    Harnessed {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent became a copy of another object (CR 707.2), with the copiable values of the copy effect.
    BecameCopy(BecameCopy),
    /// A spell was copied (CR 707.10): the copy is put on the stack above it, not cast.
    SpellCopied(Box<SpellCopy>),
    /// A card in exile was copied (CR 707.12): the copy is a token card in exile that its owner may cast, and it ceases to
    /// exist once it is anywhere but on the stack.
    CardCopied(Box<CardCopy>),
    /// A card put onto the battlefield face down took the characteristics an effect listed (CR 708.2).
    R6008Listed(Box<crate::r6008_face_down::R6008Listed>),
    /// A card lying face down in exile was turned face up, for every seat to see (CR 406.3).
    R406TurnedUp {
        /// The card.
        object: ObjectId,
    },
    /// A card exiled from the stack keeps the spell it was, for a later put back (`Effect::R707ExileSpell`, CR 707.10).
    R707Kept(Box<crate::r707_spells::R707Kept>),
    /// A player put a card from exile onto the stack as a copy of the spell it kept (CR 707.10): not cast.
    R707PutBack {
        /// The card, as it is in exile.
        object: ObjectId,
        /// Who controls the spell.
        player: PlayerId,
    },
    /// A copy of a card ceased to exist in a zone other than the stack (CR 704.5e).
    CopyCeased {
        /// The copy.
        object: ObjectId,
    },
    /// An activated or triggered ability was copied (CR 707.10): the copy is put on the stack above it, not activated or triggered.
    AbilityCopied(Box<AbilityCopy>),
    /// The targets of a spell or ability on the stack changed (CR 115.7).
    TargetsChanged(Box<TargetsChanged>),
    /// A source remembered objects, or forgot a slot (`Effect::Remember`, `Effect::Forget`; the exiles of a permanent too).
    Remembered(Box<crate::memory::Remembered>),
    /// A permanent's copy effect that lasted until end of turn, or until a player's next turn, ended.
    CopyEnded {
        /// The permanent.
        object: ObjectId,
        /// The player whose next turn began: the copies until that turn end; none for those until end of turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        next_turn_of: Option<PlayerId>,
        /// The object whose staying attached kept the copy going ("for as long as this Equipment remains attached to it").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        attached_by: Option<ObjectRef>,
        /// The object whose staying tapped kept the copy going ("for as long as that creature remains tapped").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        r6104_tapped: Option<ObjectRef>,
        /// The permanent whose static ability made the copy ("enchanted creature is a copy of ...", `R707CopyOf`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        r707_static: Option<ObjectRef>,
        /// The copy lasted until the next end step, which began ("until the next end step").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        r513_end_step: bool,
    },
    /// A player became the monarch (CR 724).
    MonarchChanged {
        /// The new monarch.
        player: PlayerId,
    },
    /// The Ring tempted a player (CR 701.54): their Ring level rose to `level`, and they chose `bearer`, if they controlled a creature.
    RingTempted {
        /// The tempted player.
        player: PlayerId,
        /// Their Ring level now.
        level: u8,
        /// The creature they chose as their Ring-bearer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        bearer: Option<ObjectId>,
    },
    /// A player's Ring-bearer stopped being it (CR 701.54c): another player gained control of it.
    RingBearerLost {
        /// The player whose creature it was.
        player: PlayerId,
    },
    /// A player's speed became `speed` (CR 702.179): start your engines! started it, or an opponent's life loss on their
    /// turn raised it.
    SpeedChanged {
        /// Whose speed.
        player: PlayerId,
        /// The new speed, 1 to 4.
        speed: u8,
    },
    /// A player got the city's blessing (CR 702.131): a designation they keep for the rest of the game.
    CitysBlessingGained {
        /// Who got it.
        player: PlayerId,
    },
    /// A player got an enduring story (storied): a designation they keep for the rest of the game.
    EnduringStoryGained {
        /// Who got it.
        player: PlayerId,
    },
    /// A player ventured into the dungeon (CR 701.49).
    Ventured(Box<crate::w11d_dungeon::W11dVentured>),
    /// A player completed a dungeon, which leaves the game (CR 309.7).
    DungeonCompleted {
        /// Who completed it.
        player: PlayerId,
        /// The dungeon.
        object: ObjectId,
    },
    /// A player took the initiative (CR 725); `object` stands for it in their command zone.
    InitiativeTaken {
        /// Who took it.
        player: PlayerId,
        /// The initiative's object.
        object: ObjectId,
    },
    /// A player learned (CR 701.48): the discard and the draw are the events that follow it.
    Learned {
        /// Who learned.
        player: PlayerId,
    },
    /// A player collected evidence (CR 701.59): the cards it exiled are the moves that come before it.
    EvidenceCollected {
        /// Who collected it.
        player: PlayerId,
    },
    /// A player bent an element (Avatar): paid a waterbend cost, or an earthbend, firebending ability or airbend was done.
    Bent {
        /// Who bent it.
        player: PlayerId,
        /// The element.
        element: Bending,
    },
    /// A player chose the card type they untap during their untap step (`GameRule::R112UntapChosenType`, CR 502.3).
    R112UntapTypeChosen {
        /// Who chose.
        player: PlayerId,
        /// Artifact, creature or land.
        kind: mtg_core::card::CardType,
    },
    /// A player paid to ignore the effects of a source until end of turn (`GameRule::PayToIgnore`, a special action).
    IgnoredEffect {
        /// Who paid.
        player: PlayerId,
        /// The source whose effects they ignore.
        source: ObjectRef,
    },
    /// A player investigated (CR 701.16a): the Clue token is the creation before it.
    Investigated {
        /// Who investigated.
        player: PlayerId,
    },
    /// A player discovered (CR 701.57): the exile, cast or move to the hand are the events before it.
    Discovered {
        /// Who discovered.
        player: PlayerId,
        /// The discover value.
        value: u32,
        /// The card discovered, when one was found and still exists.
        card: Option<ObjectId>,
    },
    /// A player manifested dread (CR 701.62): the look, the move to the graveyard and the manifest are the events before it.
    DreadManifested {
        /// Who manifested dread.
        player: PlayerId,
        /// The card they put into their graveyard this way, if any.
        card: Option<ObjectId>,
    },
    /// A player paid or didn't pay an upkeep cost of a permanent: its cumulative upkeep (CR 702.24a) or its echo cost
    /// (CR 702.30a); the payment or the sacrifice is the events around it.
    UpkeepSettled {
        /// Who paid or didn't.
        player: PlayerId,
        /// The permanent whose cost it was.
        object: ObjectId,
        /// The cost was an echo cost, else a cumulative upkeep.
        echo: bool,
        /// It was paid.
        paid: bool,
        /// The mana spent on a paid cumulative upkeep, in the order spent ("for each {B} or {R} spent this way");
        /// left out means none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ManaType>>", optional))]
        r119_mana: Vec<ManaType>,
    },
    /// A creature mentored a creature (CR 702.134a): its mentor ability put a +1/+1 counter on the creature, the event
    /// before it.
    R6401Mentored {
        /// The controller of the mentor ability.
        player: PlayerId,
        /// The creature that mentored.
        mentor: ObjectId,
        /// The creature it mentored.
        mentored: ObjectId,
    },
    /// A player gave a gift (CR 702.174): the gift itself is the events before it.
    GiftGiven {
        /// Who gave it.
        player: PlayerId,
        /// The player it was given to.
        to: PlayerId,
    },
    /// The counters the `CountersChanged` events after it in its batch put are put by this player, not by the controller of
    /// the resolving spell or ability (CR 122.6, "they put the same number and kind of counters"): replacement effects and
    /// triggers that care who puts counters see this player.
    R115CountersPutBy {
        /// Who puts the counters.
        player: PlayerId,
    },
    /// A player foraged (CR 701.61): the Food sacrificed or the cards exiled are the moves that come before it.
    Foraged {
        /// Who foraged.
        player: PlayerId,
    },
    /// A player rolled a die (CR 706): the number came from the game's random stream, which the roll moved on by one.
    DieRolled {
        /// Who rolled.
        player: PlayerId,
        /// How many faces the die has.
        sides: u32,
        /// The result: the number rolled, from 1 to `sides`, or that number changed (CR 706.2).
        result: u32,
        /// The natural result, the number the die showed, when an effect changed it into `result` (CR 706.2).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        natural: Option<u32>,
    },
    /// A player changed the result of a die they rolled through a permanent's rule (`Rule::R6402ChangeRoll`): it paid the
    /// cost and counts as used this turn. The roll is logged after it with the changed result.
    R6402DieChanged {
        /// Who rolled.
        player: PlayerId,
        /// The permanent whose rule changed the result.
        source: ObjectRef,
        /// The result before the change.
        from: u32,
        /// The result after it.
        to: u32,
    },
    /// A player rerolled dice of the roll they just made through a permanent's rule (`Rule::R706Reroll`, CR 706.2b): the
    /// cost is paid in the same batch and the rule counts as used this turn. Each reroll spends a substream of the
    /// generator; the roll is logged after it with the new results.
    R706DiceRerolled {
        /// Who rolled.
        player: PlayerId,
        /// The permanent whose rule rerolled the dice.
        source: ObjectRef,
        /// The results rerolled, before the reroll.
        from: Vec<u32>,
    },
    /// A player chose which roll of another player's roll to ignore (`Rule::R706IgnoreChosen`, CR 706.6): the ignored
    /// roll never happened and triggers nothing, it spends the substream it came from, and the rule change ends.
    R706DieIgnored {
        /// Who rolled.
        roller: PlayerId,
        /// Who chose.
        chooser: PlayerId,
        /// The rule change that added the die.
        effect: EffectId,
        /// The result ignored.
        result: u32,
    },
    /// The turn-based roll to visit Attractions asks questions (CR 505.5, 706.2b): the step waits for the answers, which
    /// the game keeps until the roll is logged.
    R706VisitAsks {
        /// Who rolls.
        player: PlayerId,
    },
    /// Each pile a defending player made is assigned to an attacker at random (`Rule::R509BlockPiles`), with the next
    /// substream of the generator: each attacker with the creatures of the pile it got. The blocks follow.
    R509PilesAssigned {
        /// The defending player.
        player: PlayerId,
        /// Each attacker and its pile.
        piles: Vec<(ObjectId, Vec<ObjectId>)>,
    },
    /// A player assigned their creatures that were in no sector to sectors of space sculptor (`p170a_sectors`), as a
    /// state-based action.
    P170aSectorsAssigned {
        /// The player who assigned them, their controller.
        player: PlayerId,
        /// Each creature and its sector.
        sectors: Vec<(ObjectId, crate::p170a_sectors::Sector)>,
    },
    /// Every sector of space sculptor is lost: no permanent with space sculptor is left on the battlefield.
    P170aSectorsLost,
    /// A player put the cards of their graveyard in an order they chose (CR 404.2).
    R111GraveyardOrdered {
        /// Whose graveyard.
        player: PlayerId,
        /// Its cards in the new order, bottom to top.
        order: Vec<ObjectId>,
    },
    /// A player's Attraction deck was shuffled (CR 103.3a): its cards in the new order, top card last.
    AttractionDeckShuffled {
        /// Whose deck.
        player: PlayerId,
        /// The cards, top card last.
        order: Vec<ObjectId>,
    },
    /// A player rolled to visit their Attractions (CR 701.52a): the result of the `DieRolled` before it and the
    /// Attractions with that number lit up, each visited.
    RolledToVisit {
        /// Who rolled.
        player: PlayerId,
        /// The number rolled.
        result: u32,
        /// The Attractions visited.
        visited: Vec<ObjectId>,
    },
    /// A player claimed the prize of an Attraction: its prize abilities trigger.
    PrizeClaimed {
        /// Who claimed it.
        player: PlayerId,
        /// The Attraction.
        attraction: ObjectId,
    },
    /// The sticker sheets a seat plays with were chosen at random from the ones it brought (CR 103.2d), which moved
    /// the game's random stream on by one.
    StickerSheetsChosen {
        /// Whose sheets.
        player: PlayerId,
        /// The sheets, as indexes into the ones the seat brought, in that order.
        sheets: Vec<usize>,
    },
    /// A player put a sticker of one of their sheets on a permanent they own (CR 123); the tickets paid are the
    /// `CountersChanged` before it and its change the `EffectCreated` after it.
    StickerPut {
        /// Whose sticker.
        player: PlayerId,
        /// The permanent.
        object: ObjectId,
        /// The sheet, as an index into the sheets the player brought.
        sheet: usize,
        /// The sticker, as an index among the stickers of the sheet.
        sticker: usize,
    },
    /// A player flipped a coin (CR 705): the flip came from the game's random stream, which it moved on by one.
    CoinFlipped {
        /// Who flipped.
        player: PlayerId,
        /// The flip was won: the call matched the side the coin came up (CR 705.1).
        won: bool,
        /// The side the player called.
        #[serde(default)]
        call: CoinSide,
        /// The side the coin came up: the coin kept when the player flipped more than one.
        #[serde(default)]
        side: CoinSide,
        /// Every coin flipped for this flip when a rule made it more than one (Krark's Thumb, CR 705.3), in the order
        /// they came up; empty for a single coin.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CoinSide>>", optional))]
        coins: Vec<CoinSide>,
    },
    /// A player proliferated (CR 701.34): the counters it gave are the `CountersChanged` events that follow it.
    Proliferated {
        /// Who proliferated.
        player: PlayerId,
    },
    /// A creature explored (CR 701.44): the card it revealed is the `Revealed` that follows, what became of it the
    /// moves and counters after that.
    Explored {
        /// The creature that explored.
        object: ObjectId,
        /// Its controller, who revealed the card.
        player: PlayerId,
        /// Whether the card revealed was a land card; none when the library was empty and nothing was revealed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        land: Option<bool>,
    },
    /// A player amassed (CR 701.47): the counters and the Army token are the events around it.
    Amassed {
        /// The Army that got the counters.
        object: ObjectId,
        /// Its controller, who amassed.
        player: PlayerId,
    },
    /// A permanent devoured as it entered (CR 702.82): the sacrifices are the events before it, and its abilities read
    /// the number while it stays on the battlefield.
    Devoured {
        /// The permanent that entered.
        object: ObjectId,
        /// How many permanents it devoured.
        count: u32,
        /// The permanents it devoured, by the ids they had on the battlefield; their sacrifices log them as they were.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        devoured: Vec<ObjectId>,
    },
    /// A creature became monstrous (CR 701.37): the permanent keeps the designation while it stays on the battlefield;
    /// the counters are the event before it.
    Monstrous {
        /// The creature that became monstrous.
        object: ObjectId,
        /// Its controller.
        player: PlayerId,
        /// The X of the monstrosity, which a trigger on becoming monstrous reads (CR 701.37c).
        #[serde(default, skip_serializing_if = "is_zero")]
        x: u32,
    },
    /// A creature became renowned (CR 702.112): the permanent keeps the designation while it stays on the battlefield;
    /// the counters are the event before it.
    Renowned {
        /// The creature that became renowned.
        object: ObjectId,
        /// Its controller.
        player: PlayerId,
    },
    /// A creature enlisted a creature (CR 702.154a): its enlist tapped the creature, the event before it.
    Enlisted {
        /// The creature that enlisted.
        object: ObjectId,
        /// The creature tapped for it.
        enlisted: ObjectId,
        /// The controller of the enlisting creature.
        player: PlayerId,
    },
    /// A creature fought another (CR 701.14): one event for each creature of a fight, after the damage of the fight.
    Fought {
        /// The creature that fought.
        object: ObjectId,
        /// The creature it fought.
        other: ObjectId,
    },
    /// A creature exploited a creature (CR 702.110b): its exploit ability resolved and the sacrifice is the event
    /// before it.
    Exploited {
        /// The creature whose exploit ability resolved.
        object: ObjectId,
        /// The creature sacrificed.
        sacrificed: ObjectId,
        /// The controller of the exploit ability, who sacrificed.
        player: PlayerId,
    },
    /// A Vehicle became crewed or a Mount saddled (CR 702.122, 702.171): its crew or saddle ability resolved; the type
    /// change is the effect before it.
    Crewed {
        /// The Vehicle or Mount.
        object: ObjectId,
        /// Its controller, who activated the ability.
        player: PlayerId,
        /// The creatures tapped to pay the cost, in the order they were picked.
        creatures: Vec<ObjectId>,
        /// The ability was saddle: the Mount became saddled.
        #[serde(default, skip_serializing_if = "is_false")]
        saddle: bool,
    },
    /// A Class became a higher level (CR 716.2): the permanent keeps the level while it stays on the battlefield.
    ClassLevelGained {
        /// The Class that gained the level.
        object: ObjectId,
        /// Its controller.
        player: PlayerId,
        /// The level it is now.
        level: u32,
    },
    /// A face-down permanent was turned face up (CR 702.37e), a special action: it shows its card and has its characteristics and
    /// abilities from now on. The counter of a megamorph is the event before it.
    TurnedFaceUp {
        /// The permanent that was turned face up.
        object: ObjectId,
        /// Its controller, who turned it.
        player: PlayerId,
        /// The X of the morph, megamorph or disguise cost paid, which a trigger on turning it face up reads (CR 107.3).
        #[serde(default, skip_serializing_if = "is_zero")]
        x: u32,
    },
    /// A permanent was turned face down by an effect (CR 708.2a): it is a face-down 2/2 from now on.
    TurnedFaceDown {
        /// The permanent that was turned face down.
        object: ObjectId,
        /// Its controller, who turned it.
        player: PlayerId,
        /// It is a 2/2 Horror creature (`FaceDown::R6701Horror`) rather than a plain 2/2 creature.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        horror: bool,
    },
    /// A commander's owner chose its color before the game began ("If ~ is your commander, choose a color before the game
    /// begins."): the card is that color in every zone.
    CommanderColorChosen {
        /// Who chose.
        player: PlayerId,
        /// The commander.
        object: ObjectId,
        /// The color.
        color: mtg_core::color::Color,
    },
    /// An emblem that held abilities a player had until end of turn ceased to exist in the cleanup step.
    R6051EmblemEnded {
        /// The emblem.
        object: ObjectId,
    },
    /// An owner circled colors for a card before the game began ("As you create your deck, circle two of the colors
    /// below."): public, and read by the card's mana ability in every zone.
    ColorsCircled {
        /// Who circled them.
        player: PlayerId,
        /// The card.
        object: ObjectId,
        /// The colors circled.
        colors: Vec<mtg_core::color::Color>,
    },
    /// A player revealed the first card they drew this turn as they drew it, through the `RevealFirstDraw` rules of
    /// these sources; a `Revealed` of the card comes just before it. Abilities "whenever you reveal ... this way" of
    /// those sources trigger on it.
    FirstDrawRevealed {
        /// Who revealed it.
        player: PlayerId,
        /// The card.
        card: ObjectId,
        /// The permanents whose rules revealed it.
        sources: Vec<ObjectId>,
    },
    /// A player looked at a face-down permanent by an effect (CR 708.5): they may identify it while it stays that object.
    FaceDownLooked {
        /// The permanent looked at.
        object: ObjectId,
        /// Who looked.
        player: PlayerId,
    },
    /// A door of a Room was unlocked (CR 709.5e): by the special action of its controller or by an effect. The
    /// permanent has that door's characteristics from now on.
    DoorUnlocked {
        /// The Room.
        object: ObjectId,
        /// The door, as an index into the card's faces.
        door: u8,
        /// Who unlocked it.
        player: PlayerId,
    },
    /// An unlocked door of a Room was locked by an effect (CR 709.5): the permanent loses that door's characteristics.
    DoorLocked {
        /// The Room.
        object: ObjectId,
        /// The door, as an index into the card's faces.
        door: u8,
        /// Who locked it.
        player: PlayerId,
    },
    /// A mutating creature spell merged with a creature (CR 702.140b, 729.2): the spell left the stack and its card is
    /// part of the permanent, which keeps its id, counters, attachments and status.
    Mutated {
        /// The merged permanent.
        object: ObjectId,
        /// The spell that merged with it; its card is an object again when the permanent leaves the battlefield.
        spell: ObjectId,
        /// Whether the spell went on top of the permanent (the permanent has its characteristics) or under it.
        over: bool,
        /// The controller of the spell.
        player: PlayerId,
    },
    /// Two cards of a meld pair in exile became one melded permanent (CR 701.42a, 712.4): the card `object` entered the
    /// battlefield earlier in the batch, and the other card joins it.
    Melded(Box<crate::merged::Melded>),
    /// A card of the merged permanent `permanent` goes to `to` in place of the zone the permanent goes to when it leaves
    /// the battlefield in this batch (CR 729.3): a commander card its owner puts into the command zone (CR 903.9b), or
    /// the other cards when the card the permanent was goes there.
    CardParted {
        /// The merged permanent.
        permanent: ObjectId,
        /// The card, an object of its own again once the permanent has left.
        card: ObjectId,
        /// Where it goes.
        to: Zone,
    },
    /// A creature adapted (CR 701.46): the counters are the event before it.
    Adapted {
        /// The creature that adapted.
        object: ObjectId,
        /// Its controller.
        player: PlayerId,
    },
    /// A player bolstered (CR 701.39): the counters are the event before it.
    Bolstered {
        /// The creature that got the counters.
        object: ObjectId,
        /// Its controller, who bolstered.
        player: PlayerId,
    },
    /// A creature connived (CR 701.50): the draw, the discard and the counter are the events that follow it.
    Connived {
        /// The creature that connived.
        object: ObjectId,
        /// Its controller, who drew and discarded.
        player: PlayerId,
    },
    /// A player clashed with an opponent (CR 701.23) after both revealed and placed their top cards; `won` when the
    /// clashing player won.
    Clashed {
        /// Who clashed.
        player: PlayerId,
        /// The opponent clashed with.
        opponent: PlayerId,
        /// The clashing player won.
        won: bool,
    },
    /// The players finished a vote (CR 701.38): each ballot is a voter and the choice they voted for, by its place among
    /// the options, in the order the votes were cast.
    VotesFinished {
        /// The voter and the place of their choice, for each vote cast.
        ballots: Vec<(PlayerId, u32)>,
    },
    /// A creature was exerted as it was declared an attacker (CR 701.43): it won't untap during the next untap step of
    /// `player`, who exerted it.
    Exerted {
        /// The creature exerted.
        object: ObjectId,
        /// The player who exerted it.
        player: PlayerId,
    },
    /// The untap step of `player` used up their exert of the permanent (CR 701.43c), whether it untapped or not.
    ExertEnded {
        /// The permanent exerted.
        object: ObjectId,
        /// The player whose untap step it was.
        player: PlayerId,
    },
    /// The game became day or night (CR 726).
    DayNightChanged {
        /// What it became.
        to: DayNight,
    },
    /// A player took a manual action (`manual.rs`); the events of the change follow it in the same batch.
    ManualAction {
        /// Who took it.
        player: PlayerId,
        /// What they did.
        action: Box<ManualAction>,
    },
    /// A coin was flipped or a die rolled by hand (`ManualAction::Roll`): the result came from the game's random stream,
    /// which the roll moved on by one. Every seat sees it, and no trigger watches it.
    ManualRolled {
        /// Who rolled.
        player: PlayerId,
        /// A coin or a die.
        kind: ManualRoll,
        /// The result: 1 for heads and 2 for tails, or the face of the die from 1 to its sides.
        result: u32,
    },
    /// The owner of a commander in a graveyard or exile chose to leave it there (CR 903.9a); they are asked again if
    /// it moves and lies in a graveyard or exile once more.
    CommanderStayed {
        /// The commander.
        object: ObjectId,
    },
    /// A player revealed the companion they named (CR 702.139): the card, public from now on, lies outside the game.
    CompanionRevealed {
        /// Whose companion.
        player: PlayerId,
    },
    /// A player paid {3} and put their companion into their hand (CR 702.139, a special action): the card is the object
    /// `object` from now on, and the payment is the events around this one.
    CompanionBrought {
        /// Whose companion.
        player: PlayerId,
        /// The object the card became, in the hand.
        object: ObjectId,
    },
    /// The value of X of a spell on the stack changed ("double the value of X", CR 107.3): its choices hold the new X.
    R112XChanged {
        /// The spell.
        object: ObjectRef,
        /// The new value of X.
        x: u32,
    },
}

/// How the resolution of a spell or ability ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ResolutionOutcome {
    /// It did what it says.
    Resolved,
    /// Every target had become illegal, so it did nothing (CR 608.2b).
    Fizzled,
    /// It was countered, or its controller left the game, and it did nothing (CR 701.6).
    Countered,
    /// Its intervening "if" was no longer true as it resolved, so it did nothing (CR 603.4).
    Lapsed,
}

/// Why an object moved from one zone to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ZoneCause {
    /// An effect moved it.
    Moved,
    /// It was destroyed (CR 701.8).
    Destroyed,
    /// It was destroyed and can't be regenerated.
    Buried,
    /// Its controller sacrificed it (CR 701.21).
    Sacrificed,
    /// It was exiled (CR 406).
    Exiled,
    /// It was exiled face down by foretelling (CR 702.143a): it lies face down, and only its owner may look at it.
    Foretold,
    /// It was exiled face up by plotting (CR 702.170a): it lies in exile and its owner may cast it on a later turn.
    Plotted,
    /// It was manifested by `by` (CR 701.40a): it entered the battlefield face down under the control of `by`, who alone may look at it.
    Manifested {
        /// The player who manifested it and controls it.
        by: PlayerId,
    },
    /// It was cloaked by `by` (CR 701.58a): as a manifested card, with ward {2}.
    Cloaked {
        /// The player who cloaked it and controls it.
        by: PlayerId,
    },
    /// An effect put it onto the battlefield face down (CR 708.3) under the control of `by`, who alone may look at it: a
    /// 2/2 creature turned face up only for its morph, megamorph or disguise cost.
    PutFaceDown {
        /// The player who controls it.
        by: PlayerId,
    },
    /// It was exiled face down by the permanent `by` with hideaway (CR 702.75a): it lies face down, and its owner and the
    /// controller of the permanent may look at it.
    Hideaway {
        /// The permanent that hides it.
        by: ObjectRef,
    },
    /// It was exiled face down by an effect of the source `by`, and `looker` alone may look at it for as long as it lies
    /// exiled, whatever becomes of the source (not its owner, unless that is `looker`).
    Held {
        /// The source of the effect.
        by: ObjectRef,
        /// The player who exiled it and may look at it.
        looker: PlayerId,
    },
    /// It was exiled face down by an effect of the source `by`, and no player may look at it (CR 406.3).
    Sealed {
        /// The source of the effect.
        by: ObjectRef,
    },
    /// It was exiled until the permanent `by` leaves the battlefield (CR 610.3): it returns once that permanent is gone.
    ExiledUntil {
        /// The permanent whose leaving returns it.
        by: ObjectRef,
    },
    /// It was exiled until an opponent of the player `by` becomes the monarch: it returns once one has.
    ExiledUntilMonarch {
        /// The player whose opponents' becoming the monarch returns it.
        by: PlayerId,
    },
    /// It was exiled encoded on the creature `by` (cipher, CR 702.99a): it lies face up in exile linked to the creature,
    /// until it leaves exile or the creature leaves the battlefield.
    Encoded {
        /// The creature it is encoded on.
        by: ObjectRef,
    },
    /// It was exiled haunting the creature `of` (haunt, CR 702.55a): it lies face up in exile linked to the creature.
    Haunting {
        /// The creature it haunts.
        of: ObjectRef,
    },
    /// It resolved as an Adventure (CR 715.4) and was exiled instead of going to its owner's graveyard: its controller may cast the card from exile.
    Adventure,
    /// It was countered (CR 701.6).
    Countered,
    /// It was milled (CR 701.17).
    Milled,
    /// A state-based action put it into its owner's graveyard (CR 704.5), and which one.
    StateBased(StateRule),
    /// It was discarded and a replacement effect sent it somewhere other than the graveyard (CR 701.9).
    Discarded,
    /// It was played as its owner's land and a replacement effect put it here instead of onto the battlefield: the land
    /// play is used (CR 305.2).
    LandPlayed,
}

impl ZoneCause {
    /// How the object lies after a move for this cause on turn `turn`, when it lies face down. A move that ends face
    /// down is how the log tells a card is face down, so a new kind of face-down object is a new cause here.
    pub const fn face_down(self, turn: u32) -> Option<FaceDown> {
        match self {
            ZoneCause::Foretold => Some(FaceDown::Foretold { turn }),
            ZoneCause::Hideaway { by } => Some(FaceDown::Hideaway { by }),
            ZoneCause::Held { by, looker } => Some(FaceDown::Held { by, looker }),
            ZoneCause::Sealed { by } => Some(FaceDown::Sealed { by }),
            ZoneCause::Manifested { .. } => Some(FaceDown::Manifest),
            ZoneCause::Cloaked { .. } => Some(FaceDown::Cloak),
            ZoneCause::PutFaceDown { .. } => Some(FaceDown::Morph),
            _ => None,
        }
    }

    /// The player a move for this cause puts the object under the control of, when the cause says so: whoever manifested,
    /// cloaked or put it face down, who may differ from its owner.
    pub const fn enters_under(self) -> Option<PlayerId> {
        match self {
            ZoneCause::Manifested { by }
            | ZoneCause::Cloaked { by }
            | ZoneCause::PutFaceDown { by } => Some(by),
            _ => None,
        }
    }
}

impl Event {
    /// Objects this event moves or changes, for incremental consistency checks.
    #[allow(clippy::too_many_lines)]
    pub fn touched_objects(&self) -> Vec<ObjectId> {
        match self {
            Event::R111GraveyardOrdered { order, .. } => order.clone(),
            Event::LibraryShuffled { .. }
            | Event::AttackersChosen { .. }
            | Event::AttackChoicesRefused { .. }
            | Event::R5081AttackersChosen { .. }
            | Event::FocusShifted { .. }
            | Event::SpiritRevived { .. }
            | Event::AttractionDeckShuffled { .. }
            | Event::RolledToVisit { .. }
            | Event::PrizeClaimed { .. }
            | Event::StickerSheetsChosen { .. }
            | Event::MulliganTaken { .. }
            | Event::DamageCleared
            | Event::EffectCreated { .. }
            | Event::EffectExpired { .. }
            | Event::RuleCreated { .. }
            | Event::RuleExpired { .. }
            | Event::ShieldUsed { .. }
            | Event::DamagePrevented { .. }
            | Event::FirstPlayerChosen { .. }
            | Event::CommanderColorChosen { .. }
            | Event::ColorsCircled { .. }
            | Event::GameRestarted { .. }
            | Event::MulliganPhaseBegan
            | Event::MulliganDeclared { .. }
            | Event::MulliganKept { .. }
            | Event::MulliganPhaseEnded
            | Event::TurnBegan { .. }
            | Event::ExtraTurnAdded { .. }
            | Event::ExtraTurnSkipped { .. }
            | Event::PlayerControlGained { .. }
            | Event::PhasesAdded { .. }
            | Event::StepAdded { .. }
            | Event::StepBegan { .. }
            | Event::NothingToDo { .. }
            | Event::AnnouncementRewound { .. }
            | Event::TookBack { .. }
            | Event::ManaUndone { .. }
            | Event::DayNightChanged { .. }
            | Event::MonarchChanged { .. }
            | Event::CitysBlessingGained { .. }
            | Event::EnduringStoryGained { .. }
            | Event::SpeedChanged { .. }
            | Event::RingTempted { .. }
            | Event::RingBearerLost { .. }
            | Event::DungeonCompleted { .. }
            | Event::Learned { .. }
            | Event::EvidenceCollected { .. }
            | Event::Investigated { .. }
            | Event::Bent { .. }
            | Event::IgnoredEffect { .. }
            | Event::R112UntapTypeChosen { .. }
            | Event::Discovered { .. }
            | Event::DreadManifested { .. }
            | Event::GiftGiven { .. }
            | Event::R6401Mentored { .. }
            | Event::UpkeepSettled { .. }
            | Event::Foraged { .. }
            | Event::R115CountersPutBy { .. }
            | Event::DieRolled { .. }
            | Event::R6402DieChanged { .. }
            | Event::R706DiceRerolled { .. }
            | Event::R706DieIgnored { .. }
            | Event::R706VisitAsks { .. }
            | Event::R509PilesAssigned { .. }
            | Event::P170aSectorsAssigned { .. }
            | Event::P170aSectorsLost
            | Event::CoinFlipped { .. }
            | Event::Clashed { .. }
            | Event::VotesFinished { .. }
            | Event::Proliferated { .. }
            | Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::ManaAdded { .. }
            | Event::ManaSpent { .. }
            | Event::ManaPoolEmptied { .. }
            | Event::ManaBurned { .. }
            | Event::LifePaid { .. }
            | Event::DrewFromEmptyLibrary { .. }
            | Event::R109DrawReplaced { .. }
            | Event::BlockersDeclared { .. }
            | Event::DamageAssigned { .. }
            | Event::CombatEnded
            | Event::PlayerLost { .. }
            | Event::GameEnded { .. }
            | Event::LoopDrawn { .. }
            | Event::LoopRepeated(_)
            | Event::LoopOffered(_)
            | Event::LoopStopped { .. }
            | Event::AbilityActivated { .. }
            | Event::ManaAbilityActivated { .. }
            | Event::AbilityTriggered(_)
            | Event::TriggersOrdered { .. }
            | Event::TriggerRemoved { .. }
            | Event::DelayedTriggerCreated(_)
            | Event::DelayedTriggerExpired { .. }
            | Event::ResolutionBegan { .. }
            | Event::ChoiceMade { .. }
            | Event::StepDone { .. }
            | Event::R115Continued { .. }
            | Event::ResolutionEnded { .. }
            | Event::ConditionsFailed { .. }
            | Event::AbilityLeftToTable(_)
            | Event::ByHandDue(_)
            | Event::LifeGained { .. }
            | Event::LifeLost { .. }
            | Event::ManualAction { .. }
            | Event::ManualRolled { .. }
            | Event::CommanderStayed { .. }
            | Event::CompanionRevealed { .. }
            | Event::LibraryLooked(_)
            | Event::HandLooked(_)
            | Event::CardsFound(_)
            | Event::ChosenAtRandom { .. }
            | Event::PlayersChosenAtRandom { .. }
            | Event::KindChosenAtRandom { .. }
            | Event::Sought { .. }
            | Event::Revealed(_)
            | Event::FirstDrawRevealed { .. } => Vec::new(),
            Event::CountersChanged(changed) => changed
                .on
                .object()
                .map(|named| named.id)
                .into_iter()
                .collect(),
            Event::LibraryRefilled { order: cards, .. }
            | Event::HandDealt { cards, .. }
            | Event::R115HandExiled { drawn: cards, .. }
            | Event::CardsBottomed { cards, .. }
            | Event::OpeningCardsPlayed { cards, .. }
            | Event::LibraryInserted { cards, .. }
            | Event::LibraryBottomed { cards, .. } => cards.clone(),
            Event::R6400Blocks { blocker, attacker } => vec![*blocker, *attacker],
            Event::R115BlockMoved { blocker, from, to } => {
                [*blocker, *to].into_iter().chain(*from).collect()
            }
            Event::AttackersBlocked { attackers } | Event::BandFormed { attackers, .. } => {
                attackers.clone()
            }
            Event::Scried(scried) => scried.top.iter().chain(&scried.bottom).copied().collect(),
            Event::Surveilled(surveilled) => surveilled
                .top
                .iter()
                .chain(&surveilled.graveyard)
                .copied()
                .collect(),
            Event::TokenCreated(token) => vec![token.object],
            Event::CardConjured(card) => vec![card.object],
            Event::EmblemCreated(emblem) => vec![emblem.object],
            Event::BecameSpirit { emblem, .. }
            | Event::PhaseBegan { emblem, .. }
            | Event::SchemeSetInMotion { emblem, .. } => vec![*emblem],
            Event::Ventured(ventured) => vec![ventured.object],
            Event::BecameCopy(copy) => vec![copy.object],
            // The partner is part of the permanent and no object of its own after the event.
            Event::Melded(melded) => vec![melded.object],
            Event::SpellCopied(copy) => vec![copy.object],
            Event::CardCopied(copy) => vec![copy.object],
            Event::R6008Listed(listed) => vec![listed.object],
            Event::R707Kept(kept) => vec![kept.object],
            Event::AbilityCopied(copy) => vec![copy.object],
            Event::TargetsChanged(change) => vec![change.object.id],
            Event::R112XChanged { object, .. } => vec![object.id],
            Event::Remembered(change) => vec![change.source.id],
            Event::AbilityPutOnStack(put) => vec![put.object],
            Event::LandPlayed { object, .. }
            | Event::R406TurnedUp { object }
            | Event::R707PutBack { object, .. }
            | Event::SpellCast { object, .. }
            | Event::CompanionBrought { object, .. }
            | Event::ZoneChanged { object, .. }
            | Event::SpellResolved { object }
            | Event::Entered { object, .. }
            | Event::Unattached { object }
            | Event::JoinedAttack { object, .. }
            | Event::AttackRedirected { object, .. }
            | Event::AttachedToPlayer { object, .. }
            | Event::ChosenAsAttached { object, .. }
            | Event::R110ChoseSecretly { object, .. }
            | Event::R111ChoiceKept { object, .. }
            | Event::R112Grouped { object }
            | Event::R110ChoiceRevealed { object, .. }
            | Event::Transformed { object, .. }
            | Event::Flipped { object }
            | Event::Prepared { object }
            | Event::Unprepared { object }
            | Event::Suspected { object }
            | Event::Unsuspected { object }
            | Event::Solved { object }
            | Event::Harnessed { object }
            | Event::PermanentTapped { object }
            | Event::SpellControlGained { object, .. }
            | Event::PermanentUntapped { object }
            | Event::PermanentPhasedOut { object, .. }
            | Event::PermanentPhasedIn { object }
            | Event::RemovedFromCombat { object }
            | Event::R6400Unblocked { attacker: object }
            | Event::Regenerated { object }
            | Event::DamageRemoved { object }
            | Event::CardDrawn { object, .. }
            | Event::CardDiscarded { object, .. }
            | Event::Explored { object, .. }
            | Event::Connived { object, .. }
            | Event::Exerted { object, .. }
            | Event::ExertEnded { object, .. }
            | Event::Amassed { object, .. }
            | Event::StickerPut { object, .. }
            | Event::Monstrous { object, .. }
            | Event::Devoured { object, .. }
            | Event::Renowned { object, .. }
            | Event::Exploited { object, .. }
            | Event::Fought { object, .. }
            | Event::Enlisted { object, .. }
            | Event::Crewed { object, .. }
            | Event::ClassLevelGained { object, .. }
            | Event::TurnedFaceUp { object, .. }
            | Event::TurnedFaceDown { object, .. }
            | Event::FaceDownLooked { object, .. }
            | Event::DoorUnlocked { object, .. }
            | Event::DoorLocked { object, .. }
            | Event::Adapted { object, .. }
            | Event::Bolstered { object, .. }
            | Event::Cycled { object, .. }
            | Event::CreatureDied { object, .. }
            | Event::CopyEnded { object, .. }
            | Event::CopyCeased { object }
            | Event::R6051EmblemEnded { object }
            // The spell of a merge is part of the permanent and no object of its own after the event.
            | Event::Mutated { object, .. }
            | Event::CardParted { permanent: object, .. }
            | Event::ObjectLeftGame { object }
            | Event::InitiativeTaken { object, .. } => vec![*object],
            Event::Attached { object, to } => vec![*object, *to],
            Event::Paired { object, with } | Event::Unpaired { object, with } => vec![*object, *with],
            Event::AttackersDeclared { attackers, .. } => {
                attackers.iter().map(|(o, _)| *o).collect()
            }
            Event::DamageDealt { source, target, .. } => match target {
                DamageTarget::Object(o) => vec![*source, *o],
                DamageTarget::Player(_) => vec![*source],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an event stays small, since the log keeps every one of a game, and the events that changed shape keep the JSON they had when they carry nothing new.
    use super::*;

    #[test]
    fn an_event_is_at_most_a_hundred_and_forty_bytes() {
        // The log is a vector of events, each as big as the biggest, so a heavy payload is boxed.
        assert!(
            std::mem::size_of::<Event>() <= 144,
            "{}",
            std::mem::size_of::<Event>()
        );
    }

    #[test]
    fn a_cast_without_announcements_has_the_two_fields_it_always_had() {
        let cast = Event::SpellCast {
            player: PlayerId(1),
            object: ObjectId(7),
            choices: Choices::default(),
        };
        assert_eq!(
            serde_json::to_string(&cast).unwrap(),
            r#"{"SpellCast":{"player":1,"object":7}}"#
        );
        let old: Event = serde_json::from_str(r#"{"SpellCast":{"player":1,"object":7}}"#).unwrap();
        assert_eq!(old, cast);
    }

    #[test]
    fn a_cast_with_targets_says_which() {
        let cast = Event::SpellCast {
            player: PlayerId(0),
            object: ObjectId(3),
            choices: Choices {
                targets: vec![vec![Target::Player(PlayerId(1))]],
                ..Choices::default()
            },
        };
        let json = serde_json::to_string(&cast).unwrap();
        assert_eq!(
            json,
            r#"{"SpellCast":{"player":0,"object":3,"choices":{"targets":[[{"Player":1}]]}}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), cast);
    }

    #[test]
    fn damage_without_deathtouch_has_the_three_fields_it_always_had() {
        let plain = Event::DamageDealt {
            source: ObjectId(4),
            target: DamageTarget::Player(PlayerId(1)),
            amount: 3,
            deathtouch: false,
            form: crate::combat::DamageForm::Normal,
            toxic: 0,
            unpreventable: false,
        };
        let json = r#"{"DamageDealt":{"source":4,"target":{"Player":1},"amount":3}}"#;
        assert_eq!(serde_json::to_string(&plain).unwrap(), json);
        assert_eq!(serde_json::from_str::<Event>(json).unwrap(), plain);
    }

    #[rstest::rstest]
    #[case::crewed(false, r#"{"Crewed":{"object":4,"player":1,"creatures":[7,8]}}"#)]
    #[case::saddled(
        true,
        r#"{"Crewed":{"object":4,"player":1,"creatures":[7,8],"saddle":true}}"#
    )]
    fn a_crew_names_its_creatures_and_says_so_only_for_a_saddle(
        #[case] saddle: bool,
        #[case] json: &str,
    ) {
        let crewed = Event::Crewed {
            object: ObjectId(4),
            player: PlayerId(1),
            creatures: vec![ObjectId(7), ObjectId(8)],
            saddle,
        };
        assert_eq!(serde_json::to_string(&crewed).unwrap(), json);
        assert_eq!(serde_json::from_str::<Event>(json).unwrap(), crewed);
        assert_eq!(crewed.touched_objects(), vec![ObjectId(4)]);
    }

    #[test]
    fn damage_from_a_source_with_deathtouch_says_so() {
        let touched = Event::DamageDealt {
            source: ObjectId(4),
            target: DamageTarget::Object(ObjectId(9)),
            amount: 1,
            deathtouch: true,
            form: crate::combat::DamageForm::Normal,
            toxic: 0,
            unpreventable: false,
        };
        let json = serde_json::to_string(&touched).unwrap();
        assert_eq!(
            json,
            r#"{"DamageDealt":{"source":4,"target":{"Object":9},"amount":1,"deathtouch":true}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), touched);
    }
}
