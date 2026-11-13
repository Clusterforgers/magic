//! References to players and objects, selectors over the game, and target specifications.

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::keyword::Keyword;
use crate::value::{Amount, CastWay, Cmp, Condition, CounterKind, StatKind};

/// The slot of `Ref::Remembered` that holds what the source exiled (`Pred::ExiledWith` reads it).
pub const EXILED_SLOT: &str = "exiled";

/// A zone a selector or a pattern can name.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum ZoneKind {
    /// The battlefield (the default place to look).
    #[default]
    Battlefield,
    /// A hand.
    Hand,
    /// A library.
    Library,
    /// A graveyard.
    Graveyard,
    /// The stack.
    Stack,
    /// Exile.
    Exile,
    /// The command zone.
    Command,
    /// Outside the game (a sideboard): a Commander game has none, so nothing is ever found there.
    Outside,
}

impl ZoneKind {
    /// Whether this is the default zone, so serialization can leave it out.
    pub const fn is_battlefield(&self) -> bool {
        matches!(self, ZoneKind::Battlefield)
    }

    /// The default zone a cast comes from.
    pub const fn hand() -> ZoneKind {
        ZoneKind::Hand
    }

    /// Whether this is a hand, the default zone of a cast.
    pub const fn is_hand(&self) -> bool {
        matches!(self, ZoneKind::Hand)
    }

    /// The default zone a search looks in.
    pub const fn library() -> ZoneKind {
        ZoneKind::Library
    }

    /// Whether this is a library, the default zone of a search.
    pub const fn is_library(&self) -> bool {
        matches!(self, ZoneKind::Library)
    }
}

