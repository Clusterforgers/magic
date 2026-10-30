// Goal: a player's chips list the Auras and Curses attached to that player and nothing else: one chip per name and controller in id order, a count when several share one, the controller named when it is not the player, what the Aura gives the player in words and marks, and none for an Aura on a permanent, a Curse that left the battlefield or another player's.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { enchantChips } from './enchantChips'

const NAMES = ['Ana', 'Bo', 'Cy']
const nameOf = (seat: number): string => NAMES[seat] ?? `Seat ${seat + 1}`

const curse = (id: number, over: Partial<ObjectView> = {}): ObjectView =>
  card({ id, name: 'Curse of Opulence', controller: 0, attached_player: 1, ...over })

const chipsFor = (seat: number, ...objects: ObjectView[]) =>
  enchantChips(seat, Object.fromEntries(objects.map((object) => [object.id, object])), nameOf)

describe('the chips of an enchanted player', () => {
  it('gives one chip for a Curse, named with its controller when that is another seat', () => {
    expect(chipsFor(1, curse(7))).toEqual([
      {
        key: 7,
        count: 1,
        text: 'Curse of Opulence',
        words: "Curse of Opulence (Ana's) enchants Bo",
        gives: [],
      },
    ])
  })

  it('names no controller when the player controls the Aura on themself', () => {
    expect(chipsFor(1, curse(7, { controller: 1, name: 'Faith Unbroken' }))[0]?.words).toBe(
      'Faith Unbroken enchants Bo',
    )
  })

  it('counts Auras of one name and one controller in a single chip, led by the first id', () => {
    const [chip, ...rest] = chipsFor(1, curse(9), curse(7), curse(8))
    expect(rest).toEqual([])
    expect(chip).toEqual({
      key: 7,
      count: 3,
      text: 'Curse of Opulence ×3',
      words: "3 × Curse of Opulence (Ana's) enchants Bo",
      gives: [],
    })
  })

  it('keeps Auras of another name or another controller apart, in id order', () => {
    const chips = chipsFor(
      1,
      curse(9, { controller: 2 }),
      curse(8, { name: 'Curse of the Pierced Heart' }),
      curse(7),
    )
    expect(chips.map((chip) => [chip.key, chip.text])).toEqual([
      [7, 'Curse of Opulence'],
      [8, 'Curse of the Pierced Heart'],
      [9, 'Curse of Opulence'],
    ])
  })

  it('shortens a name at its comma so the chip fits', () => {
    expect(chipsFor(1, curse(7, { name: 'Curse of Death, Revisited' }))[0]?.text).toBe('Curse of Death')
  })

  it.each([
    ['an Aura on a permanent', { attached_player: undefined, attached_to: 3 }],
    ['an Aura in a graveyard', { zone: { Graveyard: 0 } as const }],
    ['an Aura on another player', { attached_player: 2 }],
  ])('gives nothing for %s', (_name, over) => {
    expect(chipsFor(1, curse(7, over))).toEqual([])
  })

  it('gives nothing for a player nothing is attached to', () => {
    expect(chipsFor(0, curse(7))).toEqual([])
  })

  it.each([
    [{ keywords: ['Hexproof'] }, ['Hexproof'], ': hexproof'],
    [
      { keywords: ['Shroud', 'Protection from creatures'] },
      ['Shroud', 'Pro creatures'],
      ': shroud, protection from creatures',
    ],
    [
      { keywords: ['Protection from red and from blue'] },
      ['Pro red, blue'],
      ': protection from red and from blue',
    ],
    [{ rules: ['LoseKeyword' as const] }, ['Loses a keyword'], ': loses a keyword'],
    [undefined, [], ''],
  ])('says what the Aura gives the player: %j', (gives, marks, words) => {
    const [chip] = chipsFor(1, curse(7, gives === undefined ? {} : { gives }))
    expect(chip?.gives.map((item) => item.text)).toEqual(marks)
    expect(chip?.words).toBe(`Curse of Opulence (Ana's) enchants Bo${words}`)
  })
})
