// Goal: a table card shows each state the owner needs — lethal damage, face down, back face, copy, merge, prepared, level, what it is attached to (a permanent or a player) and the colour of changed power and toughness — as chips and marks over the picture and on the text face, and says the same aloud.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice, setImageChoice } from '@/features/cards/imageSetting'
import { indexById } from '@/play/view'
import type { CardDetail, ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { card, permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import { setDetailFetcher } from './printed'

const ID = 900
const IDENTITY = { oracle_id: 'delver-oracle' }

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

/** Draws card 900 beside `others` on the battlefield of a two-seat fixture. */
function renderCard(object: ObjectView, others: ObjectView[] = []) {
  return renderTable({
    ui: <Card id={ID} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...object, id: ID }, ...others]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

/** Draws the card and lets its picture load, so the chips sit over the picture. */
async function loaded(object: ObjectView, others: ObjectView[] = []) {
  const view = renderCard(object, others)
  act(() => fireEvent.load(must(view.container.querySelector('img'))))
  await act(async () => {
    await Promise.resolve()
  })
  return view
}

const chipTexts = (container: HTMLElement): string[] =>
  [...container.querySelectorAll('[data-slot="card-status"]')].map((chip) => chip.textContent ?? '')

const faces = (names: string[]): CardDetail =>
  ({
    faces: names.map((name) => ({ name, power: '1', toughness: '1', loyalty: null })),
  }) as unknown as CardDetail

describe('the state chips over a picture', () => {
  it.each<[string, ObjectView, string[]]>([
    ['nothing to say', permanent({}, { identity: IDENTITY }), []],
    ['damage short of lethal', permanent({ damage: 1 }, { identity: IDENTITY }), ['1 dmg']],
    ['lethal damage', permanent({ damage: 2 }, { identity: IDENTITY }), ['2 dmg']],
    ['a back face', permanent({}, { identity: IDENTITY, face: 1 }), ['Back face']],
    ['a copy', permanent({}, { identity: IDENTITY, copy: {} }), ['Copy']],
    [
      'a face-down card the viewer may look at',
      permanent({}, { identity: IDENTITY, face_down: true }),
      ['Face down'],
    ],
    [
      'a merge',
      permanent(
        {},
        {
          identity: IDENTITY,
          merged: [
            { id: 1, name: 'A' },
            { id: 2, name: 'B' },
          ],
        },
      ),
      ['Merged ×2'],
    ],
    ['a prepared permanent', permanent({ prepared: true }, { identity: IDENTITY }), ['Prepared']],
    ['a Class level', permanent({ level: 3 }, { identity: IDENTITY }), ['Level 3']],
  ])('%s', async (_, object, expected) => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = await loaded(object)
    expect(chipTexts(container)).toEqual(expected)
  })

  it('draws lethal damage as a warning and other damage plainly', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const lethal = await loaded(permanent({ damage: 2 }, { identity: IDENTITY }))
    expect(lethal.container.querySelector('[data-slot="card-status"]')).toHaveAttribute('data-tone', 'danger')
    lethal.unmount()
    const short = await loaded(permanent({ damage: 1 }, { identity: IDENTITY }))
    expect(short.container.querySelector('[data-slot="card-status"]')).toHaveAttribute('data-tone', 'plain')
  })

  it('names a face-down card that is hidden from the viewer on its back', () => {
    const { container } = renderCard(
      permanent({}, { face_down: true, name: '', types: [], subtypes: [], colors: [] }),
    )
    expect(screen.getByRole('img', { name: 'Face-down card' })).toBeInTheDocument()
    expect(chipTexts(container)).toEqual(['Face down'])
  })

  it('names no state on the back of a card that is only hidden', () => {
    const { container } = renderCard(permanent({}, { name: '', types: [], subtypes: [], colors: [] }))
    expect(chipTexts(container)).toEqual([])
  })
})

describe('what a card says aloud about its other faces and originals', () => {
  it('names the original of a copy', () => {
    renderCard(permanent({}, { copy: { of: 5 }, name: 'Clone' }), [card({ id: 5, name: 'Hill Giant' })])
    expect(screen.getByRole('button')).toHaveAccessibleName(/copy of Hill Giant/)
  })

  it('says only "copy" when the original is out of sight', () => {
    renderCard(permanent({}, { copy: {}, name: 'Clone' }))
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(/, copy(,|$)/)
  })

  it('names the front of a card that shows its back, once the card is known', async () => {
    setDetailFetcher(() => Promise.resolve(faces(['Delver of Secrets', 'Insectile Aberration'])))
    renderCard(permanent({}, { identity: IDENTITY, face: 1, name: 'Insectile Aberration' }))
    await act(async () => {
      await Promise.resolve()
    })
    expect(screen.getByRole('button')).toHaveAccessibleName(/back face of Delver of Secrets/)
  })
})

describe('what is attached', () => {
  it('marks an attachment, names its host on the picture and says what it is attached to', async () => {
    const { container } = await loaded(
      permanent({}, { identity: IDENTITY, attached_to: 5, name: 'Pacifism', types: ['Enchantment'] }),
      [card({ id: 5, name: 'Hill Giant', attachments: [ID] })],
    )
    expect(screen.getByRole('button')).toHaveAttribute('data-attached', 'true')
    const mark = must(container.querySelector('[data-slot="card-attached-to"]'))
    expect(mark).toHaveTextContent('Hill Giant')
    expect(mark.closest('[data-slot="card-marks-foot"]')).not.toBeNull()
    expect(screen.getByRole('button')).toHaveAccessibleName(/attached to Hill Giant/)
  })

  it('lists on the host picture what is attached to it', async () => {
    const { container } = await loaded(permanent({}, { identity: IDENTITY, attachments: [5, 6] }), [
      card({ id: 5, name: 'Pacifism', attached_to: ID }),
      card({ id: 6, name: 'Boots', attached_to: ID }),
    ])
    // The foot stack grows upward: the first attachment, the deepest tucked, is the highest mark.
    const marks = [...container.querySelectorAll('[data-slot="card-grant"]')]
    expect(marks.map((mark) => mark.textContent)).toEqual(['Boots', 'Pacifism'])
  })

  it('names the player a Curse enchants', () => {
    const { container } = renderCard(
      permanent({}, { attached_player: 1, name: 'Curse of Chains', types: ['Enchantment'] }),
    )
    expect(screen.getByRole('button')).toHaveAccessibleName(/attached to Bo/)
    expect(container.querySelector('[data-slot="card-attached-to"]')).toHaveTextContent('On Bo')
  })

  it('tells its host how many are attached and what they are', () => {
    renderCard(permanent({}, { attachments: [5, 6], name: 'Hill Giant' }), [
      card({ id: 5, name: 'Pacifism', attached_to: ID }),
      card({ id: 6, name: 'Boots', attached_to: ID }),
    ])
    const host = screen.getByRole('button')
    expect(host).toHaveAttribute('data-hosting', '2')
    expect(host).not.toHaveAttribute('data-attached')
    expect(host).toHaveAccessibleName(/Pacifism attached, Boots attached/)
  })
})

describe('the colour of changed power and toughness on the text face', () => {
  const printed = (power: string, toughness: string): CardDetail =>
    ({ faces: [{ name: 'Hill Giant', power, toughness, loyalty: null }] }) as unknown as CardDetail

  it.each<[string, number, number, string | null]>([
    ['as printed', 2, 2, null],
    ['raised', 4, 4, 'up'],
    ['lowered', 1, 1, 'down'],
    ['raised and lowered', 3, 1, 'changed'],
  ])('%s', async (_, power, toughness, modified) => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.resolve(printed('2', '2')))
    const { container } = renderCard(permanent({}, { identity: IDENTITY, power, toughness }))
    await act(async () => {
      await Promise.resolve()
    })
    const stat = must(container.querySelector('[data-slot="card-face"] > span:last-child'))
    expect(stat).toHaveTextContent(`${power}/${toughness}`)
    expect(stat.getAttribute('data-modified')).toBe(modified)
  })
})
