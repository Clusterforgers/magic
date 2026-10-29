/**
 * `/dev/table?cards=real`: the fixture with the oracle ids of the real cards it names, read from the card search of a
 * running server, so the table draws real pictures; and a few permanents that show what pictures carry over them.
 */
import type { CardsApi } from '@/net/cards'
import type { Fixture } from './fixture'
import type { ObjectView } from '@/protocol/generated'

/** Oracle ids by card name. */
export type RealIds = Readonly<Record<string, string>>

/** The cards added to the viewer's battlefield: a token, a planeswalker, a transformed card and a pumped creature. */
export const SHOWCASE_NAMES = ['Jace Beleren', 'Delver of Secrets', 'Grizzly Bears'] as const

const base = (id: number, owner: number): Omit<ObjectView, 'name' | 'types' | 'colors'> => ({
  id,
  owner,
  controller: owner,
  zone: 'Battlefield',
  mana_cost: null,
  supertypes: [],
  subtypes: [],
  power: null,
  toughness: null,
  abilities: [],
  permanent: { tapped: false, damage: 0, summoning_sick: false },
})

function showcase(first: number, viewer: number, ids: RealIds): ObjectView[] {
  const identity = (name: string) => (ids[name] === undefined ? {} : { identity: { oracle_id: ids[name] } })
  return [
    {
      ...base(first, viewer),
      kind: 'Token',
      name: 'Soldier',
      types: ['Creature'],
      subtypes: ['Soldier'],
      colors: ['White'],
      power: 1,
      toughness: 1,
    },
    {
      ...base(first + 1, viewer),
      ...identity('Jace Beleren'),
      name: 'Jace Beleren',
      supertypes: ['Legendary'],
      types: ['Planeswalker'],
      subtypes: ['Jace'],
      colors: ['Blue'],
      counters: [['loyalty', 5]],
    },
    {
      ...base(first + 2, viewer),
      ...identity('Delver of Secrets'),
      name: 'Insectile Aberration',
      face: 1,
      types: ['Creature'],
      subtypes: ['Human', 'Insect'],
      colors: ['Blue'],
      power: 3,
      toughness: 2,
      keywords: ['Flying'],
    },
    {
      ...base(first + 3, viewer),
      ...identity('Grizzly Bears'),
      name: 'Grizzly Bears',
      types: ['Creature'],
      subtypes: ['Bear'],
      colors: ['Green'],
      power: 4,
      toughness: 4,
      counters: [['+1/+1', 2]],
      permanent: { tapped: true, damage: 1, summoning_sick: false },
    },
  ]
}

/** The fixture with every card it can name identified, and the showcase on the viewer's battlefield. */
export function withRealCards(fixture: Fixture, ids: RealIds, viewer: number): Fixture {
  const named = fixture.view.objects.map((object) =>
    ids[object.name] === undefined || object.kind !== undefined
      ? object
      : { ...object, identity: { oracle_id: ids[object.name] ?? '' } },
  )
  const first = Math.max(0, ...named.map((object) => object.id)) + 1
  const extra = showcase(first, viewer, ids)
  const cards = [...(fixture.view.battlefield.cards ?? []), ...extra.map((object) => object.id)]
  return {
    ...fixture,
    view: {
      ...fixture.view,
      objects: [...named, ...extra],
      battlefield: { count: cards.length, cards },
    },
  }
}

/** Looks each name up in the card search and keeps the exact match; one request per name, once. */
export async function fetchRealIds(api: CardsApi, names: readonly string[]): Promise<RealIds> {
  const found: Record<string, string> = {}
  for (const name of new Set(names)) {
    try {
      const page = await api.search({ text: name, perPage: 20 })
      const hit = page.cards.find((card) => card.name === name || card.name.startsWith(`${name} // `))
      if (hit !== undefined) found[name] = hit.oracle_id
    } catch {
      // The table keeps its text face for a card the server does not know.
    }
  }
  return found
}
