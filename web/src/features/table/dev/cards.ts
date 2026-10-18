/** A small catalogue of real card names with their types and costs, for the fixture. */
import type {
  CardType,
  Color,
  ManaCost,
  ManaSymbol,
  ManaType,
  Subtype,
  Supertype,
} from '@/protocol/generated'

/** What a fixture card is made from. */
export interface Template {
  name: string
  cost: ManaCost | null
  supertypes?: Supertype[]
  types: CardType[]
  subtypes?: Subtype[]
  colors: Color[]
  power?: number
  toughness?: number
  /** The mana a land or mana rock taps for. */
  produces?: ManaType
}

/** A list with at least one item, so picking from it by index always finds something. */
export type NonEmpty<T> = [T, ...T[]]

const COLORS: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']

// A cost of `generic` and the coloured symbols, counted in W U B R G order.
const cost = (
  generic: number,
  colored: [number, number, number, number, number] = [0, 0, 0, 0, 0],
): ManaCost => [
  ...(generic > 0 ? [{ Generic: generic }] : []),
  ...COLORS.flatMap((color, index) =>
    Array.from({ length: colored[index] ?? 0 }, (): ManaSymbol => ({ Colored: color })),
  ),
]

const basic = (name: string, produces: ManaType): Template => ({
  name,
  cost: null,
  supertypes: ['Basic'],
  types: ['Land'],
  subtypes: [name],
  colors: [],
  produces,
})

/** The five basic lands. */
export const LANDS: NonEmpty<Template> = [
  basic('Plains', 'White'),
  basic('Island', 'Blue'),
  basic('Swamp', 'Black'),
  basic('Mountain', 'Red'),
  basic('Forest', 'Green'),
]

const creature = (
  name: string,
  mana: ManaCost,
  colors: Color[],
  subtypes: Subtype[],
  power: number,
  toughness: number,
): Template => ({ name, cost: mana, types: ['Creature'], subtypes, colors, power, toughness })

/** Fourteen creatures in every frame colour, gold included. */
export const CREATURES: NonEmpty<Template> = [
  creature('Grizzly Bears', cost(1, [0, 0, 0, 0, 1]), ['Green'], ['Bear'], 2, 2),
  creature('Llanowar Elves', cost(0, [0, 0, 0, 0, 1]), ['Green'], ['Elf', 'Druid'], 1, 1),
  creature('Serra Angel', cost(3, [2, 0, 0, 0, 0]), ['White'], ['Angel'], 4, 4),
  creature('Hill Giant', cost(3, [0, 0, 0, 1, 0]), ['Red'], ['Giant'], 3, 3),
  creature('Air Elemental', cost(3, [0, 2, 0, 0, 0]), ['Blue'], ['Elemental'], 4, 4),
  creature('Gravedigger', cost(3, [0, 0, 1, 0, 0]), ['Black'], ['Zombie'], 2, 2),
  creature('Wind Drake', cost(2, [0, 1, 0, 0, 0]), ['Blue'], ['Drake'], 2, 2),
  creature('Goblin Chieftain', cost(1, [0, 0, 0, 2, 0]), ['Red'], ['Goblin'], 2, 2),
  creature('Pelakka Wurm', cost(4, [0, 0, 0, 0, 2]), ['Green'], ['Wurm'], 7, 7),
  creature('Selesnya Evangel', cost(2, [1, 0, 0, 0, 1]), ['White', 'Green'], ['Elf', 'Shaman'], 1, 1),
  creature('Izzet Chronarch', cost(3, [0, 1, 0, 1, 0]), ['Blue', 'Red'], ['Djinn', 'Wizard'], 3, 3),
  creature('Orzhov Pontiff', cost(1, [1, 0, 1, 0, 0]), ['White', 'Black'], ['Vampire', 'Cleric'], 2, 2),
  creature('Stonehorn Dignitary', cost(3, [1, 0, 0, 0, 0]), ['White'], ['Rhino', 'Monk'], 1, 3),
  creature('Vampire Nighthawk', cost(1, [0, 0, 2, 0, 0]), ['Black'], ['Vampire', 'Shade'], 2, 3),
]

const artifact = (name: string, mana: number, produces?: ManaType): Template => ({
  name,
  cost: cost(mana),
  types: ['Artifact'],
  colors: [],
  produces,
})

