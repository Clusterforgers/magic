/** The deck lists of `/dev/table?decks=public|hidden`: the viewer's own list and every opponent's, public or hidden. */
import type { ListCard, PlayerId, SeatList, TokenPicture } from '@/protocol/generated'

const line = (name: string, group: ListCard['group'], count = 1): ListCard => ({
  name,
  identity: { oracle_id: `dev-${name}` },
  count,
  group,
})

const GOBLIN: TokenPicture = {
  name: 'Goblin',
  colors: ['Red'],
  types: ['Creature'],
  subtypes: ['Goblin'],
  power: 1,
  toughness: 1,
}
const TREASURE: TokenPicture = { name: 'Treasure', types: ['Artifact'], subtypes: ['Treasure'] }
const SOLDIER: TokenPicture = {
  name: 'Soldier',
  colors: ['White'],
  types: ['Creature'],
  subtypes: ['Soldier'],
  power: 1,
  toughness: 1,
}

const GOBLIN_PRECON: ListCard[] = [
  line('Krenko, Mob Boss', 'Commander'),
  line('Goblin Instigator', 'Creature', 2),
  line('Legion Warboss', 'Creature'),
  line('Siege-Gang Commander', 'Creature'),
  line('Lightning Bolt', 'Instant'),
  line('Dragon Fodder', 'Sorcery'),
  line('Hordeling Outburst', 'Sorcery'),
  line('Smothering Tithe', 'Enchantment'),
  line('Mind Stone', 'Artifact'),
  line('Mountain', 'Land', 30),
]

const OWN: ListCard[] = [
  line('Elspeth Tirel', 'Planeswalker'),
  line('Raise the Alarm', 'Instant', 4),
  line('Plains', 'Land', 24),
]

/** The lists of a fixture table: the viewer's whole, each other seat whole (`public`) or its commander alone. */
export function devDeckLists(seats: number, viewer: PlayerId, kind: 'public' | 'hidden'): SeatList[] {
  return Array.from({ length: seats }, (_, seat): SeatList => {
    if (seat === viewer) return { seat, whole: true, cards: OWN, tokens: [SOLDIER] }
    if (kind === 'public') return { seat, whole: true, cards: GOBLIN_PRECON, tokens: [GOBLIN, TREASURE] }
    return { seat, whole: false, cards: GOBLIN_PRECON.slice(0, 1), tokens: [GOBLIN] }
  })
}
