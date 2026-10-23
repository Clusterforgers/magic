//! Keywords: a card names them with their parameters; the engine implements each once.

use serde::{Deserialize, Serialize};

use crate::ability::Ability;
use crate::companion::DeckCondition;
use crate::cost::Cost;
use crate::refs::Selector;

/// A keyword ability with its parameters (CR 702).
///
/// The engine holds one rules value per variant, each in its own file, so a keyword task never
/// edits this enum (`docs/cards-dsl.md`, Keywords).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Keyword {
    /// Flash: may be cast any time the player could cast an instant.
    Flash,
    /// Flying.
    Flying,
    /// Reach.
    Reach,
    /// First strike.
    FirstStrike,
    /// Double strike.
    DoubleStrike,
    /// Deathtouch.
    Deathtouch,
    /// Trample.
    Trample,
    /// Lifelink.
    Lifelink,
    /// Vigilance.
    Vigilance,
    /// Haste.
    Haste,
    /// Menace.
    Menace,
    /// Defender.
    Defender,
    /// Indestructible.
    Indestructible,
    /// Hexproof.
    Hexproof,
    /// Shroud: can't be the target of any spell or ability.
    Shroud,
    /// Protection from a quality (CR 702.16): the sources the selector matches can't damage, enchant, equip, block or
    /// target the holder. The empty selector is protection from everything.
    Protection(Selector),
    /// Prowess.
    Prowess,
    /// Convoke.
    Convoke,
    /// Delve.
    Delve,
    /// Cascade.
    Cascade,
    /// Storm.
    Storm,
    /// Partner.
    Partner,
    /// Companion (CR 702.139): the card may be brought into the game from outside it when the starting deck meets the
    /// condition.
    Companion(Box<DeckCondition>),
    /// Battle cry.
    BattleCry,
    /// Fuse: both halves of a split card may be cast from hand together.
    Fuse,
    /// Ward with its cost.
    Ward(Cost),
    /// Equip with its cost.
    Equip(Cost),
    /// Reconfigure with its cost.
    Reconfigure(Cost),
    /// Living weapon: when the Equipment enters, a 0/0 black Phyrexian Germ is created and the Equipment attached to it.
    LivingWeapon,
    /// Enchant: what an Aura may enchant.
    Enchant(Selector),
    /// Enchant player or enchant opponent: an Aura that enchants a player (CR 303.4).
    EnchantPlayer {
        /// Whether only an opponent of the Aura's controller may be enchanted.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        opponent: bool,
    },
    /// Cycling with its cost.
    Cycling(Cost),
    /// Kicker with its cost.
    Kicker(Cost),
    /// Multikicker with its cost.
    Multikicker(Cost),
    /// Buyback with its cost: the spell returns to its owner's hand as it resolves when the cost was paid (CR 702.27).
    Buyback(Cost),
    /// Entwine with its cost: all modes are chosen when the cost is paid (CR 702.42).
    Entwine(Cost),
    /// Flashback with its cost.
    Flashback(Cost),
    /// Madness with its cost.
    Madness(Cost),
    /// Miracle with its cost: the card may be cast for it when it is the first card its owner drew this turn (CR 702.94).
    Miracle(Cost),
    /// Dredge N (CR 702.52): in a graveyard, a player who would draw a card may instead mill N cards and return this card to their hand.
    Dredge(u32),
    /// Morph with its cost: the card may be cast face down for {3}, and turned face up for this cost (CR 702.37).
    Morph(Cost),
    /// Megamorph with its cost: morph, and the permanent gets a +1/+1 counter as it is turned face up (CR 702.37b).
    Megamorph(Cost),
    /// Disguise with its cost: morph, and the permanent has ward {2} while it is face down (CR 702.168).
    Disguise(Cost),
    /// Suspend: the number of time counters and the cost.
    Suspend {
        /// Time counters put on the card in exile.
        time: u32,
        /// The suspend cost.
        cost: Cost,
    },
    /// Evoke with its cost.
    Evoke(Cost),
    /// Overload with its cost.
    Overload(Cost),
    /// Partner with the named card.
    PartnerWith(String),
    /// Changeling: the object is every creature type (CR 702.73).
    Changeling,
    /// Devoid: the object is colorless (CR 702.114).
    Devoid,
    /// Infect: damage to creatures is dealt as -1/-1 counters and to players as poison counters (CR 702.90).
    Infect,
    /// Myriad: when the creature attacks, token copies attack each other opponent (CR 702.116).
    Myriad,
    /// Crew with its power: tapping creatures with that total power turns a Vehicle into a creature (CR 702.122).
    Crew(u32),
    /// Mutate with its cost: the spell may be cast onto a non-Human creature (CR 702.140).
    Mutate(Cost),
    /// Encore with its cost: exiled from a graveyard, a token copy attacks each opponent (CR 702.141).
    Encore(Cost),
    /// Squad with its cost: paid any number of times as the spell is cast, each time a token copy enters (CR 702.157).
    Squad(Cost),
    /// Foretell with its cost: a special action from the hand exiles the card face down, and it is cast later for its foretell cost (CR 702.143).
    Foretell(Cost),
    /// Daybound (CR 702.145): the game becomes day when this permanent appears while it is neither, and the permanent is turned to its night face when it becomes night.
    Daybound,
    /// Nightbound (CR 702.145): the permanent is turned to its day face when it becomes day.
    Nightbound,
    /// Wither (CR 702.80): damage this deals to a creature is dealt in the form of -1/-1 counters.
    Wither,
    /// Toxic N (CR 702.164): players dealt combat damage by this creature also get N poison counters.
    Toxic(u32),
    /// Fear (CR 702.36): can't be blocked except by artifact creatures and/or black creatures.
    Fear,
    /// Intimidate (CR 702.13): can't be blocked except by artifact creatures and/or creatures that share a color with it.
    Intimidate,
    /// Landwalk (CR 702.14): can't be blocked as long as defending player controls a land the selector names (Islandwalk: an Island).
    Landwalk(Selector),
    /// Skulk (CR 702.118): can't be blocked by creatures with greater power.
    Skulk,
    /// Shadow (CR 702.28): can block or be blocked by only creatures with shadow.
    Shadow,
    /// Split second (CR 702.61): while this spell is on the stack, players can't cast spells or activate abilities that aren't mana abilities.
    SplitSecond,
    /// Undaunted (CR 702.125): this spell costs {1} less to cast for each opponent.
    Undaunted,
    /// Partner with a text (`Partner—Friends forever`, `Partner—Character select`): two commanders that both have the
    /// same text pair (CR 702.124).
    PartnerGroup(String),
    /// Doctor's companion: the second commander beside a legendary Time Lord Doctor (CR 702.124).
    DoctorsCompanion,
    /// Choose a Background: the second commander may be a Background enchantment (CR 702.124).
    ChooseABackground,
    /// Unearth with its cost: from a graveyard at sorcery speed the card returns with haste and is exiled at the next end step (CR 702.84).
    Unearth(Cost),
    /// Escape with its cost: the card may be cast from a graveyard for it, which includes exiling other cards of the graveyard (CR 702.138).
    Escape(Cost),
    /// Rebound: a spell cast from its owner's hand is exiled as it resolves and cast free at their next upkeep (CR 702.88).
    Rebound,
    /// Aftermath: the second half of a split card is cast only from a graveyard and is exiled when it leaves the stack (CR 702.127).
    Aftermath,
    /// Dash with its cost: an alternative cost for a creature spell, which gains haste and returns to its owner's hand at the next end step (CR 702.109).
    Dash(Cost),
    /// Bestow with its cost: the creature card may be cast as an Aura spell with enchant creature for it, and becomes a creature again when it is not attached (CR 702.103).
    Bestow(Cost),
    /// Blitz with its cost: an alternative cost for a creature spell, which gains haste and "When this creature dies, draw a card" and is sacrificed at the next end step (CR 702.152).
    Blitz(Cost),
    /// Prowl with its cost: an alternative cost for a spell, cast when a player was dealt combat damage this turn by a source its caster controlled that shared a creature type with it (CR 702.76).
    Prowl(Cost),
    /// Mayhem with its cost: an alternative cost to cast the card from its owner's graveyard in the turn they discarded it, with the timing the card has (CR 702.190).
    Mayhem(Cost),
    /// Ninjutsu with its cost: from the hand during combat after blockers are declared, an unblocked attacker returns to its owner's hand and the card enters tapped and attacking (CR 702.49).
    Ninjutsu(Cost),
    /// Echo with its cost (CR 702.30): at the beginning of its controller's upkeep, if it came under their control since their last turn began, it is sacrificed unless they pay the cost.
    Echo(Cost),
    /// Cumulative upkeep with its cost (CR 702.24): at the beginning of its controller's upkeep an age counter is put on it, then it is sacrificed unless they pay the cost once for each age counter.
    CumulativeUpkeep(Cost),
    /// Undying (CR 702.93): when it dies, if it had no +1/+1 counter on it, it returns with a +1/+1 counter.
    Undying,
    /// Persist (CR 702.79): when it dies, if it had no -1/-1 counter on it, it returns with a -1/-1 counter.
    Persist,
    /// Exalted (CR 702.83): whenever a creature you control attacks alone, it gets +1/+1 until end of turn.
    Exalted,
    /// Melee (CR 702.121): whenever it attacks, it gets +1/+1 until end of turn for each opponent you attacked this combat.
    Melee,
    /// Mentor (CR 702.134): whenever it attacks, a +1/+1 counter goes on target attacking creature with lesser power.
    Mentor,
    /// Dethrone (CR 702.105): whenever it attacks the player with the most life or tied for most life, it gets a +1/+1 counter.
    Dethrone,
    /// Annihilator N (CR 702.86): whenever it attacks, the defending player sacrifices N permanents.
    Annihilator(u32),
    /// Extort (CR 702.101): whenever you cast a spell, you may pay {W/B}; each opponent then loses 1 life and you gain that much.
    Extort,
    /// Evolve (CR 702.100): whenever a creature enters under your control with greater power or toughness, it gets a +1/+1 counter.
    Evolve,
    /// Level up with its cost (CR 702.87): "{cost}: Put a level counter on this permanent. Level up only as a sorcery."
    LevelUp(Cost),
    /// Outlast with its cost (CR 702.107): "{cost}, {T}: Put a +1/+1 counter on this permanent. Outlast only as a sorcery."
    Outlast(Cost),
    /// Saddle with its power (CR 702.171): tapping other creatures with that total power makes the Mount saddled until end of turn, at sorcery speed.
    Saddle(u32),
    /// Saddled (CR 702.171b): the designation of a Mount whose saddle ability resolved this turn, granted until end of turn.
    Saddled,
    /// Station (CR 702.184): tapping another creature puts charge counters equal to its power on the Spacecraft, at sorcery speed.
    Station,
    /// Ravenous (CR 702.156): it enters with X +1/+1 counters, and its controller draws a card when it enters if X is 5 or more.
    Ravenous,
    /// Jump-start (CR 702.133): the card may be cast from a graveyard by discarding a card in addition to its other costs, and is then exiled.
    JumpStart,
    /// Retrace (CR 702.81): the card may be cast from a graveyard by discarding a land card in addition to its other costs.
    Retrace,
    /// Embalm with its cost (CR 702.128): from a graveyard at sorcery speed the card is exiled for a token copy that is a white Zombie with no mana cost.
    Embalm(Cost),
    /// Eternalize with its cost (CR 702.129): from a graveyard at sorcery speed the card is exiled for a token copy that is a 4/4 black Zombie with no mana cost.
    Eternalize(Cost),
    /// Improvise (CR 702.126): each artifact tapped while the spell is cast pays for {1} of its cost.
    Improvise,
    /// Affinity (CR 702.41): this spell costs {1} less to cast for each permanent its controller controls that the selector names (Affinity for artifacts).
    Affinity(Selector),
    /// Typecycling (CR 702.29): "[Cost], Discard this card: Search your library for a card the selector names, reveal it, put it into your hand, then shuffle" (Plainscycling, basic landcycling); it is a kind of cycling.
    Typecycling {
        /// What the card searched for must be.
        find: Selector,
        /// The cost, paid beside the discard.
        cost: Cost,
    },
    /// Fabricate with its number (CR 702.123): "When this permanent enters, you may put N +1/+1 counters on it. If you don't, create N 1/1 colorless Servo artifact creature tokens."
    Fabricate(u32),
    /// Ascend (CR 702.131): a permanent with it gives its controller the city's blessing as soon as they control ten or more permanents; the designation lasts for the rest of the game.
    Ascend,
    /// Vanishing N (CR 702.63): it enters with N time counters, at the beginning of its controller's upkeep one is removed, and it is sacrificed when the last is removed.
    Vanishing(u32),
    /// Fading N (CR 702.32): it enters with N fade counters, at the beginning of its controller's upkeep one is removed, and it is sacrificed if none can be.
    Fading(u32),
    /// Backup N (CR 702.165): when it enters, N +1/+1 counters go on target creature; if that is another creature it gains `abilities`, those listed after the keyword on the card, until end of turn.
    Backup {
        /// How many +1/+1 counters.
        count: u32,
        /// The abilities that follow the keyword on the card, set by the compiler.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        abilities: Vec<Ability>,
    },
    /// Devour (CR 702.82): as it enters, its controller may sacrifice any number of other permanents `of` matches, and it enters with `counters` +1/+1 counters for each.
    Devour {
        /// The +1/+1 counters for each permanent sacrificed.
        counters: u32,
        /// What may be sacrificed: creatures, artifacts, lands or Foods.
        of: Selector,
    },
    /// Casualty N (CR 702.153): as an additional cost to cast the spell, its caster may sacrifice a creature with power N or greater; when they cast it, if they did, they copy it.
    Casualty(u32),
    /// Offspring with its cost (CR 702.175): the caster may pay it in addition as they cast the spell; when the creature enters, if they did, they create a 1/1 token copy of it.
    Offspring(Cost),
    /// Demonstrate (CR 702.144): when the spell is cast, its caster may copy it and, if they do, choose an opponent who also copies it.
    Demonstrate,
    /// Hideaway N (CR 702.75): when the permanent enters, its controller looks at the top N cards of their library, exiles one face down and puts the rest on the bottom in a random order.
    Hideaway(u32),
    /// Gift (CR 702.174): as an additional cost the caster may promise an opponent a gift; the chosen opponent gets it as the spell resolves or when the permanent enters, before other effects.
    Gift(GiftKind),
    /// For Mirrodin! (CR 702.163): when this Equipment enters, its controller creates a 2/2 red Rebel creature token, then attaches the Equipment to it.
    ForMirrodin,
    /// Job select: when this Equipment enters, its controller creates a 1/1 colorless Hero creature token, then attaches the Equipment to it.
    JobSelect,
    /// Equip with a restriction and its cost (CR 702.6): "[Cost]: Attach this permanent to target creature you control that `to` also names. Activate only as a sorcery" (Equip legendary creature, Equip commander, Equip Knight).
    EquipTo {
        /// What the creature must also be: legendary, a commander, a token, a creature type.
        to: Box<Selector>,
        /// The cost, paid as the cost of the activated ability the keyword adds.
        cost: Cost,
    },
    /// Scavenge with its cost (CR 702.97): from a graveyard at sorcery speed the card is exiled for as many +1/+1 counters as its power on target creature.
    Scavenge(Cost),
    /// Transmute with its cost (CR 702.53): from a hand at sorcery speed the card is discarded to search the library for a card with the same mana value.
    Transmute(Cost),
    /// Awaken N with its cost (CR 702.113): the spell may be cast from a hand for the cost, and then also puts N +1/+1 counters on target land its caster controls, which becomes a 0/0 Elemental creature with haste.
    Awaken {
        /// The +1/+1 counters put on the land.
        count: u32,
        /// The alternative cost.
        cost: Cost,
    },
    /// Gravestorm (CR 702.69): when the spell is cast, its caster copies it for each permanent put into a graveyard from the battlefield this turn.
    Gravestorm,
    /// Sunburst (CR 702.44): the permanent enters with a +1/+1 counter (a charge counter unless it is a creature) for each color of mana spent to cast it.
    Sunburst,
}