/// A player or object an effect, pattern or value talks about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Ref {
    /// The controller of the spell or ability.
    You,
    /// Every opponent of the controller.
    Opponents,
    /// Every player.
    EachPlayer,
    /// The source object of the ability (the spell itself for a spell).
    This,
    /// The targets chosen for the target specification with this index.
    Target(u8),
    /// The player or object the trigger event is about ("that creature", "that player").
    Triggering,
    /// The player the trigger event is about, never the object it also holds ("that player" of an activation: damage to it spares the activated permanent).
    TriggeringPlayer,
    /// The first of the things the trigger event is about: the object that became a target (the spell or ability that targets it
    /// and its controller follow it among the subjects, and `Dealer` names that spell or ability).
    TriggeringFirst,
    /// The object that did what the trigger event is about: the source of the damage ("Whenever a Vampire you control deals
    /// combat damage to a player, put a +1/+1 counter on it"); nobody for an event with no such object.
    Dealer,
    /// What the subject of the trigger event went against: the player, planeswalker or battle an attacker attacks, the
    /// attacker a creature blocks; nobody for an event with no such thing.
    Against,
    /// What the source is attached to ("enchanted creature", "equipped creature").
    Attached,
    /// The permanent whose static ability gave the source the ability being run ("this Equipment", "this Aura" in the
    /// words the Equipment gives the creature); nobody for an ability the source has of its own, or once that permanent has left the battlefield.
    Granter,
    /// The player the source is attached to ("enchanted player").
    AttachedPlayer,
    /// The monarch ("the monarch"); nobody while no player is the monarch.
    Monarch,
    /// The player the source chose as it entered ("the chosen player"); nobody before the choice, for a source that chose no player or once that player has left the game.
    Chosen,
    /// The opponent the gift was promised to as the spell the ability reads was cast (CR 702.174); nobody when none was promised or they have left the game.
    Promised,
    /// The defending player of the attacking creature that triggered the ability, else of the source: whoever it
    /// attacks, or controls the planeswalker it attacks (CR 506.3); still named once the creature has left combat. For
    /// a source that is not attacking, every player but the active one during combat (CR 506.2, 802.2).
    Defending,
    /// The opponents of the controller that their creatures attacked this combat, directly or through a planeswalker
    /// ("each opponent you attacked this combat"); a creature that left combat still counts.
    Attacked,
    /// The player with the most life, or each of the players tied for it.
    MostLife,
    /// The player whose turn it is; nobody once they have left the game.
    ActivePlayer,
    /// The player making the choice of an `Effect::Choose` whose candidates are worked out for each chooser ("each player
    /// chooses a creature they control"); nobody anywhere else.
    Chooser,
    /// Inside the test of a `PlayersWhere`, the player whose ability asks it, while "you" is the player tested ("an
    /// opponent who controls more lands than you"); the controller everywhere else.
    Asker,
    /// The result an earlier step bound with `as`.
    Bound(String),
    /// The objects picked for the cost of the ability ("the creature tapped to pay it"): their characteristics are those they
    /// have as the ability resolves, and none is named when the cost picks nothing.
    Paid,
    /// The creatures tapped to help cast the spell the source is or became, with convoke (CR 702.51): "each creature that
    /// convoked it"; none for a spell cast without convoking and once a creature has changed zones.
    Convoked,
    /// The object a static ability's change applies to, worked out for each such object ("for each other creature that shares a type with it").
    Affected,
    /// The object a `Pred::Test` is judging: "a creature you control with power greater than its base power".
    Candidate,
    /// The objects the source named by `on` remembers in `slot` (`Effect::Remember`; the slot `exiled` holds what the source
    /// exiled), as the objects they are now: one that has changed zones since is another object and drops out.
    Remembered {
        /// The object whose memory is read.
        on: Box<Ref>,
        /// The name of the memory.
        slot: String,
    },
    /// The controller of the referenced object.
    ControllerOf(Box<Ref>),
    /// The owner of the referenced object.
    OwnerOf(Box<Ref>),
    /// The player the reference names, or the controller of the object it names ("target player or planeswalker" read as
    /// that player or that planeswalker's controller).
    OrController(Box<Ref>),
    /// The object an object named one zone change ago has become, when it is now in this zone ("return it to the battlefield"
    /// after it died): the same card one incarnation on (CR 400.7). Nobody when it moved twice, moved elsewhere or is gone.
    Arrived {
        /// The reference to the object as it was before it moved.
        of: Box<Ref>,
        /// The zone it must be in now.
        zone: ZoneKind,
    },
    /// The objects `of` names as they were before they last moved (CR 400.7, 608.2h): "its controller" over permanents
    /// an earlier step destroyed reads the controller they had. Each is named by what it was, as the resolution saw it
    /// leave; one with no such record (it moved twice, or came from a hidden zone) is nobody.
    Departed(Box<Ref>),
    /// The source of each ability on the stack the reference names, as the object it was when the ability was put there
    /// ("that permanent" of a countered ability); a spell or any other object names nobody.
    SourceOf(Box<Ref>),
    /// Every object matching the selector when the effect runs.
    Each(Selector),
    /// Every player and object any of these references names, as one recipient ("you and/or permanents you control"): a
    /// damage shield used up by what it covers shares what it has left among them.
    Union(Vec<Ref>),
    /// The next player still in the game seated on `side` of each player `of` names (CR 101.4: turns pass to the left):
    /// "the player to your right"; with `opponent`, the nearest opponent of that player on that side ("the opponent to
    /// your left"). Nobody when no other player is left.
    Seated {
        /// The players whose neighbour is named.
        of: Box<Ref>,
        /// Which side of them.
        side: Side,
        /// Only an opponent of theirs counts.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        opponent: bool,
    },
    /// The players `who` names for whom the condition holds, each taken as "you" ("each opponent who lost life this turn",
    /// "a player with seven or more cards in their graveyard"); nobody when it holds for none.
    PlayersWhere {
        /// The players tested.
        who: Box<Ref>,
        /// What must hold, written for "you".
        test: Box<Condition>,
    },
}

impl Ref {
    /// The default `who`: the controller.
    pub const fn you() -> Ref {
        Ref::You
    }

    /// Whether this is the controller, so serialization can leave it out.
    pub const fn is_you(&self) -> bool {
        matches!(self, Ref::You)
    }

    /// The default player scope of a pattern: every player.
    pub const fn each_player() -> Ref {
        Ref::EachPlayer
    }

    /// Whether this is every player, so serialization can leave it out.
    pub const fn is_each_player(&self) -> bool {
        matches!(self, Ref::EachPlayer)
    }
}

/// A side of a player at the table (CR 101.4): turns pass from each player to the player on their left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// The side turns pass to: the next seat.
    Left,
    /// The side turns come from: the seat before.
    Right,
}

/// Which kind of ability on the stack a predicate selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AbilityKind {
    /// An activated ability, a loyalty ability included.
    Activated,
    /// A triggered ability.
    Triggered,
}

/// Which abilities an object has, for a predicate on what it can do ("with a mana ability").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeldAbility {
    /// Any ability at all, a keyword or one the engine leaves to the table included ("no abilities" is its negation).
    Any,
    /// A mana ability (CR 605.1a).
    Mana,
    /// An activated ability that isn't a mana ability, a loyalty ability included.
    NonManaActivated,
    /// An activated ability with {T} in its cost, a mana ability included ("that has an activated ability with {T} in its
    /// cost").
    TapCost,
}

