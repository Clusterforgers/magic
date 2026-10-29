// Goal: every card, printing, token and face maps to the server's picture endpoint at the size its drawn width needs, and an object the viewer may not see has no picture source.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { cardImageUrl, faceOf, imageSizeFor, imageSourceOf, showsBack } from './images'
import type { ImageSize, ImageSource } from './images'

const ORACLE = '4b3f5c1e-0000-4000-8000-000000000001'
const PRINTING = 'a1b2c3d4-0000-4000-8000-000000000002'
const soldier: ImageSource = {
  kind: 'token',
  name: 'Soldier',
  type: 'Creature — Soldier',
  colors: ['W'],
  power: 1,
  toughness: 1,
}

describe('cardImageUrl', () => {
  it.each<[string, ImageSource, number, ImageSize, string]>([
    [
      'oracle front',
      { kind: 'card', identity: { oracle_id: ORACLE } },
      0,
      'small',
      `/api/images/oracle/${ORACLE}/0/small`,
    ],
    [
      'oracle back',
      { kind: 'card', identity: { oracle_id: ORACLE } },
      1,
      'large',
      `/api/images/oracle/${ORACLE}/1/large`,
    ],
    [
      'printing wins',
      { kind: 'card', identity: { oracle_id: ORACLE, printing_id: PRINTING } },
      0,
      'normal',
      `/api/images/printing/${PRINTING}/0/normal`,
    ],
    [
      'bad face floors to 0',
      { kind: 'card', identity: { oracle_id: ORACLE } },
      -2,
      'png',
      `/api/images/oracle/${ORACLE}/0/png`,
    ],
    [
      'token',
      soldier,
      0,
      'normal',
      '/api/images/token/0/normal?name=Soldier&type=Creature+%E2%80%94+Soldier&colors=W&power=1&toughness=1',
    ],
    [
      'colorless token without p/t',
      {
        kind: 'token',
        name: 'Treasure',
        type: 'Artifact — Treasure',
        colors: [],
        power: null,
        toughness: null,
      },
      0,
      'small',
      '/api/images/token/0/small?name=Treasure&type=Artifact+%E2%80%94+Treasure&colors=C',
    ],
    [
      'escapes ids',
      { kind: 'card', identity: { oracle_id: 'a/b' } },
      0,
      'small',
      '/api/images/oracle/a%2Fb/0/small',
    ],
  ])('%s', (_, source, face, size, url) => {
    expect(cardImageUrl(source, face, size)).toBe(url)
  })
})

describe('imageSizeFor', () => {
  it.each([
    [40, 'small'],
    [96, 'small'],
    [109, 'small'],
    [110, 'normal'],
    [240, 'normal'],
    [329, 'normal'],
    [330, 'large'],
    [600, 'large'],
  ])('%i px draws %s', (width, size) => {
    expect(imageSizeFor(width)).toBe(size)
  })
})

const object = (over: Partial<ObjectView>): ObjectView => ({
  id: 1,
  owner: 1,
  controller: 1,
  zone: 'Battlefield',
  name: 'Grizzly Bears',
  mana_cost: null,
  supertypes: [],
  types: ['Creature'],
  subtypes: ['Bear'],
  colors: ['Green'],
  power: 2,
  toughness: 2,
  abilities: [],
  permanent: null,
  ...over,
})

describe('imageSourceOf', () => {
  it.each<[string, Partial<ObjectView>, ImageSource | null, boolean]>([
    [
      'identified card',
      { identity: { oracle_id: ORACLE } },
      { kind: 'card', identity: { oracle_id: ORACLE } },
      false,
    ],
    [
      'token by its facts',
      { kind: 'Token', name: 'Soldier', subtypes: ['Soldier'], colors: ['White'], power: 1, toughness: 1 },
      soldier,
      false,
    ],
    [
      'face-down to another seat',
      { face_down: true, name: '', types: [], subtypes: [], colors: [] },
      null,
      true,
    ],
    [
      'face-down to its controller',
      { face_down: true, identity: { oracle_id: ORACLE } },
      { kind: 'card', identity: { oracle_id: ORACLE } },
      false,
    ],
    ['hidden card', { name: '' }, null, true],
    ['ability on the stack', { kind: 'Ability' }, null, false],
    ['card with no record', {}, null, false],
  ])('%s', (_, over, source, back) => {
    expect(imageSourceOf(object(over))).toEqual(source)
    expect(showsBack(object(over))).toBe(back)
  })

  it('shows the face in use', () => {
    expect(faceOf(object({}))).toBe(0)
    expect(faceOf(object({ face: 1 }))).toBe(1)
  })
})
