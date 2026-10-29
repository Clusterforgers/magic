/**
 * `/dev/table?states=1`: the four-seat Commander fixture with one permanent for every state a card can show — counters,
 * loyalty, changed power and toughness, damage, attachments, other faces, face-down cards, copies, merges, prepared and
 * level — spread over the seats, and the printed cards that make the changed numbers read as buffed or debuffed.
 */
import type { CardDetail, ManualClause, ObjectView } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** The seats' permanents per seat in the base table, so the states have room (lands, creatures and others). */
export const STATES_BASE_PERMANENTS = 6

/** One printed face as the card endpoint gives it. */
interface Printed {
  name: string
  power: string | null
  toughness: string | null
  loyalty?: string
}

/** The printed faces by oracle id, for `printedDetail`. */
const PRINTED: Record<string, Printed[]> = {
  'st-bears': [{ name: 'Grizzly Bears', power: '2', toughness: '2' }],
  'st-giant': [{ name: 'Hill Giant', power: '3', toughness: '3' }],
  'st-lion': [{ name: 'Silvercoat Lion', power: '2', toughness: '2' }],
  'st-angel': [{ name: 'Serra Angel', power: '4', toughness: '4' }],
  'st-wall': [{ name: 'Wall of Omens', power: '0', toughness: '4' }],
  'st-delver': [
    { name: 'Delver of Secrets', power: '1', toughness: '1' },
    { name: 'Insectile Aberration', power: '3', toughness: '2' },
  ],
  'st-jace': [{ name: 'Jace Beleren', power: null, toughness: null, loyalty: '3' }],
  'st-liliana': [{ name: 'Liliana of the Veil', power: null, toughness: null, loyalty: '3' }],
  'st-teferi': [{ name: 'Teferi, Hero of Dominaria', power: null, toughness: null, loyalty: '4' }],
  'st-pegasus': [{ name: 'Pegasus Courser', power: '1', toughness: '3' }],
  'st-mage': [{ name: 'Wild Mage Apprentice', power: '2', toughness: '2' }],
  'st-oracle': [{ name: 'Verbose Oracle', power: '1', toughness: '1' }],
}

/** What the card endpoint would answer for a state card, so changed numbers can be compared with the printed ones. */
export function printedDetail(oracleId: string): Promise<CardDetail> {
  const faces = PRINTED[oracleId]
  if (faces === undefined) return Promise.reject(new Error(`no printed card ${oracleId}`))
  const stats = faces.map((face) => ({ ...face, loyalty: face.loyalty ?? null }))
  return Promise.resolve({ faces: stats } as unknown as CardDetail)
}

const base = (id: number, seat: number, over: Partial<ObjectView>): ObjectView => ({
  id,
  owner: seat,
  controller: seat,
  zone: 'Battlefield',
  name: 'Grizzly Bears',
  mana_cost: null,
  supertypes: [],
  types: ['Creature'],
  subtypes: [],
  colors: ['Green'],
  power: 2,
  toughness: 2,
  abilities: [],
  permanent: { tapped: false, damage: 0, summoning_sick: false },
  ...over,
})

const creature = (name: string, oracle: string, power: number, toughness: number): Partial<ObjectView> => ({
  name,
  identity: { oracle_id: oracle },
  power,
  toughness,
})

const walker = (name: string, oracle: string, loyalty: number): Partial<ObjectView> => ({
  name,
  identity: { oracle_id: oracle },
  supertypes: ['Legendary'],
  types: ['Planeswalker'],
  subtypes: [],
  colors: ['Blue'],
  power: null,
  toughness: null,
  counters: [['loyalty', loyalty]],
})

const aura = (
  name: string,
  host: number | undefined,
  over: Partial<ObjectView> = {},
): Partial<ObjectView> => ({
  name,
  types: ['Enchantment'],
  subtypes: ['Aura'],
  colors: ['White'],
  power: null,
  toughness: null,
  ...(host === undefined ? {} : { attached_to: host }),
  ...over,
})

const soldier = (over: Partial<ObjectView> = {}): Partial<ObjectView> => ({
  name: 'Soldier',
  kind: 'Token',
  colors: ['White'],
  subtypes: ['Soldier'],
  power: 1,
  toughness: 1,
  ...over,
})

/** Observed Stasis, a Partial card: the engine runs neither clause, so the table plays both by hand. */
const STASIS: ManualClause[] = [
  {
    text: 'When this Aura enters, remove enchanted creature from combat, then draw a card for each tapped creature its controller controls.',
    reason: 'the engine does not read this sentence yet',
  },
  {
    text: 'Enchanted creature loses all abilities and can’t attack or block.',
    reason: 'the engine does not read this sentence yet',
  },
]
const COIN_FLIP: ManualClause = {
  text: 'Whenever this creature attacks, flip a coin.',
  reason: 'a random choice the engine does not offer',
}
const LONG_CLAUSE: ManualClause = {
  text: 'At the beginning of your upkeep, each player reveals the top card of their library, then you choose one of the revealed cards that shares a card type with another revealed card and put it into your hand.',
  reason: 'the engine does not read this sentence yet',
}

