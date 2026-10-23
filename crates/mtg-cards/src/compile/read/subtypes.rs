//! The subtypes of the game, so that a capitalized word in a clause can be told from a name.

/// The words after the dash of the type lines of creatures, artifacts, enchantments, lands and
/// spells in the Oracle data, sorted (a two-word type such as "Time Lord" is two entries).
const SUBTYPES: [&str; 435] = [
    "Adventure",
    "Advisor",
    "Aetherborn",
    "Alicorn",
    "Alien",
    "Ally",
    "Andorian",
    "Angel",
    "Antelope",
    "Ape",
    "Arcane",
    "Archer",
    "Archon",
    "Armadillo",
    "Armored",
    "Art",
    "Artificer",
    "Artist",
    "Assassin",
    "Assembly-Worker",
    "Astartes",
    "Athlete",
    "Atog",
    "Attraction",
    "Aura",
    "Aurochs",
    "Autobot",
    "Avatar",
    "Azra",
    "Background",
    "Badger",
    "Barbarian",
    "Bard",
    "Barnyard",
    "Basilisk",
    "Bat",
    "Bear",
    "Beast",
    "Beaver",
    "Beeble",
    "Beholder",
    "Berserker",
    "Bird",
    "Bison",
    "Blood",
    "Boar",
    "Bobblehead",
    "Book",
    "Borg",
    "Boxer",
    "Brainiac",
    "Bringer",
    "Brushwagg",
    "Bureaucrat",
    "C'tan",
    "Caitian",
    "Camel",
    "Capybara",
    "Carrier",
    "Cartouche",
    "Case",
    "Cat",
    "Cave",
    "Centaur",
    "Cephalid",
    "Chameleon",
    "Champion",
    "Chef",
    "Chicken",
    "Child",
    "Chimera",
    "Chorus",
    "Citizen",
    "Clamfolk",
    "Class",
    "Cleric",
    "Cloud",
    "Clown",
    "Clue",
    "Cockatrice",
    "Construct",
    "Contraption",
    "Cow",
    "Coward",
    "Coyote",
    "Crab",
    "Crocodile",
    "Curse",
    "Custodes",
    "Cyberman",
    "Cyborg",
    "Cyclops",
    "Dalek",
    "Dauthi",
    "Deer",
    "Demigod",
    "Demon",
    "Desert",
    "Designer",
    "Detective",
    "Devil",
    "Dinosaur",
    "Djinn",
    "Doctor",
    "Dog",
    "Donkey",
    "Dragon",
    "Drake",
    "Dreadnought",
    "Drix",
    "Drone",
    "Druid",
    "Dryad",
    "Dwarf",
    "Echidna",
    "Efreet",
    "Egg",
    "Elder",
    "Eldrazi",
    "Elemental",
    "Elephant",
    "Elf",
    "Elk",
    "Elves",
    "Employee",
    "Equipment",
    "Eternal",
    "Etiquette",
    "Eye",
    "Faerie",
    "Ferret",
    "Fire",
    "Fish",
    "Flagbearer",
    "Food",
    "Forest",
    "Fortification",
    "Fox",
    "Fractal",
    "Frog",
    "Fungus",
    "Gamer",
    "Gamma",
    "Gargoyle",
    "Gate",
    "Giant",
    "Giraffe",
    "Gith",
    "Glimmer",
    "Gnoll",
    "Gnome",
    "Goat",
    "Goblin",
    "God",
    "Golem",
    "Gorgon",
    "Gorn",
    "Grandchild",
    "Gremlin",
    "Griffin",
    "Guest",
    "Gus",
    "Hag",
    "Halfling",
    "Hamster",
    "Harpy",
    "Hatificer",
    "Hawk",
    "Head",
    "Hedgehog",
    "Hellion",
    "Hero",
    "Hippo",
    "Hippogriff",
    "Homarid",
    "Homunculus",
    "Horror",
    "Horse",
    "Human",
    "Hydra",
    "Hyena",
    "Illusion",
    "Imp",
    "Incarnation",
    "Incubator",
    "Infinity",
    "Inhuman",
    "Inkling",
    "Inquisitor",
    "Insect",
    "Island",
    "Jackal",
    "Jellyfish",
    "Judge",
    "Juggernaut",
    "Kangaroo",
    "Kavu",
    "Kelpien",
    "Key",
    "Killbot",
    "Kirin",
    "Kithkin",
    "Klingon",
    "Knight",
    "Koala",
    "Kobold",
    "Kor",
    "Kraken",
    "Kree",
    "Lady",
    "Lair",
    "Lamia",
    "Lammasu",
    "Lander",
    "Lanthanite",
    "Leech",
    "Lemur",
    "Lesson",
    "Leviathan",
    "Lhurgoyf",
    "Licid",
    "Lizard",
    "Lobster",
    "Locus",
    "Lord",
    "Luxior",
    "Mammoth",
    "Manticore",
    "Masticore",
    "Mercenary",
    "Merfolk",
    "Metathran",
    "Mime",
    "Mine",
    "Minion",
    "Minotaur",
    "Mite",
    "Mole",
    "Monger",
    "Mongoose",
    "Monk",
    "Monkey",
    "Moogle",
    "Moonfolk",
    "Mount",
    "Mountain",
    "Mouse",
    "Mummy",
    "Mutant",
    "Myr",
    "Mystic",
    "Nautilus",
    "Necron",
    "Nephilim",
    "Nightmare",
    "Nightstalker",
    "Ninja",
    "Noble",
    "Noggle",
    "Nomad",
    "Nymph",
    "Octopus",
    "Officer",
    "Ogre",
    "Omen",
    "Omenpath",
    "Ooze",
    "Orc",
    "Orgg",
    "Orion",
    "Otter",
    "Ouphe",
    "Ox",
    "Oyster",
    "Pangolin",
    "Paratrooper",
    "Peasant",
    "Pegasus",
    "Penguin",
    "Performer",
    "Pest",
    "Phelddagrif",
    "Phoenix",
    "Phyrexian",
    "Pilot",
    "Pirate",
    "Plains",
    "Plan",
    "Planet",
    "Plant",
    "Platypus",
    "Pony",
    "Porcupine",
    "Possum",
    "Power-Plant",
    "Powerstone",
    "Praetor",
    "Primarch",
    "Processor",
    "Proper",
    "Q",
    "Qu",
    "Quest",
    "Rabbit",
    "Raccoon",
    "Ranger",
    "Rat",
    "Realm",
    "Rebel",
    "Reflection",
    "Rhino",
    "Rigger",
    "Robot",
    "Rogue",
    "Role",
    "Room",
    "Rune",
    "Sable",
    "Saga",
    "Salamander",
    "Samurai",
    "Sand",
    "Saproling",
    "Satyr",
    "Scarecrow",
    "Scientist",
    "Scion",
    "Scorpion",
    "Scout",
    "Seal",
    "Serpent",
    "Servo",
    "Shade",
    "Shaman",
    "Shapeshifter",
    "Shark",
    "Sheep",
    "Shi'ar",
    "Ship",
    "Shrine",
    "Siren",
    "Skeleton",
    "Skrull",
    "Skunk",
    "Slith",
    "Sliver",
    "Sloth",
    "Slug",
    "Snail",
    "Snake",
    "Soldier",
    "Soltari",
    "Sorcerer",
    "Spacecraft",
    "Spawn",
    "Specter",
    "Spellshaper",
    "Sphere",
    "Sphinx",
    "Spider",
    "Spike",
    "Spirit",
    "Splinter",
    "Sponge",
    "Spuzzem",
    "Spy",
    "Squid",
    "Squirrel",
    "Starfish",
    "Stone",
    "Surrakar",
    "Survivor",
    "Swamp",
    "Symbiote",
    "Synth",
    "Talosian",
    "Tellarite",
    "Thalakos",
    "The",
    "Tholian",
    "Thopter",
    "Thrull",
    "Tiefling",
    "Time",
    "Tosk",
    "Tower",
    "Town",
    "Townsfolk",
    "Toy",
    "Trap",
    "Treasure",
    "Tree",
    "Treefolk",
    "Trilobite",
    "Triskelavite",
    "Troll",
    "Turtle",
    "Tyranid",
    "Ulamog's",
    "Unicorn",
    "Urza's",
    "Utrom",
    "Vampire",
    "Vampyre",
    "Varmint",
    "Vedalken",
    "Vehicle",
    "Vengeance",
    "Villain",
    "Volver",
    "Vorta",
    "Vulcan",
    "Waiter",
    "Wall",
    "Walrus",
    "Warlock",
    "Warrior",
    "Weasel",
    "Weird",
    "Werewolf",
    "Whale",
    "Wizard",
    "Wolf",
    "Wolverine",
    "Wombat",
    "Worm",
    "Wraith",
    "Wrenn",
    "Wrestler",
    "Wurm",
    "Xindi",
    "Yeti",
    "Zombie",
    "Zonian",
    "Zubera",
];

