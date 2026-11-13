//! The card names first printed in the old expansions that cards name ("a name originally printed in the Antiquities
//! expansion"): Arabian Nights, Antiquities and Homelands, each the full list of names whose first printing is that set.

use serde::{Deserialize, Serialize};

/// An expansion whose original names a card names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FirstSet {
    /// Arabian Nights (1993).
    ArabianNights,
    /// Antiquities (1994).
    Antiquities,
    /// Homelands (1995).
    Homelands,
}

impl FirstSet {
    /// The name of the expansion as cards print it.
    pub const fn title(self) -> &'static str {
        match self {
            FirstSet::ArabianNights => "Arabian Nights",
            FirstSet::Antiquities => "Antiquities",
            FirstSet::Homelands => "Homelands",
        }
    }

    /// Every expansion a card names this way.
    pub const ALL: [FirstSet; 3] = [
        FirstSet::ArabianNights,
        FirstSet::Antiquities,
        FirstSet::Homelands,
    ];

    /// The card names first printed in the expansion, sorted.
    pub const fn names(self) -> &'static [&'static str] {
        match self {
            FirstSet::ArabianNights => ARABIAN_NIGHTS,
            FirstSet::Antiquities => ANTIQUITIES,
            FirstSet::Homelands => HOMELANDS,
        }
    }

    /// Whether `name` was first printed in the expansion.
    pub fn has(self, name: &str) -> bool {
        self.names().binary_search(&name).is_ok()
    }
}

/// The names first printed in Arabian Nights.
const ARABIAN_NIGHTS: &[&str] = &[
    "Abu Ja'far",
    "Aladdin",
    "Aladdin's Lamp",
    "Aladdin's Ring",
    "Ali Baba",
    "Ali from Cairo",
    "Army of Allah",
    "Bazaar of Baghdad",
    "Bird Maiden",
    "Bottle of Suleiman",
    "Brass Man",
    "Camel",
    "City in a Bottle",
    "City of Brass",
    "Cuombajj Witches",
    "Cyclone",
    "Dancing Scimitar",
    "Dandân",
    "Desert",
    "Desert Nomads",
    "Desert Twister",
    "Diamond Valley",
    "Drop of Honey",
    "Ebony Horse",
    "El-Hajjâj",
    "Elephant Graveyard",
    "Erg Raiders",
    "Erhnam Djinn",
    "Eye for an Eye",
    "Fishliver Oil",
    "Flying Carpet",
    "Flying Men",
    "Ghazbán Ogre",
    "Giant Tortoise",
    "Guardian Beast",
    "Hasran Ogress",
    "Hurr Jackal",
    "Ifh-Bíff Efreet",
    "Island Fish Jasconius",
    "Island of Wak-Wak",
    "Jandor's Ring",
    "Jandor's Saddlebags",
    "Jeweled Bird",
    "Jihad",
    "Junún Efreet",
    "Juzám Djinn",
    "Khabál Ghoul",
    "King Suleiman",
    "Kird Ape",
    "Library of Alexandria",
    "Magnetic Mountain",
    "Merchant Ship",
    "Metamorphosis",
    "Mijae Djinn",
    "Moorish Cavalry",
    "Nafs Asp",
    "Oasis",
    "Old Man of the Sea",
    "Oubliette",
    "Piety",
    "Pyramids",
    "Repentant Blacksmith",
    "Ring of Ma'rûf",
    "Rukh Egg",
    "Sandals of Abdallah",
    "Sandstorm",
    "Serendib Djinn",
    "Serendib Efreet",
    "Shahrazad",
    "Sindbad",
    "Singing Tree",
    "Sorceress Queen",
    "Stone-Throwing Devils",
    "Unstable Mutation",
    "War Elephant",
    "Wyluli Wolf",
    "Ydwen Efreet",
];

/// The names first printed in Antiquities.
const ANTIQUITIES: &[&str] = &[
    "Amulet of Kroog",
    "Argivian Archaeologist",
    "Argivian Blacksmith",
    "Argothian Pixies",
    "Argothian Treefolk",
    "Armageddon Clock",
    "Artifact Blast",
    "Artifact Possession",
    "Artifact Ward",
    "Ashnod's Altar",
    "Ashnod's Battle Gear",
    "Ashnod's Transmogrant",
    "Atog",
    "Battering Ram",
    "Bronze Tablet",
    "Candelabra of Tawnos",
    "Circle of Protection: Artifacts",
    "Citanul Druid",
    "Clay Statue",
    "Clockwork Avian",
    "Colossus of Sardia",
    "Coral Helm",
    "Crumble",
    "Cursed Rack",
    "Damping Field",
    "Detonate",
    "Drafna's Restoration",
    "Dragon Engine",
    "Dwarven Weaponsmith",
    "Energy Flux",
    "Feldon's Cane",
    "Gaea's Avenger",
    "Gate to Phyrexia",
    "Goblin Artisans",
    "Golgothian Sylex",
    "Grapeshot Catapult",
    "Haunting Wind",
    "Hurkyl's Recall",
    "Ivory Tower",
    "Jalum Tome",
    "Martyrs of Korlis",
    "Mightstone",
    "Millstone",
    "Mishra's Factory",
    "Mishra's War Machine",
    "Mishra's Workshop",
    "Obelisk of Undoing",
    "Onulet",
    "Orcish Mechanics",
    "Ornithopter",
    "Phyrexian Gremlins",
    "Power Artifact",
    "Powerleech",
    "Priest of Yawgmoth",
    "Primal Clay",
    "Rakalite",
    "Reconstruction",
    "Reverse Polarity",
    "Rocket Launcher",
    "Sage of Lat-Nam",
    "Shapeshifter",
    "Shatterstorm",
    "Staff of Zegon",
    "Strip Mine",
    "Su-Chi",
    "Tablet of Epityr",
    "Tawnos's Coffin",
    "Tawnos's Wand",
    "Tawnos's Weaponry",
    "Tetravus",
    "The Rack",
    "Titania's Song",
    "Transmute Artifact",
    "Triskelion",
    "Urza's Avenger",
    "Urza's Chalice",
    "Urza's Mine",
    "Urza's Miter",
    "Urza's Power Plant",
    "Urza's Tower",
    "Wall of Spears",
    "Weakstone",
    "Xenic Poltergeist",
    "Yawgmoth Demon",
    "Yotian Soldier",
];