const tapped = { tapped: true, damage: 0, summoning_sick: false }
const sick = { tapped: false, damage: 0, summoning_sick: true }
const hurt = (damage: number) => ({ tapped: false, damage, summoning_sick: false })

/** The permanents of the viewer's side: every state a card carries, with the ids the attachments need. */
function mine(first: number, seat: number, rival: number): ObjectView[] {
  const id = (n: number): number => first + n
  const list: ObjectView[] = [
    // Counters and changed power and toughness.
    base(id(0), seat, { ...creature('Grizzly Bears', 'st-bears', 5, 5), counters: [['+1/+1', 3]] }),
    base(id(1), seat, { ...creature('Hill Giant', 'st-giant', 2, 2), counters: [['-1/-1', 1]] }),
    base(id(2), seat, {
      ...creature('Silvercoat Lion', 'st-lion', 4, 2),
      attachments: [id(3), id(4)],
      keywords: ['First strike'],
    }),
    base(id(3), seat, aura('Pacifism', id(2))),
    base(id(4), seat, {
      name: 'Bonesplitter',
      types: ['Artifact'],
      subtypes: ['Equipment'],
      colors: [],
      power: null,
      toughness: null,
      attached_to: id(2),
    }),
    // Damage: short of lethal, then lethal.
    base(id(5), seat, {
      ...creature('Serra Angel', 'st-angel', 4, 4),
      keywords: ['Flying', 'Vigilance'],
      permanent: hurt(2),
    }),
    base(id(6), seat, { ...creature('Wall of Omens', 'st-wall', 0, 4), permanent: hurt(4) }),
    // A copy of the Angel and a copy of a token, both marked as copies.
    base(id(7), seat, {
      ...creature('Serra Angel', 'st-angel', 4, 4),
      copy: { of: id(5) },
      keywords: ['Flying', 'Vigilance'],
    }),
    // Another face: transformed, and a modal double-faced card cast as its back.
    base(id(8), seat, {
      name: 'Insectile Aberration',
      identity: { oracle_id: 'st-delver' },
      face: 1,
      colors: ['Blue'],
      power: 3,
      toughness: 2,
      keywords: ['Flying'],
      permanent: sick,
    }),
    base(id(9), seat, {
      name: 'Agadeem, the Undercrypt',
      face: 1,
      supertypes: [],
      types: ['Land'],
      colors: [],
      power: null,
      toughness: null,
    }),
    // A merged creature, a prepared permanent and a Class at level 2.
    base(id(10), seat, {
      name: 'Gemrazer',
      colors: ['Green'],
      power: 6,
      toughness: 5,
      merged: [
        { id: id(10), name: 'Gemrazer' },
        { id: id(11), name: 'Hill Giant' },
      ],
    }),
    base(id(12), seat, {
      name: 'Hostile Hostel',
      types: ['Artifact'],
      colors: [],
      power: null,
      toughness: null,
      permanent: { tapped: false, damage: 0, summoning_sick: false, prepared: true },
    }),
    base(id(13), seat, {
      name: 'Bard Class',
      types: ['Enchantment'],
      subtypes: ['Class'],
      colors: ['Red'],
      power: null,
      toughness: null,
      permanent: { tapped: false, damage: 0, summoning_sick: false, level: 2 },
    }),
    // Planeswalkers: loyalty raised, lowered and as printed.
    base(id(14), seat, walker('Jace Beleren', 'st-jace', 5)),
    base(id(15), seat, walker('Liliana of the Veil', 'st-liliana', 1)),
    base(id(16), seat, walker('Teferi, Hero of Dominaria', 'st-teferi', 4)),
    // Three equal tokens share a box; one with a counter and one tapped stay apart; named counters.
    base(id(17), seat, soldier()),
    base(id(18), seat, soldier()),
    base(id(19), seat, soldier()),
    base(id(20), seat, soldier({ counters: [['+1/+1', 1]], power: 2, toughness: 2 })),
    base(id(21), seat, soldier({ permanent: tapped })),
    base(id(22), seat, {
      name: 'Sundial of the Infinite',
      types: ['Artifact'],
      colors: [],
      power: null,
      toughness: null,
      counters: [
        ['charge', 2],
        [{ Other: 'time' }, 1],
        [{ Other: 'shield' }, 3],
      ],
    }),
    // A card the viewer may look at that lies face down, and a Curse on a rival.
    base(id(23), seat, {
      ...creature('Grizzly Bears', 'st-bears', 2, 2),
      face_down: true,
    }),
    base(id(24), seat, aura('Curse of Chains', undefined, { attached_player: rival, colors: ['Blue'] })),
    // An Aura of the viewer's on a rival's creature: it stays in the viewer's row.
    base(id(25), seat, aura('Arrest', first + 100)),
    // An Aura on a land follows the land.
    base(id(26), seat, {
      name: 'Forest',
      supertypes: ['Basic'],
      types: ['Land'],
      subtypes: ['Forest'],
      colors: [],
      power: null,
      toughness: null,
      attachments: [id(27)],
      abilities: [{ Mana: { produces: 'Green' } }],
    }),
    base(id(27), seat, aura('Wild Growth', id(26), { colors: ['Green'] })),
    // Clauses left to the table: an Aura whose two clauses are played by hand (a Partial card), the creature it
    // enchants, a creature with one clause and a long one.
    base(id(28), seat, { ...creature('Pegasus Courser', 'st-pegasus', 1, 3), attachments: [id(29)] }),
    base(id(29), seat, aura('Observed Stasis', id(28), { manual: STASIS })),
    base(id(30), seat, { ...creature('Wild Mage Apprentice', 'st-mage', 2, 2), manual: [COIN_FLIP] }),
    base(id(31), seat, {
      ...creature('Verbose Oracle', 'st-oracle', 1, 1),
      permanent: tapped,
      manual: [COIN_FLIP, LONG_CLAUSE, ...STASIS],
    }),
  ]
  return list
}

