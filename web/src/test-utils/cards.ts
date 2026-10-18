import type { ObjectView, PermanentView } from '@/protocol/generated'

/** A 2/2 green creature on the battlefield, untapped; `over` changes any field. */
export function card(over: Partial<ObjectView> = {}): ObjectView {
  return {
    id: 1,
    owner: 0,
    controller: 0,
    zone: 'Battlefield',
    name: 'Grizzly Bears',
    mana_cost: { generic: 1, colorless: 0, colored: [0, 0, 0, 0, 1] },
    supertypes: [],
    types: ['Creature'],
    subtypes: ['Bear'],
    colors: ['Green'],
    power: 2,
    toughness: 2,
    abilities: [],
    permanent: { tapped: false, damage: 0, summoning_sick: false },
    ...over,
  }
}

/** A permanent whose state is `state` on top of the untapped, undamaged default. */
export function permanent(state: Partial<PermanentView>, over: Partial<ObjectView> = {}): ObjectView {
  return card({ permanent: { tapped: false, damage: 0, summoning_sick: false, ...state }, ...over })
}

/** A basic Forest that taps for green. */
export function forest(over: Partial<ObjectView> = {}): ObjectView {
  return card({
    name: 'Forest',
    mana_cost: null,
    supertypes: ['Basic'],
    types: ['Land'],
    subtypes: ['Forest'],
    colors: [],
    power: null,
    toughness: null,
    abilities: [{ Mana: { produces: 'Green' } }],
    ...over,
  })
}