/// A condition on one object; selectors hold a list of them that must all be true.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Pred {
    /// Is the source object of the ability.
    This,
    /// Is not the source object.
    Other,
    /// Is one of the objects the reference names (`{"Is": {"Bound": "creature"}}`).
    Is(Ref),
    /// Shares a subtype with one of the objects the reference names (`{"SharesSubtype": {"Bound": "land"}}`).
    SharesSubtype(Ref),
    /// Shares a creature type with each of the objects the reference names ("that shares a creature type with each
    /// creature tapped this way"); true when it names none.
    SharesCreatureTypeWithEach(Ref),
    /// Is a dungeon a player is in (CR 309): the object in a command zone that holds a venture marker.
    Dungeon,
    /// Has the same name as one of the objects the reference names ("with the same name as that creature"); a nameless
    /// object (a face-down one) shares no name.
    SharesName(Ref),
    /// Shares a card type with one of the objects the reference names ("that shares a card type with it").
    SharesType(Ref),
    /// Shares a color with one of the objects the reference names ("that shares a color with it").
    SharesColor(Ref),
    /// Has the mana value of one of the objects the reference names ("protection from each mana value among artifacts you
    /// control").
    SharesManaValue(Ref),
    /// Shares one of these card types with one of the objects the reference names ("that share a permanent type",
    /// "that shares one of those types with it").
    SharesTypeOf {
        /// The objects compared with.
        with: Ref,
        /// The card types that count.
        types: Vec<mtg_core::card::CardType>,
    },
    /// Is an Aura card that could enchant the source ("an Aura card in your hand that could enchant this creature", CR 303.4).
    CanEnchantSource,
    /// Has {X} in its mana cost ("a spell with {X} in its mana cost", CR 107.3).
    XInCost,
    /// Has a Phyrexian symbol in its mana cost ("a spell with {H} in its mana cost", CR 107.4f).
    PhyrexianInCost,
    /// Has a mana cost of exactly that many generic mana and nothing else ("with mana cost {1}"): `{0}` is a cost, no cost
    /// is not, and a coloured, hybrid, Phyrexian or {X} symbol is another cost with the same mana value.
    GenericCost(u32),
    /// Is a creature on the battlefield that some player goads (CR 701.15).
    Goaded,
    /// Has a permanent attached to it that matches the selector ("enchanted": `[{"Subtype":"Aura"}]`, "enchanted by an
    /// Aura you control", "equipped").
    HasAttached(Selector),
    /// Is a permanent attached to one of the objects the reference names ("Aura attached to it", "Equipment attached to
    /// equipped creature").
    AttachedTo(Ref),
    /// Is a card exiled by one of the permanents the reference names, which has not left since: face down by hideaway
    /// (CR 702.75a), or by any effect of that permanent (CR 607.2a), read from its `exiled` memory.
    ExiledWith(Ref),
    /// Is a creature on the battlefield that one of the cards the reference names is encoded on, which has not left since (cipher, CR 702.99).
    EncodedWith(Ref),
    /// Is the creature one of the cards the reference names haunts (CR 702.55b): the card lies in exile, exiled haunting
    /// this creature as it then was; a creature that has died is judged as it last was on the battlefield.
    HauntedBy(Ref),
    /// Is a creature tapped this turn to crew one of the Vehicles the reference names ("creature that crewed it this turn").
    Crewed(Ref),
    /// Is a creature tapped this turn to saddle one of the Mounts the reference names ("creature that saddled it this turn").
    Saddled(Ref),
    /// Entered the battlefield this turn, from anywhere and under any control (a token made this turn included).
    EnteredThisTurn,
    /// Is a permanent that summoning sickness holds back from attacking and from {T} abilities (CR 302.6).
    SummoningSick,
    /// Is a card in a graveyard that a mill put there this turn ("a land card in a graveyard that was milled this turn").
    MilledThisTurn,
    /// Is a card its owner drew this turn, still in the hand ("cards in your hand drawn this turn").
    DrawnThisTurn,
    /// Is a card with an Adventure (CR 715): a card of the Adventure layout, in any zone and on the stack as either half.
    HasAdventure,
    /// Has the card type.
    Type(CardType),
    /// Has the supertype.
    Supertype(Supertype),
    /// Has the subtype.
    Subtype(Subtype),
    /// Is the color.
    Color(Color),
    /// Is colorless.
    Colorless,
    /// Has a color outside the color identity of the commanders the player reading it (`You`) owns, every color when
    /// they own none ("protection from each color that's not in your commander's color identity").
    OutsideCommanderIdentity,
    /// Is a token.
    Token,
    /// Is a player's commander (CR 903.3): a card designated as one when the game began, whoever controls it now.
    Commander,
    /// Is a spell on the stack.
    Spell,
    /// Is an activated or a triggered ability on the stack (a mana ability never is one).
    Ability(AbilityKind),
    /// Is an ability on the stack whose source matches the selector ("activated ability from an artifact source"): the
    /// source as it is now, or as it last existed once it has left; written beside `Ability`, which keeps it to the stack.
    AbilityFrom(Selector),
    /// Is the source of one of the abilities on the stack the reference names, as the same object ("that permanent": the
    /// source of the ability a spell counters).
    SourceOf(Ref),
    /// Has an ability of the kind ("creatures with no abilities", "an artifact with a mana ability"): the abilities it
    /// has now on the battlefield, its printed ones elsewhere.
    HasAbility(HeldAbility),
    /// Is tapped.
    Tapped,
    /// Is untapped.
    Untapped,
    /// Is monstrous (CR 701.37): a permanent on the battlefield that has become monstrous since it entered.
    Monstrous,
    /// Is renowned (CR 702.112): a permanent on the battlefield that has become renowned since it entered.
    Renowned,
    /// Is prepared: a permanent on the battlefield whose controller may cast a copy of the spell face of its card.
    Prepared,
    /// Is suspected (CR 701.60): a permanent on the battlefield that has been suspected and is not yet no longer suspected.
    Suspected,
    /// Is solved (CR 719.3): a Case on the battlefield that has been solved.
    Solved,
    /// Is a permanent one of whose mana abilities could produce mana of the type, costs and restrictions ignored ("could
    /// produce {C}", CR 106.7).
    CouldProduce(mtg_core::mana::ManaType),
    /// Is stickered (CR 123): has one or more stickers on it, a permanent or a card elsewhere.
    Stickered,
    /// Has a sticker of the kind on it (CR 123): "with an art sticker on it", on the battlefield or a card elsewhere
    /// (stickers stay on a card as it changes zones).
    WithSticker(crate::ability::StickerKind),
    /// Is a harnessed permanent: its ∞ ability works.
    Harnessed,
    /// Attacked, blocked or became blocked (as marked) since your last upkeep.
    FoughtSinceYourLastUpkeep {
        /// It attacked.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        attacked: bool,
        /// It blocked.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        blocked: bool,
        /// It became blocked.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        was_blocked: bool,
    },
    /// Is a double-faced card (CR 712.1): a transforming or a modal double-faced card in any zone; a permanent that is a copy of one is not.
    DoubleFaced,
    /// Is transformed (CR 701.28): a transforming double-faced permanent with its back face up; a copy of one is not.
    Transformed,
    /// Is paired with another creature (CR 702.95): a creature on the battlefield that a soulbond pairing holds.
    Paired,
    /// Is the creature paired with one of the objects the reference names (the partner: "each of those creatures" is `AnyOf` of `This` and `PairedWith(This)`).
    PairedWith(Ref),
    /// Is attacking.
    Attacking,
    /// Is attacking the player, not a planeswalker of theirs ("attacking that player").
    AttackingPlayer(Ref),
    /// Is attacking a planeswalker one of the players controls ("attacking you or a planeswalker you control" is `AnyOf`
    /// of this and `AttackingPlayer`).
    AttackingPlaneswalkerOf(Ref),
    /// Is the only creature attacking ("attacking alone", CR 506.5).
    AttackingAlone,
    /// Is attacking a battle.
    AttackingBattle,
    /// Is attacking the player or a planeswalker they control ("whenever an opponent attacks you and/or a planeswalker you
    /// control").
    AttackingPlayerOrPlaneswalker(Ref),
    /// Is blocking.
    Blocking,
    /// Is an attacking creature that no creature was declared to block, once its defending player has declared blockers (CR 509.1h).
    Unblocked,
    /// Is controlled by the player.
    ControlledBy(Ref),
    /// Is owned by the player.
    OwnedBy(Ref),
    /// Is a battle one of the players protects (CR 310.5): "if an opponent protects it".
    ProtectedBy(Ref),
    /// Is in the zone (without it, the battlefield).
    In(ZoneKind),
    /// Is a spell cast from the zone, a hand, a graveyard or exile (the spell being cast included).
    CastFrom(ZoneKind),
    /// Has the keyword.
    HasKeyword(Keyword),
    /// Has the keyword at any value of its parameter ("with toxic", "with cycling"); the name is Scryfall's spelling
    /// (`Keyword::NAMES`).
    HasKeywordNamed(String),
    /// Has at least one counter of the kind on it.
    HasCounter(CounterKind),
    /// Has at least one counter of any kind on it (CR 122.1).
    HasAnyCounter,
    /// The number of counters of the kind on it compares to `to` ("three or more +1/+1 counters"); none at all counts as zero.
    CounterCount {
        /// The kind of counter counted.
        kind: CounterKind,
        /// The comparison of the number held with `to`.
        op: Cmp,
        /// The number compared with.
        to: u32,
    },
    /// Has exactly this card name.
    Named(String),
    /// Is the colour the controller of the source chose as the source entered the battlefield.
    ChosenColor,
    /// Has the creature type the controller of the source chose as the source entered the battlefield; in a landwalk the land type.
    ChosenSubtype,
    /// Has the card name the controller of the source chose as the source entered the battlefield.
    ChosenName,
    /// Matches the value the resolution remembered under the name (`Effect::ChooseValue`): has the colour, the creature,
    /// land or card type or the card name; a number or a word matches nothing.
    ChosenAs(String),
    /// Its power, toughness, mana value or loyalty compares to the amount.
    Compare {
        /// The characteristic.
        stat: StatKind,
        /// The comparison.
        op: Cmp,
        /// The amount compared with.
        to: Amount,
    },
    /// Two amounts of the object compare: `Ref::Candidate` in them is the object itself ("power less than toughness").
    Test {
        /// The amount on the left.
        left: Amount,
        /// The comparison.
        op: Cmp,
        /// The amount on the right.
        right: Amount,
    },
    /// A spell or ability on the stack with a target that matches the selector. A spell being cast reads the targets
    /// announced so far; the cost judged before any target holds it where some object in play matches.
    Targeting(Selector),
    /// Was dealt damage this turn.
    DamagedThisTurn,
    /// Was dealt damage this turn by a source one of the players the reference names controlled as it dealt it ("a
    /// creature dealt damage this turn by a source you controlled").
    DamagedThisTurnBy(Ref),
    /// Was declared as an attacker this turn ("creatures that attacked this turn").
    AttackedThisTurn,
    /// Attacked during its controller's last turn, as the same object ("creatures that attacked during their controller's
    /// last turn").
    AttackedLastTurn,
    /// Has dealt damage since it entered the battlefield, as this object ("as long as it hasn't dealt damage yet").
    DealtDamageYet,
    /// Has dealt combat damage since it entered the battlefield, as this object.
    DealtCombatDamageYet,
    /// Is the spell that was the nth spell cast this turn, by any player ("spell that's the second spell cast this
    /// turn"); a copy is not cast.
    NthCastThisTurn(u32),
    /// Was dealt damage this turn by one of the objects the reference names (the object as it is now: one that left and came back is another).
    DamagedBy(Ref),
    /// Was the object that did the event this turn ("creature that blocked this turn", "creature that dealt damage to you
    /// this turn", "creature that attacked this combat"): the turn's records of the kind, with the object as the one
    /// that did it (the dealer of damage, the attacker, the blocker, the source of an activated ability, the permanent
    /// that got counters). A record of a hidden object names no one.
    DidThisTurn {
        /// The kind of event.
        event: crate::value::TallyEvent,
        /// The players it counts for (the controller of the dealer, the attacker, the blocker or the source; the player
        /// who put the counters); anyone when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Ref>,
        /// What it was done to (the player or object damaged, attacked or blocked); anything when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Ref>,
        /// Only in the combat phase now going on or the latest one.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        combat: bool,
    },
    /// Entered the battlefield since the latest turn of a player the reference names ended ("that entered since your last
    /// turn ended"); before any turn of theirs, whenever it entered.
    EnteredSinceLastTurn(Ref),
    /// Blocked or was blocked this turn by a creature the selector matches as it was when the block was declared ("that
    /// blocked or was blocked by a Zombie this turn"); by any creature when the selector is empty.
    BlockedThisTurn(Selector),
    /// Is the card one of the objects the reference names is, in whatever zone it is now: a card that went to a graveyard is a
    /// new object there (CR 400.7), and this finds it again by its card.
    SameCard(Ref),
    /// Is the Ring-bearer of its controller (CR 701.54).
    RingBearer,
    /// Is modified (CR 700.9): a permanent with a counter on it, an Equipment attached, or an Aura its controller controls attached.
    Modified,
    /// Lies face down: a face-down permanent, or a card exiled face down.
    FaceDown,
    /// A spell or ability on the stack with a target that is one of the players the reference names ("spell that targets you").
    TargetsPlayer(Ref),
    /// A spell on the stack with at least `least` mana from sources of the kind spent to cast it ("if mana from a Treasure
    /// was spent to cast it", "if three or more mana from creatures was spent to cast it").
    SpentFrom {
        /// The kind of source.
        from: crate::spend::SpendFilter,
        /// The fewest mana from such sources.
        least: u32,
    },
    /// Is a card in a graveyard that was put there from the battlefield this turn ("creature cards in your graveyard that were
    /// put there from the battlefield this turn").
    GravedThisTurn,
    /// Went into the zone it is in now this turn from a zone of the kind ("creature cards put into your graveyard from your
    /// library this turn"); an object that moved again counts only for its latest move, and a token made there came from nowhere.
    ArrivedFrom(ZoneKind),
    /// Is a spell cast, or a permanent that entered as a spell cast, by one of the players `by` names (anyone when absent)
    /// from the kind of zone `from` (anywhere when absent); a copy or a token was never cast.
    WasCast {
        /// The players who cast it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Ref>,
        /// The zone it was cast from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ZoneKind>,
    },
    /// Is a spell, or a permanent that entered as a spell, cast for the alternative cost (kicked, flashed back, evoked).
    CastWith(CastWay),
    /// Is the top card of its owner's library.
    TopOfLibrary,
    /// Is the bottom card of its owner's library.
    BottomOfLibrary,
    /// Is the top card of its owner's graveyard.
    TopOfGraveyard,
    /// Is the topmost creature card of its owner's graveyard.
    TopCreatureOfGraveyard,
    /// Was blocked this turn by one of the creatures the reference names (read from the turn's log of blocks), whether or
    /// not it is still attacking ("all creatures that were blocked by that creature this turn").
    BlockedThisTurnBy(Ref),
    /// Is an attacking creature blocked by one of the creatures the reference names.
    BlockedBy(Ref),
    /// Is an attacking creature in the band of one of the creatures the reference names, other than that creature
    /// ("all creatures banded with it", CR 702.22c).
    BandedWith(Ref),
    /// Is a creature blocking one of the creatures the reference names.
    Blocks(Ref),
    /// Is a target of one of the spells or abilities on the stack the reference names.
    TargetedBy(Ref),
    /// Is an activated or triggered ability on the stack whose source matches the selector, as the source is now or, once
    /// it has left its zone, as it last existed ("activated ability from an artifact source", CR 113.7a).
    FromSource(Selector),
    /// Is a spell or ability on the stack with exactly one target (CR 115.7: "a spell with a single target").
    SingleTarget,
    /// Has the greatest value of the characteristic among the objects the group selector matches (a tie counts for each).
    GreatestAmong {
        /// The characteristic compared.
        stat: StatKind,
        /// The objects compared; the object itself counts when it matches.
        group: Selector,
    },
    /// Has the least value of the characteristic among the objects the group selector matches (a tie counts for each).
    LeastAmong {
        /// The characteristic compared.
        stat: StatKind,
        /// The objects compared; the object itself counts when it matches.
        group: Selector,
    },
    /// Is a card in exile with a time counter on it and suspend (CR 702.62).
    Suspended,
    /// Is a spell, or a permanent that entered as a spell, cast with at least one kicker cost paid (CR 702.33).
    Kicked,
    /// Is a spell, or a permanent that entered as a spell, cast with an additional cost paid (bargain, casualty, gift; CR 601.2b).
    AdditionalCostPaid,
    /// The predicate is false.
    Not(Box<Pred>),
    /// At least one of the predicates is true.
    AnyOf(Vec<Pred>),
}