/** A rival's permanents: the creature the viewer's Aura sits on, a face-down card the viewer cannot see, and more. */
function theirs(first: number, seat: number, index: number, arrest: number): ObjectView[] {
  const list: ObjectView[] = []
  if (index === 0) {
    list.push(base(first, seat, { ...creature('Hill Giant', 'st-giant', 3, 3), attachments: [arrest] }))
  }
  if (index === 1) {
    list.push(
      base(first, seat, {
        name: 'Hill Giant',
        identity: { oracle_id: 'st-giant' },
        power: 3,
        toughness: 3,
        attachments: [first + 1],
      }),
      base(first + 1, seat, aura('Pacifism', first, { colors: ['White'] })),
      base(first + 2, seat, { ...soldier(), copy: { of: first } }),
    )
  }
  if (index === 2) {
    list.push(
      base(first, seat, {
        name: '',
        types: [],
        subtypes: [],
        colors: [],
        power: null,
        toughness: null,
        face_down: true,
      }),
      base(first + 1, seat, {
        ...soldier({
          name: 'Treasure',
          colors: [],
          types: ['Artifact'],
          subtypes: ['Treasure'],
          power: null,
          toughness: null,
        }),
      }),
      base(first + 2, seat, {
        ...soldier({
          name: 'Treasure',
          colors: [],
          types: ['Artifact'],
          subtypes: ['Treasure'],
          power: null,
          toughness: null,
        }),
      }),
      base(first + 3, seat, {
        ...soldier({
          name: 'Treasure',
          colors: [],
          types: ['Artifact'],
          subtypes: ['Treasure'],
          power: null,
          toughness: null,
        }),
      }),
    )
  }
  return list
}

/** Adds the state permanents to the fixture's battlefield and a foretold card to the exile. */
export function withStates(fixture: Fixture): Fixture {
  const viewer = fixture.view.viewer
  const seats = fixture.view.players.map((player) => player.seat)
  const rivals = seats.filter((seat) => seat !== viewer)
  const top = Math.max(0, ...fixture.view.objects.map((object) => object.id))
  // The viewer's block takes 200 ids, so the rival's host sits at a known id (`first + 100`) for the cross-seat Aura.
  const first = top + 1
  const viewerSet = mine(first, viewer, rivals[0] ?? viewer)
  const rivalSets = rivals
    .slice(0, 3)
    .flatMap((seat, index) => theirs(first + 100 * (index + 1), seat, index, first + 25))
  const foretold = base(first + 500, viewer, {
    ...creature('Grizzly Bears', 'st-bears', 2, 2),
    zone: 'Exile',
    permanent: null,
    face_down: true,
  })
  const added = [...viewerSet, ...rivalSets]
  const battlefield = [...(fixture.view.battlefield.cards ?? []), ...added.map((object) => object.id)]
  const exile = [...(fixture.view.exile.cards ?? []), foretold.id]
  return {
    ...fixture,
    view: {
      ...fixture.view,
      objects: [...fixture.view.objects, ...added, foretold].sort((a, b) => a.id - b.id),
      battlefield: { count: battlefield.length, cards: battlefield },
      exile: { count: exile.length, cards: exile },
    },
  }
}
