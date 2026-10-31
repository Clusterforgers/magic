// Goal: every card, printing, token and face maps to the server's picture endpoint at the size its drawn width needs, and an object the viewer may not see has no picture source.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import {
  cardImageFallbackUrl,
  cardImageUrl,
  faceOf,
  imageSizeFor,
  imageSourceOf,
  missKeyOf,
  showsBack,
} from './images'
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

describe('missKeyOf', () => {
  it('is the token URL without power and toughness, so a buffed token shares the miss of its printed self', () => {
    const buffed: ImageSource = { ...soldier, power: 3, toughness: 3 }
    expect(missKeyOf(soldier, 0, 'normal')).toBe(missKeyOf(buffed, 0, 'normal'))
    expect(missKeyOf(soldier, 0, 'normal')).toBe(
      '/api/images/token/0/normal?name=Soldier&type=Creature+%E2%80%94+Soldier&colors=W',
    )
  })

  it.each<[string, ImageSource, number, ImageSize]>([
    ['another face', { ...soldier }, 1, 'normal'],
    ['another size', { ...soldier }, 0, 'small'],
    ['another colour', { ...soldier, colors: ['R'] }, 0, 'normal'],
    ['another name', { ...soldier, name: 'Knight' }, 0, 'normal'],
  ])('differs for %s', (_, other, face, size) => {
    expect(missKeyOf(other, face, size)).not.toBe(missKeyOf(soldier, 0, 'normal'))
  })

  it('is null for a printed card, whose miss is its own URL', () => {
    expect(missKeyOf({ kind: 'card', identity: { oracle_id: ORACLE } }, 0, 'normal')).toBeNull()
  })
})

describe('cardImageFallbackUrl', () => {
  it.each<[string, ImageSource, string | null]>([
    [
      'a printing falls back to its card',
      { kind: 'card', identity: { oracle_id: 'o-1', printing_id: 'p-1' } },
      '/api/images/oracle/o-1/1/normal',
    ],
    ['a card by oracle id has none', { kind: 'card', identity: { oracle_id: 'o-1' } }, null],
    ['a token has none', { kind: 'token', name: 'Soldier', type: 'Creature — Soldier', colors: ['W'] }, null],
  ])('%s', (_case, source, url) => {
    expect(cardImageFallbackUrl(source, 1, 'normal')).toBe(url)
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

/** The state of a Room permanent. */
const ROOM = { tapped: false, damage: 0, summoning_sick: false, doors: [false, false] as [boolean, boolean] }

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
    // A Room with both doors locked has no name, and is no hidden card.
    ['a Room with both doors locked', { name: '', permanent: ROOM }, null, false],
  ])('%s', (_, over, source, back) => {
    expect(imageSourceOf(object(over))).toEqual(source)
    expect(showsBack(object(over))).toBe(back)
  })

  it.each<[string, Partial<ObjectView>, number]>([
    ['the front face', {}, 0],
    ['the back face', { face: 1 }, 1],
    // The halves of a Room are doors of one picture.
    ['the second door of a Room, which is no back face', { face: 1, permanent: ROOM }, 0],
  ])('shows the face in use: %s', (_, over, face) => {
    expect(faceOf(object(over))).toBe(face)
  })
})
