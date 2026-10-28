//! The event log vocabulary: every change to game state is exactly one of these.

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::abilities::{AbilitySource, Cause, PutOnStack};
use crate::choose::{Choice, Choices, Target};
use crate::combat::{AttackTarget, DamageForm, DamageTarget};
use crate::copies::{AbilityCopy, BecameCopy, CardCopy, SpellCopy};
use crate::counters::CountersChanged;
use crate::day_night::DayNight;
use crate::effects::ContinuousEffect;
use crate::library::{CardsFound, LibraryLooked, Revealed, Scried, Surveilled};
use crate::manual::ManualAction;
use crate::object::{Chosen, FaceDown, ObjectRef};
use crate::player::LossReason;
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
    /// Cards were put on the bottom of the library after keeping.
    CardsBottomed {
        /// Whose cards.
        player: PlayerId,
        /// Cards in the order placed (the last is the bottom).
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
    },
    /// The latest extra turn added is not taken, because its player has left the game or a rule skips it.
    ExtraTurnSkipped {
        /// Whose turn.
        player: PlayerId,
    },
    /// Phases were added after the current one (CR 500.8): each is the step it begins with, in the order they begin,
    /// before any phase added earlier.
    PhasesAdded {
        /// The first step of each added phase.
        steps: Vec<Step>,
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
    },
    /// Attackers were declared (possibly none).
    AttackersDeclared {
        /// The attacking player.
        player: PlayerId,
        /// Each attacker and its target.
        attackers: Vec<(ObjectId, AttackTarget)>,
    },
    /// A defending player declared blockers (possibly none).
    BlockersDeclared {
        /// The defending player.
        player: PlayerId,
        /// Pairs of (blocker, attacker blocked).
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    /// An attacker's damage was divided among its blockers.
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
    },
    /// Combat ended and its state was cleared.
    CombatEnded,
    /// All damage marked on permanents was removed.
    DamageCleared,
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
        effect: ContinuousEffect,
    },
    /// A continuous effect ended and was removed.
    EffectExpired {
        /// The effect.
        id: EffectId,
    },
    /// A change to a rule of the game was registered.
    RuleCreated {
        /// The change.
        effect: RuleEffect,
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
    /// The resolution of a spell or ability is over.
    ResolutionEnded {
        /// The spell or ability.
        object: ObjectId,
        /// How it ended.
        outcome: ResolutionOutcome,
    },
    /// An object moved from one zone to another.
    ZoneChanged {
        /// The object.
        object: ObjectId,
        /// Where it was.
        from: Zone,
        /// Where it went.
        to: Zone,
        /// Why.
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
    /// A permanent stopped being attached to what it was attached to.
    Unattached {
        /// The permanent.
        object: ObjectId,
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
    /// A permanent became a copy of another object (CR 707.2), with the copiable values of the copy effect.
    BecameCopy(BecameCopy),
    /// A spell was copied (CR 707.10): the copy is put on the stack above it, not cast.
    SpellCopied(Box<SpellCopy>),
    /// A card in exile was copied (CR 707.12): the copy is a token card in exile that its owner may cast, and it ceases to
    /// exist once it is anywhere but on the stack.
    CardCopied(Box<CardCopy>),
    /// A copy of a card ceased to exist in a zone other than the stack (CR 704.5e).
    CopyCeased {
        /// The copy.
        object: ObjectId,
    },
    /// An activated or triggered ability was copied (CR 707.10): the copy is put on the stack above it, not activated or triggered.
    AbilityCopied(Box<AbilityCopy>),
    /// A permanent's copy effect that lasted until end of turn, or until a player's next turn, ended.
    CopyEnded {
        /// The permanent.
        object: ObjectId,
        /// The player whose next turn began: the copies until that turn end; none for those until end of turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        next_turn_of: Option<PlayerId>,
    },
    /// A player became the monarch (CR 724).
    MonarchChanged {
        /// The new monarch.
        player: PlayerId,
    },
    /// A player got the city's blessing (CR 702.131): a designation they keep for the rest of the game.
    CitysBlessingGained {
        /// Who got it.
        player: PlayerId,
    },
    /// A player learned (CR 701.48): the discard and the draw are the events that follow it.
    Learned {
        /// Who learned.
        player: PlayerId,
    },
    /// A player rolled a die (CR 706): the number came from the game's random stream, which the roll moved on by one.
    DieRolled {
        /// Who rolled.
        player: PlayerId,
        /// How many faces the die has.
        sides: u32,
        /// The number rolled, from 1 to `sides`.
        result: u32,
    },
    /// A player flipped a coin (CR 705): the flip came from the game's random stream, which it moved on by one.
    CoinFlipped {
        /// Who flipped.
        player: PlayerId,
        /// The flip was won.
        won: bool,
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
    },
    /// A player amassed (CR 701.47): the counters and the Army token are the events around it.
    Amassed {
        /// The Army that got the counters.
        object: ObjectId,
        /// Its controller, who amassed.
        player: PlayerId,
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
    /// It was exiled face down by the permanent `by` with hideaway (CR 702.75a): it lies face down, and its owner and the
    /// controller of the permanent may look at it.
    Hideaway {
        /// The permanent that hides it.
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
    /// It resolved as an Adventure (CR 715.4) and was exiled instead of going to its owner's graveyard: its controller may cast the card from exile.
    Adventure,
    /// It was countered (CR 701.6).
    Countered,
    /// It was milled (CR 701.17).
    Milled,
    /// A state-based action put it into its owner's graveyard (CR 704.5i, 704.5j, 704.5m).
    StateBased,
    /// It was discarded and a replacement effect sent it somewhere other than the graveyard (CR 701.9).
    Discarded,
}

impl ZoneCause {
    /// How the object lies after a move for this cause on turn `turn`, when it lies face down. A move that ends face
    /// down is how the log tells a card is face down, so a new kind of face-down object is a new cause here.
    pub const fn face_down(self, turn: u32) -> Option<FaceDown> {
        match self {
            ZoneCause::Foretold => Some(FaceDown::Foretold { turn }),
            ZoneCause::Hideaway { by } => Some(FaceDown::Hideaway { by }),
            ZoneCause::Manifested { .. } => Some(FaceDown::Manifest),
            ZoneCause::Cloaked { .. } => Some(FaceDown::Cloak),
            _ => None,
        }
    }

    /// The player a move for this cause puts the object under the control of, when the cause says so: whoever manifested
    /// or cloaked it, who may differ from its owner.
    pub const fn enters_under(self) -> Option<PlayerId> {
        match self {
            ZoneCause::Manifested { by } | ZoneCause::Cloaked { by } => Some(by),
            _ => None,
        }
    }
}

impl Event {
    /// Objects this event moves or changes, for incremental consistency checks.
    #[allow(clippy::too_many_lines)]
    pub fn touched_objects(&self) -> Vec<ObjectId> {
        match self {
            Event::LibraryShuffled { .. }
            | Event::MulliganTaken { .. }
            | Event::DamageCleared
            | Event::EffectCreated { .. }
            | Event::EffectExpired { .. }
            | Event::RuleCreated { .. }
            | Event::RuleExpired { .. }
            | Event::ShieldUsed { .. }
            | Event::FirstPlayerChosen { .. }
            | Event::MulliganPhaseBegan
            | Event::MulliganDeclared { .. }
            | Event::MulliganKept { .. }
            | Event::MulliganPhaseEnded
            | Event::TurnBegan { .. }
            | Event::ExtraTurnAdded { .. }
            | Event::ExtraTurnSkipped { .. }
            | Event::PhasesAdded { .. }
            | Event::StepBegan { .. }
            | Event::NothingToDo { .. }
            | Event::DayNightChanged { .. }
            | Event::MonarchChanged { .. }
            | Event::CitysBlessingGained { .. }
            | Event::Learned { .. }
            | Event::DieRolled { .. }
            | Event::CoinFlipped { .. }
            | Event::Proliferated { .. }
            | Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::ManaAdded { .. }
            | Event::ManaSpent { .. }
            | Event::ManaPoolEmptied { .. }
            | Event::ManaBurned { .. }
            | Event::LifePaid { .. }
            | Event::DrewFromEmptyLibrary { .. }
            | Event::BlockersDeclared { .. }
            | Event::DamageAssigned { .. }
            | Event::CombatEnded
            | Event::PlayerLost { .. }
            | Event::GameEnded { .. }
            | Event::AbilityActivated { .. }
            | Event::AbilityTriggered(_)
            | Event::TriggersOrdered { .. }
            | Event::TriggerRemoved { .. }
            | Event::DelayedTriggerCreated(_)
            | Event::DelayedTriggerExpired { .. }
            | Event::ResolutionBegan { .. }
            | Event::ChoiceMade { .. }
            | Event::StepDone { .. }
            | Event::ResolutionEnded { .. }
            | Event::LifeGained { .. }
            | Event::LifeLost { .. }
            | Event::ManualAction { .. }
            | Event::CommanderStayed { .. }
            | Event::CompanionRevealed { .. }
            | Event::LibraryLooked(_)
            | Event::CardsFound(_)
            | Event::ChosenAtRandom { .. }
            | Event::Revealed(_) => Vec::new(),
            Event::CountersChanged(changed) => changed
                .on
                .object()
                .map(|named| named.id)
                .into_iter()
                .collect(),
            Event::LibraryRefilled { order: cards, .. }
            | Event::HandDealt { cards, .. }
            | Event::CardsBottomed { cards, .. }
            | Event::LibraryBottomed { cards, .. } => cards.clone(),
            Event::Scried(scried) => scried.top.iter().chain(&scried.bottom).copied().collect(),
            Event::Surveilled(surveilled) => surveilled
                .top
                .iter()
                .chain(&surveilled.graveyard)
                .copied()
                .collect(),
            Event::TokenCreated(token) => vec![token.object],
            Event::EmblemCreated(emblem) => vec![emblem.object],
            Event::BecameCopy(copy) => vec![copy.object],
            Event::SpellCopied(copy) => vec![copy.object],
            Event::CardCopied(copy) => vec![copy.object],
            Event::AbilityCopied(copy) => vec![copy.object],
            Event::AbilityPutOnStack(put) => vec![put.object],
            Event::LandPlayed { object, .. }
            | Event::SpellCast { object, .. }
            | Event::CompanionBrought { object, .. }
            | Event::ZoneChanged { object, .. }
            | Event::SpellResolved { object }
            | Event::Entered { object, .. }
            | Event::Unattached { object }
            | Event::JoinedAttack { object, .. }
            | Event::AttachedToPlayer { object, .. }
            | Event::Transformed { object, .. }
            | Event::Flipped { object }
            | Event::Prepared { object }
            | Event::Unprepared { object }
            | Event::PermanentTapped { object }
            | Event::PermanentUntapped { object }
            | Event::PermanentPhasedOut { object, .. }
            | Event::PermanentPhasedIn { object }
            | Event::Regenerated { object }
            | Event::DamageRemoved { object }
            | Event::CardDrawn { object, .. }
            | Event::CardDiscarded { object, .. }
            | Event::Explored { object, .. }
            | Event::Connived { object, .. }
            | Event::Amassed { object, .. }
            | Event::Monstrous { object, .. }
            | Event::Renowned { object, .. }
            | Event::Exploited { object, .. }
            | Event::Crewed { object, .. }
            | Event::ClassLevelGained { object, .. }
            | Event::TurnedFaceUp { object, .. }
            | Event::Adapted { object, .. }
            | Event::Bolstered { object, .. }
            | Event::Cycled { object, .. }
            | Event::CreatureDied { object, .. }
            | Event::CopyEnded { object, .. }
            | Event::CopyCeased { object }
            // The spell of a merge is part of the permanent and no object of its own after the event.
            | Event::Mutated { object, .. }
            | Event::CardParted { permanent: object, .. }
            | Event::ObjectLeftGame { object } => vec![*object],
            Event::Attached { object, to } => vec![*object, *to],
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
        };
        let json = serde_json::to_string(&touched).unwrap();
        assert_eq!(
            json,
            r#"{"DamageDealt":{"source":4,"target":{"Object":9},"amount":1,"deathtouch":true}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), touched);
    }
}