/// Whether the word is a subtype, as it is written (capitalized).
pub fn is_subtype(word: &str) -> bool {
    SUBTYPES.binary_search(&word).is_ok()
}

/// The subtypes whose plural is not the singular with an "s": the plural of each.
const IRREGULAR_PLURALS: [(&str, &str); 12] = [
    ("Cyclops", "Cyclopes"),
    ("Dwarf", "Dwarves"),
    ("Elf", "Elves"),
    ("Fungus", "Fungi"),
    ("Hero", "Heroes"),
    ("Homunculus", "Homunculi"),
    ("Locus", "Loci"),
    ("Mouse", "Mice"),
    ("Ox", "Oxen"),
    ("Thief", "Thieves"),
    ("Werewolf", "Werewolves"),
    ("Wolf", "Wolves"),
];

/// The subtypes written the same in the plural.
const SAME_PLURAL: [&str; 17] = [
    "Aurochs",
    "Dauthi",
    "Deer",
    "Djinn",
    "Eldrazi",
    "Equipment",
    "Fish",
    "Kavu",
    "Kithkin",
    "Kor",
    "Merfolk",
    "Moonfolk",
    "Plains",
    "Samurai",
    "Sheep",
    "Soltari",
    "Thalakos",
];

/// A subtype in the plural, the way a rules text writes a group of them ("Heroes", "Elves", "Spies", "Foxes").
pub fn plural_subtype(singular: &str) -> String {
    if SAME_PLURAL.contains(&singular) {
        return singular.to_owned();
    }
    if let Some((_, plural)) = IRREGULAR_PLURALS.iter().find(|(one, _)| *one == singular) {
        return (*plural).to_owned();
    }
    if let Some(stem) = singular.strip_suffix('y')
        && !stem.ends_with(['a', 'e', 'i', 'o', 'u'])
    {
        return format!("{stem}ies");
    }
    if singular.ends_with(['s', 'x', 'z']) || singular.ends_with("ch") || singular.ends_with("sh") {
        return format!("{singular}es");
    }
    format!("{singular}s")
}