/// A description of a set of objects: all predicates must hold. The empty list matches everything.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Selector(pub Vec<Pred>);

impl Selector {
    /// The selector that matches only the source object.
    pub fn this() -> Selector {
        Selector(vec![Pred::This])
    }

    /// Whether the selector has no predicates.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What the targets chosen for one specification must have in common or differ in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetSet {
    /// One player controls them all (owns them all, outside the battlefield).
    SameController,
    /// No two have one controller (owner, outside the battlefield).
    DifferentControllers,
    /// No two have one name.
    DifferentNames,
    /// No two have one mana value.
    DifferentManaValues,
    /// They all have one toughness ("with equal toughness").
    SameToughness,
    /// Their mana values add up to at most this ("with total mana value 3 or less").
    TotalManaValueAtMost(Amount),
    /// Their powers add up to at most this ("with total power 10 or less").
    TotalPowerAtMost(Amount),
}

/// What a target can be.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Targetable {
    /// Any target: a creature, planeswalker, battle or player.
    AnyTarget,
    /// A player.
    Player,
    /// An opponent of the controller.
    Opponent,
    /// An object matching the selector, in the zone the selector names.
    Object(Selector),
    /// Any of the listed kinds.
    Either(Vec<Targetable>),
    /// For each player `players` names, an object of `what` that player controls (owns, outside the battlefield): at
    /// most one target of each player ("for each opponent, destroy up to one target artifact that player controls").
    PerPlayer {
        /// The players the targets are chosen for.
        players: Ref,
        /// What each may be, without the player.
        what: Selector,
    },
    /// Targets of `what` that together obey `rule` ("two target creatures controlled by the same player").
    Together {
        /// What each may be.
        what: Box<Targetable>,
        /// What the chosen targets share or differ in.
        rule: TargetSet,
    },
    /// "Another target": what `what` allows except the targets chosen for the earlier specifications `earlier`,
    /// players among them (CR 115.3).
    Another {
        /// What each may be.
        what: Box<Targetable>,
        /// The indices of the earlier specifications whose targets are left out.
        earlier: Vec<u8>,
    },
    /// "Target creature of their choice": what `what` allows, chosen by the first player `who` names in turn order from
    /// the controller, though the spell or ability of the controller targets it.
    ChosenBy {
        /// Who chooses.
        who: Ref,
        /// What may be chosen.
        what: Box<Targetable>,
    },
    /// "Any target chosen at random": what the inner kind allows, picked by the game's random source among the legal
    /// targets as the spell or ability is announced, not by a player (CR 115.7 rulings).
    AtRandom(Box<Targetable>),
    /// A player the reference names ("target opponent who lost life this turn": a `PlayersWhere`), worked out as the
    /// target is chosen and again on resolution (CR 608.2b).
    PlayerAmong(Ref),
}