/// What a promised gift is (CR 702.174).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GiftKind {
    /// The opponent draws a card.
    Card,
    /// The opponent creates a Treasure token.
    Treasure,
    /// The opponent creates a Food token.
    Food,
    /// The opponent creates a tapped 1/1 blue Fish creature token.
    TappedFish,
    /// The opponent creates an 8/8 blue Octopus creature token.
    Octopus,
    /// The opponent takes an extra turn after this one.
    ExtraTurn,
}

impl Keyword {
    /// Companion with this condition; the condition is boxed so a keyword, and every event that holds one, stays small.
    pub fn companion(condition: DeckCondition) -> Keyword {
        Keyword::Companion(Box::new(condition))
    }

    /// Scryfall's spelling of every keyword in this enum, for matching a record's `keywords`.
    pub const NAMES: &[&str] = &[
        "Flash",
        "Flying",
        "Reach",
        "First strike",
        "Double strike",
        "Deathtouch",
        "Trample",
        "Lifelink",
        "Vigilance",
        "Haste",
        "Menace",
        "Defender",
        "Indestructible",
        "Hexproof",
        "Shroud",
        "Protection",
        "Prowess",
        "Convoke",
        "Delve",
        "Cascade",
        "Storm",
        "Partner",
        "Companion",
        "Battle cry",
        "Fuse",
        "Ward",
        "Equip",
        "Reconfigure",
        "Living weapon",
        "Enchant",
        "Enchant player",
        "Cycling",
        "Kicker",
        "Multikicker",
        "Buyback",
        "Entwine",
        "Flashback",
        "Madness",
        "Miracle",
        "Dredge",
        "Morph",
        "Megamorph",
        "Disguise",
        "Suspend",
        "Evoke",
        "Overload",
        "Partner with",
        "Changeling",
        "Devoid",
        "Infect",
        "Myriad",
        "Crew",
        "Mutate",
        "Encore",
        "Squad",
        "Foretell",
        "Daybound",
        "Nightbound",
        "Wither",
        "Toxic",
        "Fear",
        "Intimidate",
        "Landwalk",
        "Skulk",
        "Shadow",
        "Split second",
        "Undaunted",
        "Partner group",
        "Doctor's companion",
        "Choose a Background",
        "Unearth",
        "Escape",
        "Rebound",
        "Aftermath",
        "Dash",
        "Bestow",
        "Blitz",
        "Prowl",
        "Mayhem",
        "Ninjutsu",
        "Echo",
        "Cumulative upkeep",
        "Undying",
        "Persist",
        "Exalted",
        "Melee",
        "Mentor",
        "Dethrone",
        "Annihilator",
        "Extort",
        "Evolve",
        "Level up",
        "Outlast",
        "Saddle",
        "Saddled",
        "Station",
        "Ravenous",
        "Jump-start",
        "Retrace",
        "Embalm",
        "Eternalize",
        "Improvise",
        "Affinity",
        "Typecycling",
        "Fabricate",
        "Ascend",
        "Vanishing",
        "Fading",
        "Backup",
        "Devour",
        "Casualty",
        "Offspring",
        "Demonstrate",
        "Hideaway",
        "Gift",
        "For Mirrodin!",
        "Job select",
        "Equip to",
        "Scavenge",
        "Transmute",
        "Awaken",
        "Gravestorm",
        "Sunburst",
    ];

