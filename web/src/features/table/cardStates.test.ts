// Goal: every state of a permanent the owner needs at the table — damage and when it is lethal, face down, back face, copy, merged, prepared, level — becomes the right chip, the right spoken words and the right marks, whatever else the object carries.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { card, permanent } from '@/test-utils/cards'
import { cardLabel, isLethal, showsBackFace, statusChips, statusWords } from './cardModel'

const chips = (object: ObjectView) => statusChips(object, null).map(({ label, tone }) => `${label}:${tone}`)

describe('isLethal', () => {
  it.each<[string, number, number | null, boolean]>([
    ['no damage', 0, 2, false],
    ['one short of the toughness', 1, 2, false],
    ['as much as the toughness', 2, 2, true],
    ['more than the toughness', 5, 2, true],
    ['a toughness of zero is dead anyway, not lethal damage', 3, 0, false],
    ['no toughness (a planeswalker)', 3, null, false],
  ])('%s', (_, damage, toughness, lethal) => {
    expect(isLethal(permanent({ damage }, { toughness }))).toBe(lethal)
  })

  it('is false off the battlefield', () => {
    expect(isLethal(card({ permanent: null }))).toBe(false)
  })
})

describe('showsBackFace', () => {
  it.each<[string, Partial<ObjectView>, boolean]>([
    ['the front face', {}, false],
    ['an explicit front face', { face: 0 }, false],
    ['a transformed permanent', { face: 1 }, true],
    ['a back face on the stack (an Adventure or a modal back)', { face: 1, zone: 'Stack' }, false],
  ])('%s', (_, over, back) => {
    expect(showsBackFace(card(over))).toBe(back)
  })
})

describe('statusChips', () => {
  it.each<[string, ObjectView, string[]]>([
    ['a plain permanent', permanent({}), []],
    ['damage short of lethal', permanent({ damage: 1 }), ['1 dmg:plain']],
    ['lethal damage', permanent({ damage: 2 }), ['2 dmg:danger']],
    ['a card lying face down', permanent({}, { face_down: true }), ['Face down:plain']],
    ['a transformed permanent', permanent({}, { face: 1 }), ['Back face:plain']],
    ['a copy', permanent({}, { copy: {} }), ['Copy:plain']],
    ['a copy of a visible original', permanent({}, { copy: { of: 7 } }), ['Copy:plain']],
    [
      'a merged permanent',
      permanent(
        {},
        {
          merged: [
            { id: 1, name: 'Gemrazer' },
            { id: 2, name: 'Grizzly Bears' },
            { id: 3, name: 'Hill Giant' },
          ],
        },
      ),
      ['Merged ×3:plain'],
    ],
    ['a merged list of one is no merge', permanent({}, { merged: [{ id: 1, name: 'Hill Giant' }] }), []],
    ['a prepared permanent', permanent({ prepared: true }), ['Prepared:plain']],
    ['a level 2 Class', permanent({ level: 2 }), ['Level 2:plain']],
    ['level 0 is still a level', permanent({ level: 0 }), ['Level 0:plain']],
    [
      'everything at once, in a fixed order',
      permanent(
        { tapped: true, summoning_sick: true, damage: 3, prepared: true, level: 1 },
        { face_down: true, face: 2, copy: {} },
      ),
      [
        'Tapped:plain',
        'Sick:plain',
        '3 dmg:danger',
        'Face down:plain',
        'Back face:plain',
        'Copy:plain',
        'Prepared:plain',
        'Level 1:plain',
      ],
    ],
  ])('%s', (_, object, expected) => {
    expect(chips(object)).toEqual(expected)
  })
})

describe('statusWords of the identity states', () => {
  it.each<[string, ObjectView, Parameters<typeof statusWords>[2], string[]]>([
    ['face down', permanent({}, { face_down: true }), {}, ['face down']],
    ['a back face of an unknown front', permanent({}, { face: 1 }), {}, ['back face']],
    [
      'a back face of a known front',
      permanent({}, { face: 1 }),
      { frontFace: 'Delver of Secrets' },
      ['back face of Delver of Secrets'],
    ],
    ['a copy of nothing visible', permanent({}, { copy: {} }), {}, ['copy']],
    [
      'a copy of Grizzly Bears',
      permanent({}, { copy: { of: 3 } }),
      { copyOf: 'Grizzly Bears' },
      ['copy of Grizzly Bears'],
    ],
    [
      'a merge',
      permanent(
        {},
        {
          merged: [
            { id: 1, name: 'Gemrazer' },
            { id: 2, name: 'Hill Giant' },
          ],
        },
      ),
      {},
      ['merged from Gemrazer, Hill Giant'],
    ],
    ['prepared and a level', permanent({ prepared: true, level: 3 }), {}, ['prepared', 'level 3']],
    ['lethal damage', permanent({ damage: 2 }), {}, ['2 damage, enough to destroy it']],
  ])('%s', (_, object, links, words) => {
    expect(statusWords(object, null, links)).toEqual(words)
  })

  it('reaches the spoken name of a card, with what it copies', () => {
    const label = cardLabel(permanent({}, { copy: { of: 3 }, name: 'Clone' }), null, {
      copyOf: 'Grizzly Bears',
    })
    expect(label).toContain('copy of Grizzly Bears')
  })
})