impl Targetable {
    /// The objects a `PerPlayer` lets a spell target, as one selector: `what` controlled by the players, or owned by
    /// them for a selector of another zone.
    pub fn c10_dice_per_player_objects(players: &Ref, what: &Selector) -> Selector {
        let elsewhere = what
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::In(zone) if *zone != ZoneKind::Battlefield));
        let mut all = what.0.clone();
        all.push(if elsewhere {
            Pred::OwnedBy(players.clone())
        } else {
            Pred::ControlledBy(players.clone())
        });
        Selector(all)
    }
}

/// How many of something: at least `min`, at most `max`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    /// The fewest allowed; an amount for "X target creatures".
    #[serde(default = "Amount::one")]
    pub min: Amount,
    /// The most allowed; `null` means any number.
    #[serde(default = "Bounds::one_max")]
    pub max: Option<Amount>,
}

impl Bounds {
    /// Between none and `max`: "up to two", and every number when `max` is `None`.
    pub const fn up_to(max: Option<Amount>) -> Bounds {
        Bounds {
            min: Amount::Fixed(0),
            max,
        }
    }

    /// Exactly `count`, a number or an amount such as X.
    pub fn exactly(count: &Amount) -> Bounds {
        Bounds {
            min: count.clone(),
            max: Some(count.clone()),
        }
    }

