// Goal: quick cast reaches exactly the cards that may be cast whenever a player has priority (instants and flash cards), and the card says whether it is queued; the pace settings name the saved default stops.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { defaultStopsWords } from '../stops/stopModel'
import { castsAnyTime, quickCastNote } from './quickCast'

const card = (types: string[], keywords?: string[]): ObjectView =>
  ({ id: 1, name: 'Card', types, ...(keywords === undefined ? {} : { keywords }) }) as unknown as ObjectView

describe('castsAnyTime', () => {
  it.each([
    [['Instant'], undefined, true],
    [['Creature'], ['Flash'], true],
    [['Enchantment'], ['Flash', 'Flying'], true],
    [['Creature'], ['Flying'], false],
    [['Sorcery'], undefined, false],
    [['Land'], undefined, false],
    [['Artifact', 'Creature'], [], false],
  ])('%j with %j is %s', (types, keywords, expected) => {
    expect(castsAnyTime(card(types, keywords))).toBe(expected)
  })
})

describe('words', () => {
  it.each([true, false])('a queued card (%s) says how to cancel or to queue', (queued) => {
    const note = quickCastNote(queued)
    expect(note.words).toMatch(queued ? /cancel/ : /click to cast/)
  })

  it.each([
    [0, 'No default stops'],
    [1, 'Default: 1 pinned stop'],
    [4, 'Default: 4 pinned stops'],
  ])('%i default stops read %s', (count, words) => {
    expect(defaultStopsWords(count)).toBe(words)
  })
})