const enchantment = (name: string, mana: ManaCost, colors: Color[]): Template => ({
  name,
  cost: mana,
  types: ['Enchantment'],
  colors,
})

/** Six artifacts and four enchantments: the "other permanents" row. */
export const OTHERS: NonEmpty<Template> = [
  artifact('Sol Ring', 1, 'Colorless'),
  artifact('Mind Stone', 2, 'Colorless'),
  artifact('Bonesplitter', 1),
  artifact('Millstone', 2),
  artifact('Darksteel Ingot', 3, 'Colorless'),
  artifact("Wayfarer's Bauble", 1),
  enchantment('Pacifism', cost(1, [1, 0, 0, 0, 0]), ['White']),
  enchantment('Rancor', cost(0, [0, 0, 0, 0, 1]), ['Green']),
  enchantment('Phyrexian Arena', cost(1, [0, 0, 2, 0, 0]), ['Black']),
  enchantment('Propaganda', cost(2, [0, 1, 0, 0, 0]), ['Blue']),
]

const instant = (name: string, mana: ManaCost, colors: Color[]): Template => ({
  name,
  cost: mana,
  types: ['Instant'],
  colors,
})

/** Spells for the stack, the hand and the graveyards. */
export const INSTANTS: NonEmpty<Template> = [
  instant('Lightning Bolt', cost(0, [0, 0, 0, 1, 0]), ['Red']),
  instant('Counterspell', cost(0, [0, 2, 0, 0, 0]), ['Blue']),
  instant('Giant Growth', cost(0, [0, 0, 0, 0, 1]), ['Green']),
  instant('Swords to Plowshares', cost(0, [1, 0, 0, 0, 0]), ['White']),
  instant('Doom Blade', cost(1, [0, 0, 1, 0, 0]), ['Black']),
]

const commander = (name: string, mana: ManaCost, colors: Color[], subtypes: Subtype[]): Template => ({
  name,
  cost: mana,
  supertypes: ['Legendary'],
  types: ['Creature'],
  subtypes,
  colors,
  power: 3,
  toughness: 3,
})

/** Twelve legendary creatures, one per seat. */
export const COMMANDERS: NonEmpty<Template> = [
  commander('Kaalia of the Vast', cost(1, [1, 0, 1, 1, 0]), ['White', 'Black', 'Red'], ['Human', 'Cleric']),
  commander(
    'Atraxa, Praetors’ Voice',
    cost(0, [1, 1, 1, 0, 1]),
    ['White', 'Blue', 'Black', 'Green'],
    ['Angel', 'Horror'],
  ),
  commander('Krenko, Mob Boss', cost(2, [0, 0, 0, 2, 0]), ['Red'], ['Goblin', 'Warrior']),
  commander('Meren of Clan Nel Toth', cost(2, [0, 0, 1, 0, 1]), ['Black', 'Green'], ['Human', 'Shaman']),
  commander('Talrand, Sky Summoner', cost(2, [0, 2, 0, 0, 0]), ['Blue'], ['Merfolk', 'Wizard']),
  commander('Lyra Dawnbringer', cost(3, [2, 0, 0, 0, 0]), ['White'], ['Angel']),
  commander(
    'Ghave, Guru of Spores',
    cost(3, [1, 0, 1, 0, 1]),
    ['White', 'Black', 'Green'],
    ['Fungus', 'Shaman'],
  ),
  commander('Edgar Markov', cost(3, [1, 0, 1, 1, 0]), ['White', 'Black', 'Red'], ['Vampire', 'Knight']),
  commander(
    'Yarok, the Desecrated',
    cost(2, [0, 1, 1, 0, 1]),
    ['Blue', 'Black', 'Green'],
    ['Elemental', 'Horror'],
  ),
  commander('Sakashima of a Thousand Faces', cost(3, [0, 2, 0, 0, 0]), ['Blue'], ['Human', 'Rogue']),
  commander('Prosper, Tome-Bound', cost(2, [0, 0, 1, 1, 0]), ['Black', 'Red'], ['Tiefling', 'Warlock']),
  commander('Zur the Enchanter', cost(1, [1, 1, 1, 0, 0]), ['White', 'Blue', 'Black'], ['Human', 'Wizard']),
]