/// The subtype a plural names, when the plural is that of a subtype `known` accepts: only a word that
/// `plural_subtype` writes from the subtype is read, so a rendering reads back as what it was.
pub fn singular_subtype(word: &str, known: impl Fn(&str) -> bool) -> Option<String> {
    let mut candidates = vec![word.to_owned()];
    candidates.extend(
        IRREGULAR_PLURALS
            .iter()
            .filter(|(_, plural)| *plural == word)
            .map(|(one, _)| (*one).to_owned()),
    );
    if let Some(stem) = word.strip_suffix("ies") {
        candidates.push(format!("{stem}y"));
    }
    for suffix in ["es", "s"] {
        if let Some(stem) = word.strip_suffix(suffix) {
            candidates.push(stem.to_owned());
        }
    }
    candidates
        .into_iter()
        .find(|one| known(one) && plural_subtype(one) == word)
}

#[cfg(test)]
mod tests {
    //! Goal: the subtype list stays sorted and holds the types the readers rely on and none of the names that look like them.
    use super::*;
    use rstest::rstest;

    #[test]
    fn the_list_is_sorted_and_free_of_duplicates() {
        assert!(SUBTYPES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[rstest]
    #[case("Clue")]
    #[case("Treasure")]
    #[case("Food")]
    #[case("Soldier")]
    #[case("Elf")]
    #[case("Goblin")]
    #[case("Aura")]
    #[case("Equipment")]
    #[case("Vehicle")]
    #[case("Merfolk")]
    #[case("Spirit")]
    fn the_types_the_readers_use_are_subtypes(#[case] word: &str) {
        assert!(is_subtype(word));
    }

    #[rstest]
    #[case("Goblin", "Goblins")]
    #[case("Hero", "Heroes")]
    #[case("Elf", "Elves")]
    #[case("Wolf", "Wolves")]
    #[case("Werewolf", "Werewolves")]
    #[case("Dwarf", "Dwarves")]
    #[case("Spy", "Spies")]
    #[case("Mercenary", "Mercenaries")]
    #[case("Donkey", "Donkeys")]
    #[case("Fox", "Foxes")]
    #[case("Sphinx", "Sphinxes")]
    #[case("Octopus", "Octopuses")]
    #[case("Leech", "Leeches")]
    #[case("Fungus", "Fungi")]
    #[case("Mouse", "Mice")]
    #[case("Plains", "Plains")]
    #[case("Fish", "Fish")]
    #[case("Samurai", "Samurai")]
    #[case("Equipment", "Equipment")]
    #[case("Armadillo", "Armadillos")]
    fn plurals_are_written_as_rules_text_writes_them(#[case] one: &str, #[case] many: &str) {
        assert_eq!(plural_subtype(one), many);
        assert_eq!(singular_subtype(many, is_subtype).as_deref(), Some(one));
    }

    #[test]
    fn every_subtype_of_the_list_reads_back_from_its_plural() {
        for one in SUBTYPES {
            let many = plural_subtype(one);
            assert_eq!(
                singular_subtype(&many, is_subtype).as_deref(),
                Some(one),
                "{one} -> {many}"
            );
        }
    }

    #[rstest]
    #[case("Elfs")]
    #[case("Goblinss")]
    #[case("Heroees")]
    #[case("Zorks")]
    #[case("Creatures")]
    #[case("Wolfs")]
    #[case("Mouses")]
    fn a_plural_that_is_not_one_of_a_subtype_is_not_read(#[case] word: &str) {
        assert_eq!(singular_subtype(word, is_subtype), None);
    }

    #[test]
    fn a_subtype_the_caller_knows_is_read_in_the_plural() {
        let known = |word: &str| word == "Gizmo" || word == "Gadfly";
        assert_eq!(singular_subtype("Gizmos", known).as_deref(), Some("Gizmo"));
        assert_eq!(
            singular_subtype("Gadflies", known).as_deref(),
            Some("Gadfly")
        );
        assert_eq!(singular_subtype("Widgets", known), None);
    }

    #[rstest]
    #[case("Commander")]
    #[case("Draw")]
    #[case("clue")]
    #[case("Goblins")]
    #[case("Sol")]
    #[case("")]
    fn other_words_are_not(#[case] word: &str) {
        assert!(!is_subtype(word));
    }
}
