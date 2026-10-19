import type { DeckInfo } from '@/net/api'

const creatures = (names: string[]): DeckInfo['cards'] => names.map((name) => ({ name, count: 4 }))

/** The three built-in decks as `GET /api/demo-decks` lists them (crates/mtg-server/src/demo.rs). */
export const DECKS: DeckInfo[] = [
  {
    id: 'mono-green',
    name: 'Mono Green Stompers',
    color: 'Green',
    size: 60,
    cards: [
      { name: 'Forest', count: 24 },
      ...creatures([
        'Runeclaw Bear',
        'Grizzly Bears',
        'Elvish Warrior',
        'Kalonian Tusker',
        'Centaur Courser',
        'Alpine Grizzly',
        'Leatherback Baloth',
        'Spined Wurm',
        'Craw Wurm',
      ]),
    ],
  },
  {
    id: 'mono-red',
    name: 'Mono Red Giants',
    color: 'Red',
    size: 60,
    cards: [
      { name: 'Mountain', count: 24 },
      ...creatures([
        "Mons's Goblin Raiders",
        'Goblin Piker',
        'Goblin Bully',
        'Gray Ogre',
        'Viashino Warrior',
        'Hill Giant',
        'Cyclops of One-Eyed Pass',
        'Earth Elemental',
        'Fire Elemental',
      ]),
    ],
  },
  {
    id: 'mono-white',
    name: 'Mono White Weenies',
    color: 'White',
    size: 60,
    cards: [
      { name: 'Plains', count: 24 },
      ...creatures([
        'Savannah Lions',
        'Eager Cadet',
        'Glory Seeker',
        'Silvercoat Lion',
        'Pearled Unicorn',
        'Border Guard',
        'Foot Soldiers',
        'Pillarfield Ox',
        'Siege Mastodon',
      ]),
    ],
  },
]