    /// The least as a number, when it is fixed.
    pub fn fixed_min(&self) -> Option<u32> {
        match self.min {
            Amount::Fixed(min) => Some(u32::try_from(min).unwrap_or(0)),
            _ => None,
        }
    }

    // serde's `default` must return the field's own type, which is an `Option` here.
    #[allow(clippy::unnecessary_wraps)]
    fn one_max() -> Option<Amount> {
        Some(Amount::Fixed(1))
    }

    /// Whether this is exactly one, so serialization can leave it out.
    pub fn is_one(&self) -> bool {
        *self == Bounds::default()
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            min: Amount::one(),
            max: Some(Amount::one()),
        }
    }
}

/// One "target ..." of a spell or ability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    /// What may be targeted.
    pub what: Targetable,
    /// How many targets are chosen.
    #[serde(default, skip_serializing_if = "Bounds::is_one")]
    pub bounds: Bounds,
    /// The amount divided among the targets as the controller chooses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divide: Option<Amount>,
}

#[cfg(test)]
mod tests {
    //! Goal: references, selectors and target specifications use the compact JSON forms the card files rely on.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::unit(Ref::You, r#""You""#)]
    #[case::target(Ref::Target(1), r#"{"Target":1}"#)]
    #[case::bound(Ref::Bound("found".into()), r#"{"Bound":"found"}"#)]
    #[case::nested(
        Ref::ControllerOf(Box::new(Ref::Target(0))),
        r#"{"ControllerOf":{"Target":0}}"#
    )]
    #[case::each(
        Ref::Each(Selector(vec![Pred::Type(CardType::Creature), Pred::Other])),
        r#"{"Each":[{"Type":"Creature"},"Other"]}"#
    )]
    fn refs_have_stable_json(#[case] value: Ref, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Ref>(json).unwrap(), value);
    }

    #[rstest]
    #[case::a_plus_counter(
        Pred::HasCounter(CounterKind::PlusOnePlusOne),
        r#"{"HasCounter":"+1/+1"}"#
    )]
    #[case::a_named_counter(
        Pred::HasCounter(CounterKind::Other("quest".into())),
        r#"{"HasCounter":{"Other":"quest"}}"#
    )]
    #[case::no_counter(
        Pred::Not(Box::new(Pred::HasCounter(CounterKind::Poison))),
        r#"{"Not":{"HasCounter":"poison"}}"#
    )]
    fn predicates_over_counters_have_stable_json(#[case] value: Pred, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Pred>(json).unwrap(), value);
    }

    #[rstest]
    #[case::any_cast(Pred::WasCast { by: None, from: None }, r#"{"WasCast":{}}"#)]
    #[case::cast_by_you_from_hand(
        Pred::WasCast { by: Some(Ref::You), from: Some(ZoneKind::Hand) },
        r#"{"WasCast":{"by":"You","from":"Hand"}}"#
    )]
    #[case::cast_with(Pred::CastWith(CastWay::Flashback), r#"{"CastWith":"Flashback"}"#)]
    #[case::top(Pred::TopOfLibrary, r#""TopOfLibrary""#)]
    #[case::bottom(Pred::BottomOfLibrary, r#""BottomOfLibrary""#)]
    #[case::top_grave(Pred::TopOfGraveyard, r#""TopOfGraveyard""#)]
    #[case::blocked_by(Pred::BlockedBy(Ref::This), r#"{"BlockedBy":"This"}"#)]
    #[case::blocks(Pred::Blocks(Ref::This), r#"{"Blocks":"This"}"#)]
    #[case::targeted_by(Pred::TargetedBy(Ref::Triggering), r#"{"TargetedBy":"Triggering"}"#)]
    #[case::greatest(
        Pred::GreatestAmong { stat: StatKind::Power, group: Selector(vec![Pred::Other]) },
        r#"{"GreatestAmong":{"stat":"Power","group":["Other"]}}"#
    )]
    #[case::suspended(Pred::Suspended, r#""Suspended""#)]
    #[case::kicked(Pred::Kicked, r#""Kicked""#)]
    #[case::additional_cost_paid(Pred::AdditionalCostPaid, r#""AdditionalCostPaid""#)]
    fn the_cast_library_combat_and_greatest_atoms_have_stable_json(
        #[case] value: Pred,
        #[case] json: &str,
    ) {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Pred>(json).unwrap(), value);
    }

    #[test]
    fn a_target_spec_leaves_out_its_defaults() {
        let spec = TargetSpec {
            what: Targetable::Opponent,
            bounds: Bounds::default(),
            divide: None,
        };
        assert_eq!(
            serde_json::to_string(&spec).unwrap(),
            r#"{"what":"Opponent"}"#
        );
        let any = TargetSpec {
            what: Targetable::AnyTarget,
            bounds: Bounds::up_to(None),
            divide: Some(Amount::Fixed(2)),
        };
        let json = serde_json::to_string(&any).unwrap();
        assert_eq!(
            json,
            r#"{"what":"AnyTarget","bounds":{"min":0,"max":null},"divide":2}"#
        );
        assert_eq!(serde_json::from_str::<TargetSpec>(&json).unwrap(), any);
    }

    #[test]
    fn a_missing_bound_is_one_and_null_is_unbounded() {
        let one: Bounds = serde_json::from_str("{}").unwrap();
        assert_eq!(one, Bounds::default());
        let any: Bounds = serde_json::from_str(r#"{"min":0,"max":null}"#).unwrap();
        assert_eq!(any.max, None);
    }

    #[test]
    fn unknown_fields_are_refused() {
        let error = serde_json::from_str::<TargetSpec>(r#"{"what":"Player","count":2}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field `count`"), "{error}");
    }
}
