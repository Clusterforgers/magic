// Goal: a table card draws its picture with the state over it — a stat badge only where power/toughness or loyalty moved from the printed value, chips in the art box, a support mark for manual clauses, the back for a card the viewer may not see — and Text only draws exactly the text face.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { IMAGE_CHOICE_KEY, resetImageChoice, setImageChoice } from '@/features/cards/imageSetting'
import type { CardDetail, ObjectView } from '@/protocol/generated'
import { card, permanent } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { indexById } from '@/play/view'
import { Card } from './Card'
import { statBadge } from './overlay'
import type { Printed, StatBadge } from './overlay'
import { setDetailFetcher } from './printed'
import { TableMenu } from './TableMenu'

const ID = 900
const IDENTITY = { oracle_id: 'bears-oracle' }

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

const stats = (power: string | null, toughness: string | null, loyalty: string | null = null): Printed => ({
  power,
  toughness,
  loyalty,
})

describe('statBadge', () => {
  const creature = (power: number, toughness: number): ObjectView => card({ power, toughness })
  const walker = card({ power: null, toughness: null, types: ['Planeswalker'] })
  it.each<[string, ObjectView, Printed, number | null, StatBadge | null]>([
    ['as printed', creature(2, 2), stats('2', '2'), null, null],
    ['pumped', creature(4, 4), stats('2', '2'), null, { kind: 'pt', text: '4/4', modified: 'up' }],
    ['shrunk', creature(1, 2), stats('2', '2'), null, { kind: 'pt', text: '1/2', modified: 'down' }],
    ['swapped', creature(3, 1), stats('1', '3'), null, { kind: 'pt', text: '3/1', modified: 'changed' }],
    ['star printed', creature(5, 5), stats('*', '*'), null, { kind: 'pt', text: '5/5', modified: null }],
    ['unknown printed', creature(2, 2), 'unavailable', null, { kind: 'pt', text: '2/2', modified: null }],
    ['still asking', creature(4, 4), 'loading', null, null],
    ['token', creature(4, 4), 'none', null, null],
    ['loyalty as printed', walker, stats(null, null, '3'), 3, null],
    ['loyalty up', walker, stats(null, null, '3'), 5, { kind: 'loyalty', text: '5', modified: 'up' }],
    ['loyalty down', walker, stats(null, null, '3'), 1, { kind: 'loyalty', text: '1', modified: 'down' }],
    ['no stats', card({ power: null, toughness: null }), stats(null, null), null, null],
  ])('%s', (_, object, printed, loyalty, badge) => {
    expect(statBadge(object, printed, loyalty)).toEqual(badge)
  })
})

function renderCard(object: ObjectView) {
  return renderTable({
    ui: <Card id={ID} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

const detail = (power: string, toughness: string): CardDetail =>
  ({
    faces: [{ power, toughness, loyalty: null }],
  }) as unknown as CardDetail

/** Draws the card and lets its picture load. */
async function loaded(object: ObjectView) {
  const view = renderCard(object)
  const img = must(view.container.querySelector('img'))
  act(() => fireEvent.load(img))
  await act(async () => {
    await Promise.resolve()
  })
  return view
}

describe('a card picture on the table', () => {
  it('asks for the picture the drawn width needs, by identity and face', () => {
    const { container } = renderCard(permanent({}, { identity: IDENTITY, face: 1 }))
    expect(must(container.querySelector('img')).getAttribute('src')).toBe(
      '/api/images/oracle/bears-oracle/1/small',
    )
  })

  it.each<[string, string, string, string | null]>([
    ['as printed', '2', '2', null],
    ['modified', '1', '1', 'up'],
  ])('a creature %s gets a badge only when it differs', async (_, power, toughness, modified) => {
    setDetailFetcher(() => Promise.resolve(detail(power, toughness)))
    const { container } = await loaded(permanent({}, { identity: IDENTITY, power: 2, toughness: 2 }))
    const badge = container.querySelector('[data-slot="card-pt"]')
    expect(badge?.getAttribute('data-modified') ?? null).toBe(modified)
    expect(badge === null).toBe(modified === null)
  })

  it('draws the state over a loaded picture, and a tapped one says Tapped by its turn alone', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = await loaded(
      permanent(
        { tapped: true, damage: 2 },
        {
          identity: IDENTITY,
          counters: [['+1/+1', 2]],
          manual: [{ text: 'Flip a coin.', reason: 'random' }],
        },
      ),
    )
    const overlay = must(container.querySelector('[data-slot="card-overlay"]'))
    expect(overlay.textContent).toContain('2 dmg')
    expect(overlay.textContent).toContain('+1/+1 ×2')
    expect(overlay.textContent).not.toContain('Tapped')
    expect(container.querySelector('[data-slot="card-picture"]')).toHaveAttribute('data-turned', 'true')
    expect(must(container.querySelector('[data-slot="card-support"]')).getAttribute('title')).toContain(
      'Flip a coin.',
    )
    expect(container.querySelector('[data-slot="card-face"]')).toBeNull()
  })

  it('has no support mark for a card the engine runs whole', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = await loaded(permanent({}, { identity: IDENTITY }))
    expect(container.querySelector('[data-slot="card-support"]')).toBeNull()
  })

  it('draws the back of a face-down card another seat holds', () => {
    renderCard(permanent({}, { face_down: true, name: '', types: [], subtypes: [], colors: [] }))
    expect(screen.getByRole('img', { name: 'Face-down card' })).toBeInTheDocument()
  })

  it('draws exactly the text face with Text only', () => {
    setImageChoice('text')
    const { container } = renderCard(permanent({}, { identity: IDENTITY }))
    expect(container.querySelector('img')).toBeNull()
    expect(container.querySelector('[data-slot="card-picture"]')).toBeNull()
    expect(container.querySelector('[data-slot="card-face"]')).not.toBeNull()
  })
})

describe('the Card images switch', () => {
  it('turns pictures off and on from the table menu and keeps the choice', () => {
    const noop = (): void => {}
    renderTable({
      ui: (
        <TableMenu
          mode="grid"
          onMode={noop}
          pinnedFocus={false}
          onFollow={noop}
          onConcede={noop}
          open
          onOpenChange={noop}
        />
      ),
    })
    const group = screen.getByRole('group', { name: 'Card images' })
    const text = within(group).getByRole('button', { name: 'Text only' })
    act(() => fireEvent.click(text))
    expect(text).toHaveAttribute('aria-pressed', 'true')
    expect(window.localStorage.getItem(IMAGE_CHOICE_KEY)).toBe('text')
    act(() => fireEvent.click(within(group).getByRole('button', { name: 'On' })))
    expect(window.localStorage.getItem(IMAGE_CHOICE_KEY)).toBe('on')
  })
})