/// The names first printed in Homelands.
const HOMELANDS: &[&str] = &[
    "Abbey Gargoyles",
    "Abbey Matron",
    "Aether Storm",
    "Aliban's Tower",
    "Ambush",
    "Ambush Party",
    "An-Havva Constable",
    "An-Havva Inn",
    "An-Havva Township",
    "An-Zerrin Ruins",
    "Anaba Ancestor",
    "Anaba Bodyguard",
    "Anaba Shaman",
    "Anaba Spirit Crafter",
    "Apocalypse Chime",
    "Autumn Willow",
    "Aysen Abbey",
    "Aysen Bureaucrats",
    "Aysen Crusader",
    "Aysen Highway",
    "Baki's Curse",
    "Baron Sengir",
    "Beast Walkers",
    "Black Carriage",
    "Broken Visage",
    "Carapace",
    "Castle Sengir",
    "Cemetery Gate",
    "Chain Stasis",
    "Chandler",
    "Clockwork Gnomes",
    "Clockwork Steed",
    "Clockwork Swarm",
    "Coral Reef",
    "Dark Maze",
    "Daughter of Autumn",
    "Death Speakers",
    "Didgeridoo",
    "Drudge Spell",
    "Dry Spell",
    "Dwarven Pony",
    "Dwarven Sea Clan",
    "Dwarven Trader",
    "Ebony Rhino",
    "Eron the Relentless",
    "Evaporate",
    "Faerie Noble",
    "Feast of the Unicorn",
    "Feroz's Ban",
    "Folk of An-Havva",
    "Forget",
    "Funeral March",
    "Ghost Hounds",
    "Giant Albatross",
    "Giant Oyster",
    "Grandmother Sengir",
    "Greater Werewolf",
    "Hazduhr the Abbot",
    "Headstone",
    "Heart Wolf",
    "Hungry Mist",
    "Ihsan's Shade",
    "Irini Sengir",
    "Ironclaw Curse",
    "Jinx",
    "Joven",
    "Joven's Ferrets",
    "Joven's Tools",
    "Koskun Falls",
    "Koskun Keep",
    "Labyrinth Minotaur",
    "Leaping Lizard",
    "Leeches",
    "Mammoth Harness",
    "Marjhan",
    "Memory Lapse",
    "Merchant Scroll",
    "Mesa Falcon",
    "Mystic Decree",
    "Narwhal",
    "Orcish Mine",
    "Primal Order",
    "Prophecy",
    "Rashka the Slayer",
    "Reef Pirates",
    "Renewal",
    "Retribution",
    "Reveka, Wizard Savant",
    "Root Spider",
    "Roots",
    "Roterothopter",
    "Rysorian Badger",
    "Samite Alchemist",
    "Sea Sprite",
    "Sea Troll",
    "Sengir Autocrat",
    "Sengir Bats",
    "Serra Aviary",
    "Serra Bestiary",
    "Serra Inquisitors",
    "Serra Paladin",
    "Serrated Arrows",
    "Shrink",
    "Soraya the Falconer",
    "Spectral Bears",
    "Timmerian Fiends",
    "Torture",
    "Trade Caravan",
    "Truce",
    "Veldrane of Sengir",
    "Wall of Kelp",
    "Willow Faerie",
    "Willow Priestess",
    "Winter Sky",
    "Wizards' School",
];

#[cfg(test)]
mod tests {
    //! Goal: each list is sorted and unique (the lookup is a binary search) and holds the names of the cards that name it.
    use super::FirstSet;
    use rstest::rstest;

    #[test]
    fn r4707_each_list_is_sorted_and_unique() {
        for set in FirstSet::ALL {
            assert!(
                set.names().windows(2).all(|pair| pair[0] < pair[1]),
                "{set:?}"
            );
        }
    }

    #[rstest]
    #[case::city(FirstSet::ArabianNights, "City in a Bottle", true)]
    #[case::mountain_is_older(FirstSet::ArabianNights, "Mountain", false)]
    #[case::sylex(FirstSet::Antiquities, "Golgothian Sylex", true)]
    #[case::chime(FirstSet::Homelands, "Apocalypse Chime", true)]
    #[case::not_there(FirstSet::Homelands, "Golgothian Sylex", false)]
    #[case::with_apostrophe(FirstSet::Antiquities, "Urza's Mine", true)]
    fn r4707_a_name_is_found_in_its_first_set(
        #[case] set: FirstSet,
        #[case] name: &str,
        #[case] found: bool,
    ) {
        assert_eq!(set.has(name), found);
    }
}