    /// The keyword as Scryfall spells it (`First strike`).
    pub const fn name(&self) -> &'static str {
        match self {
            Keyword::Flash => "Flash",
            Keyword::Flying => "Flying",
            Keyword::Reach => "Reach",
            Keyword::FirstStrike => "First strike",
            Keyword::DoubleStrike => "Double strike",
            Keyword::Deathtouch => "Deathtouch",
            Keyword::Trample => "Trample",
            Keyword::Lifelink => "Lifelink",
            Keyword::Vigilance => "Vigilance",
            Keyword::Haste => "Haste",
            Keyword::Menace => "Menace",
            Keyword::Defender => "Defender",
            Keyword::Indestructible => "Indestructible",
            Keyword::Hexproof => "Hexproof",
            Keyword::Shroud => "Shroud",
            Keyword::Protection(_) => "Protection",
            Keyword::Prowess => "Prowess",
            Keyword::Convoke => "Convoke",
            Keyword::Delve => "Delve",
            Keyword::Cascade => "Cascade",
            Keyword::Storm => "Storm",
            Keyword::Partner => "Partner",
            Keyword::Companion(_) => "Companion",
            Keyword::BattleCry => "Battle cry",
            Keyword::Fuse => "Fuse",
            Keyword::Ward(_) => "Ward",
            Keyword::Equip(_) => "Equip",
            Keyword::Reconfigure(_) => "Reconfigure",
            Keyword::LivingWeapon => "Living weapon",
            Keyword::Enchant(_) => "Enchant",
            Keyword::EnchantPlayer { .. } => "Enchant player",
            Keyword::Cycling(_) => "Cycling",
            Keyword::Kicker(_) => "Kicker",
            Keyword::Multikicker(_) => "Multikicker",
            Keyword::Buyback(_) => "Buyback",
            Keyword::Entwine(_) => "Entwine",
            Keyword::Flashback(_) => "Flashback",
            Keyword::Madness(_) => "Madness",
            Keyword::Miracle(_) => "Miracle",
            Keyword::Dredge(_) => "Dredge",
            Keyword::Morph(_) => "Morph",
            Keyword::Megamorph(_) => "Megamorph",
            Keyword::Disguise(_) => "Disguise",
            Keyword::Suspend { .. } => "Suspend",
            Keyword::Evoke(_) => "Evoke",
            Keyword::Overload(_) => "Overload",
            Keyword::PartnerWith(_) => "Partner with",
            Keyword::Changeling => "Changeling",
            Keyword::Devoid => "Devoid",
            Keyword::Infect => "Infect",
            Keyword::Myriad => "Myriad",
            Keyword::Crew(_) => "Crew",
            Keyword::Mutate(_) => "Mutate",
            Keyword::Encore(_) => "Encore",
            Keyword::Squad(_) => "Squad",
            Keyword::Foretell(_) => "Foretell",
            Keyword::Daybound => "Daybound",
            Keyword::Nightbound => "Nightbound",
            Keyword::Wither => "Wither",
            Keyword::Toxic(_) => "Toxic",
            Keyword::Fear => "Fear",
            Keyword::Intimidate => "Intimidate",
            Keyword::Landwalk(_) => "Landwalk",
            Keyword::Skulk => "Skulk",
            Keyword::Shadow => "Shadow",
            Keyword::SplitSecond => "Split second",
            Keyword::Undaunted => "Undaunted",
            Keyword::PartnerGroup(_) => "Partner group",
            Keyword::DoctorsCompanion => "Doctor's companion",
            Keyword::ChooseABackground => "Choose a Background",
            Keyword::Unearth(_) => "Unearth",
            Keyword::Escape(_) => "Escape",
            Keyword::Rebound => "Rebound",
            Keyword::Aftermath => "Aftermath",
            Keyword::Bestow(_) => "Bestow",
            Keyword::Ninjutsu(_) => "Ninjutsu",
            Keyword::Echo(_) => "Echo",
            Keyword::CumulativeUpkeep(_) => "Cumulative upkeep",
            Keyword::Undying => "Undying",
            Keyword::Persist => "Persist",
            other => other.name_of_later(),
        }
    }

    /// The name of the keywords declared after persist, apart from `name` so that neither outgrows a function's size.
    const fn name_of_later(&self) -> &'static str {
        match self {
            Keyword::Exalted => "Exalted",
            Keyword::Melee => "Melee",
            Keyword::Mentor => "Mentor",
            Keyword::Dethrone => "Dethrone",
            Keyword::Annihilator(_) => "Annihilator",
            Keyword::Extort => "Extort",
            Keyword::Evolve => "Evolve",
            Keyword::LevelUp(_) => "Level up",
            Keyword::Outlast(_) => "Outlast",
            Keyword::Saddle(_) => "Saddle",
            Keyword::Saddled => "Saddled",
            Keyword::Station => "Station",
            Keyword::Ravenous => "Ravenous",
            Keyword::JumpStart => "Jump-start",
            Keyword::Retrace => "Retrace",
            Keyword::Embalm(_) => "Embalm",
            Keyword::Eternalize(_) => "Eternalize",
            Keyword::Improvise => "Improvise",
            Keyword::Affinity(_) => "Affinity",
            Keyword::Typecycling { .. } => "Typecycling",
            Keyword::Fabricate(_) => "Fabricate",
            Keyword::Ascend => "Ascend",
            other => other.counter_name(),
        }
    }

    /// The names of the keywords that count, copy or devour (`name` is split to stay short; a test holds every keyword to its entry in `NAMES`).
    const fn counter_name(&self) -> &'static str {
        match self {
            Keyword::Vanishing(_) => "Vanishing",
            Keyword::Fading(_) => "Fading",
            Keyword::Backup { .. } => "Backup",
            Keyword::Devour { .. } => "Devour",
            Keyword::Casualty(_) => "Casualty",
            Keyword::Offspring(_) => "Offspring",
            Keyword::Demonstrate => "Demonstrate",
            Keyword::Hideaway(_) => "Hideaway",
            Keyword::Gift(_) => "Gift",
            Keyword::ForMirrodin => "For Mirrodin!",
            Keyword::JobSelect => "Job select",
            Keyword::EquipTo { .. } => "Equip to",
            Keyword::Scavenge(_) => "Scavenge",
            Keyword::Transmute(_) => "Transmute",
            Keyword::Awaken { .. } => "Awaken",
            Keyword::Gravestorm => "Gravestorm",
            Keyword::Sunburst => "Sunburst",
            Keyword::Dash(_) => "Dash",
            Keyword::Blitz(_) => "Blitz",
            Keyword::Prowl(_) => "Prowl",
            Keyword::Mayhem(_) => "Mayhem",
            // The keywords before it are named by `name`; the test of the names lists every variant.
            _ => "",
        }
    }

    /// Whether Scryfall's keyword text names a keyword of this enum (case-insensitive).
    pub fn is_known_name(printed: &str) -> bool {
        Keyword::NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(printed))
    }

    /// Whether the keyword gives the caster an optional additional cost that `Condition::AdditionalCostPaid` reads: casualty, offspring and gift.
    pub const fn has_additional_cost(&self) -> bool {
        matches!(
            self,
            Keyword::Casualty(_) | Keyword::Offspring(_) | Keyword::Gift(_)
        )
    }

    /// Whether paying the cost of this keyword counts as a kick (`Amount::Kicks`): kicker, multikicker and squad.
    pub const fn is_kicker(&self) -> bool {
        matches!(
            self,
            Keyword::Kicker(_) | Keyword::Multikicker(_) | Keyword::Squad(_)
        )
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every keyword has a unique Scryfall name listed in `NAMES`, and keywords serialize as plain words or single-key maps.
    use super::*;
    use rstest::rstest;

    #[allow(clippy::too_many_lines)]
    fn samples() -> Vec<Keyword> {
        let cost = Cost::default();
        vec![
            Keyword::Flash,
            Keyword::Flying,
            Keyword::Reach,
            Keyword::FirstStrike,
            Keyword::DoubleStrike,
            Keyword::Deathtouch,
            Keyword::Trample,
            Keyword::Lifelink,
            Keyword::Vigilance,
            Keyword::Haste,
            Keyword::Menace,
            Keyword::Defender,
            Keyword::Indestructible,
            Keyword::Hexproof,
            Keyword::Shroud,
            Keyword::Protection(Selector::default()),
            Keyword::Prowess,
            Keyword::Convoke,
            Keyword::Delve,
            Keyword::Cascade,
            Keyword::Storm,
            Keyword::Partner,
            Keyword::companion(DeckCondition::ExtraCards(20)),
            Keyword::BattleCry,
            Keyword::Fuse,
            Keyword::Ward(cost.clone()),
            Keyword::Equip(cost.clone()),
            Keyword::Reconfigure(cost.clone()),
            Keyword::LivingWeapon,
            Keyword::Enchant(Selector::default()),
            Keyword::EnchantPlayer { opponent: false },
            Keyword::Cycling(cost.clone()),
            Keyword::Kicker(cost.clone()),
            Keyword::Multikicker(cost.clone()),
            Keyword::Buyback(cost.clone()),
            Keyword::Entwine(cost.clone()),
            Keyword::Flashback(cost.clone()),
            Keyword::Madness(cost.clone()),
            Keyword::Miracle(cost.clone()),
            Keyword::Dredge(3),
            Keyword::Morph(cost.clone()),
            Keyword::Megamorph(cost.clone()),
            Keyword::Disguise(cost.clone()),
            Keyword::Suspend {
                time: 3,
                cost: cost.clone(),
            },
            Keyword::Evoke(cost.clone()),
            Keyword::Overload(cost.clone()),
            Keyword::PartnerWith("Some Card".into()),
            Keyword::Changeling,
            Keyword::Devoid,
            Keyword::Infect,
            Keyword::Myriad,
            Keyword::Crew(3),
            Keyword::Mutate(cost.clone()),
            Keyword::Encore(cost.clone()),
            Keyword::Squad(cost.clone()),
            Keyword::Foretell(cost.clone()),
            Keyword::Daybound,
            Keyword::Nightbound,
            Keyword::Wither,
            Keyword::Toxic(1),
            Keyword::Fear,
            Keyword::Intimidate,
            Keyword::Landwalk(Selector::default()),
            Keyword::Skulk,
            Keyword::Shadow,
            Keyword::SplitSecond,
            Keyword::Undaunted,
            Keyword::PartnerGroup("Friends forever".into()),
            Keyword::DoctorsCompanion,
            Keyword::ChooseABackground,
            Keyword::Unearth(cost.clone()),
            Keyword::Escape(cost.clone()),
            Keyword::Rebound,
            Keyword::Aftermath,
            Keyword::Dash(cost.clone()),
            Keyword::Bestow(cost.clone()),
            Keyword::Blitz(cost.clone()),
            Keyword::Prowl(cost.clone()),
            Keyword::Mayhem(cost.clone()),
            Keyword::Ninjutsu(cost),
            Keyword::Echo(Cost::default()),
            Keyword::CumulativeUpkeep(Cost::default()),
            Keyword::Undying,
            Keyword::Persist,
            Keyword::Exalted,
            Keyword::Melee,
            Keyword::Mentor,
            Keyword::Dethrone,
            Keyword::Annihilator(2),
            Keyword::Extort,
            Keyword::Evolve,
            Keyword::LevelUp(Cost::default()),
            Keyword::Outlast(Cost::default()),
            Keyword::Saddle(2),
            Keyword::Saddled,
            Keyword::Station,
            Keyword::Ravenous,
            Keyword::JumpStart,
            Keyword::Retrace,
            Keyword::Embalm(Cost::default()),
            Keyword::Eternalize(Cost::default()),
            Keyword::Improvise,
            Keyword::Affinity(Selector::default()),
            Keyword::Typecycling {
                find: Selector::default(),
                cost: Cost::default(),
            },
            Keyword::Fabricate(1),
            Keyword::Ascend,
            Keyword::Vanishing(3),
            Keyword::Fading(3),
            Keyword::Backup {
                count: 1,
                abilities: vec![Ability::Keyword(Keyword::Flying)],
            },
            Keyword::Devour {
                counters: 2,
                of: Selector::default(),
            },
            Keyword::Casualty(2),
            Keyword::Offspring(Cost::default()),
            Keyword::Demonstrate,
            Keyword::Hideaway(4),
            Keyword::Gift(GiftKind::Card),
            Keyword::ForMirrodin,
            Keyword::JobSelect,
            Keyword::EquipTo {
                to: Box::default(),
                cost: Cost::default(),
            },
            Keyword::Scavenge(Cost::default()),
            Keyword::Transmute(Cost::default()),
            Keyword::Awaken {
                count: 3,
                cost: Cost::default(),
            },
            Keyword::Gravestorm,
            Keyword::Sunburst,
        ]
    }

    #[test]
    fn names_are_unique_and_listed() {
        let names: Vec<_> = samples().iter().map(Keyword::name).collect();
        assert_eq!(names, Keyword::NAMES);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }

    #[test]
    fn names_list_every_variant_once_in_any_case() {
        // Rust cannot count the variants of an enum, so the source is counted: rustfmt keeps one variant per
        // line at four spaces, while docs, attributes and the fields of a struct variant do not start with a capital there.
        let source = include_str!("keyword.rs");
        let body = source
            .split("pub enum Keyword {")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .unwrap_or_default();
        let variants = body
            .lines()
            .filter_map(|line| line.strip_prefix("    "))
            .filter(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
            .count();
        assert!(variants > 0, "the enum was not found in the source");
        assert_eq!(Keyword::NAMES.len(), variants);
        let lowercase: std::collections::BTreeSet<_> = Keyword::NAMES
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        assert_eq!(lowercase.len(), Keyword::NAMES.len());
    }

    #[rstest]
    #[case("flying", true)]
    #[case("First Strike", true)]
    #[case("Battle Cry", true)]
    #[case("Landfall", false)]
    #[case("Scry", false)]
    fn printed_names_match_case_insensitively(#[case] printed: &str, #[case] known: bool) {
        assert_eq!(Keyword::is_known_name(printed), known);
    }

    #[test]
    fn keywords_round_trip_through_json() {
        for keyword in samples() {
            let json = serde_json::to_string(&keyword).unwrap();
            assert_eq!(serde_json::from_str::<Keyword>(&json).unwrap(), keyword);
        }
        assert_eq!(
            serde_json::to_string(&Keyword::Flying).unwrap(),
            "\"Flying\""
        );
    }
}
