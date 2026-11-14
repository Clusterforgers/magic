// Goal: a pod shows its seat's commanders as art crops (two for partners) only when they have a picture, and a stack chip shows a spell's picture but none for an ability.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { portraitIdentities } from './CommanderPortrait'

const card = (oracle: string): Pick<ObjectView, 'identity'> => ({ identity: { oracle_id: oracle } })

describe('portraitIdentities', () => {
  it.each([
    ['no commander', [], {}, []],
    ['one commander', [1], { 1: card('a') }, ['a']],
    ['partners', [1, 2], { 1: card('a'), 2: card('b') }, ['a', 'b']],
    ['a commander without a picture', [1, 2], { 1: {}, 2: card('b') }, ['b']],
    ['three cards in the command zone', [1, 2, 3], { 1: card('a'), 2: card('b'), 3: card('c') }, ['a', 'b']],
    ['an id the view does not hold', [9], {}, []],
  ] as const)('gives %s', (_, ids, objects, oracles) => {
    expect(portraitIdentities(ids, objects).map((identity) => identity.oracle_id)).toEqual(oracles)
  })
})

describe('the pictures at the table', () => {
  it('draws the commander of a one-line pod in its seat, as an art crop hidden from a screen reader', () => {
    const { container } = renderTable({
      seats: 4,
      state: (state) => {
        const view = clone(must(state.view))
        for (const object of view.objects) object.identity = { oracle_id: `o${object.id}` }
        const objectsById = Object.fromEntries(view.objects.map((object) => [object.id, object]))
        return { ...state, view, objectsById }
      },
    })
    const portraits = [
      ...container.querySelectorAll('[data-variant="line"] [data-slot="commander-portrait"]'),
    ]
    expect(portraits.length).toBeGreaterThan(0)
    for (const portrait of portraits) {
      expect(portrait).toHaveAttribute('aria-hidden', 'true')
      expect(must(portrait.querySelector('img')).getAttribute('src')).toMatch(/\/art_crop$/)
    }
  })

  it('puts a picture on the chip of a spell on the stack and none on an ability', () => {
    const { container } = renderTable({ seats: 2 })
    const chips = [...container.querySelectorAll<HTMLElement>('[data-stack-chip]')]
    expect(chips.length).toBeGreaterThan(0)
    for (const chip of chips) {
      const ability = chip.getAttribute('aria-label')?.includes('Ability of') === true
      expect(chip.querySelector('[data-slot="chip-art"]') === null).toBe(ability)
    }
  })
})
