// Goal: a creature held in a soulbond pair carries a small "Paired" link mark in the foot stack of its picture and on its text face (the icon alone, with the word for a screen reader, when compact), both creatures of the pair carry it, a creature on its own carries none, the spoken name and the preview name the partner, and a hidden partner is told as "a face-down creature".
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice, setImageChoice } from '@/features/cards/imageSetting'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import { CardPreview } from './CardPreview'
import type { CardSize } from './cardSize'
import { setDetailFetcher } from './printed'

const MINE = 900
const THEIRS = 901
const IDENTITY = { oracle_id: 'bears-oracle' }

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

/** A pair on the table: Lookout (900) paired with the partner (901) when `paired`, each with the other's id. */
function pair(partner: Partial<ObjectView> = {}, paired = true): ObjectView[] {
  return [
    permanent(
      {},
      { id: MINE, name: 'Tandem Lookout', identity: IDENTITY, ...(paired ? { paired_with: THEIRS } : {}) },
    ),
    permanent(
      {},
      {
        id: THEIRS,
        name: 'Grizzly Bears',
        identity: { oracle_id: 'bears-oracle' },
        ...(paired ? { paired_with: MINE } : {}),
        ...partner,
      },
    ),
  ]
}

function renderPair(objects: ObjectView[], id: number, size: CardSize = 'normal') {
  return renderTable({
    ui: (
      <>
        <Card id={id} size={size} />
        <CardPreview />
      </>
    ),
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, ...objects]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

/** Draws the card and lets its picture load, so the marks sit over the picture. */
async function loaded(objects: ObjectView[], id: number, size: CardSize = 'normal') {
  const view = renderPair(objects, id, size)
  act(() => fireEvent.load(must(view.container.querySelector('img'))))
  await act(async () => {
    await Promise.resolve()
  })
  return view
}

const markOf = (container: HTMLElement): HTMLElement | null =>
  container.querySelector('[data-slot="card-marks-foot"] [data-slot="card-paired"]')

describe('the paired mark over a picture', () => {
  it('sits in the foot stack of both creatures of a pair, with the word', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    for (const id of [MINE, THEIRS]) {
      const { container, unmount } = await loaded(pair(), id)
      expect(must(markOf(container))).toHaveTextContent('Paired')
      unmount()
    }
  })

  it('keeps the icon and leaves the word to a screen reader when compact', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = await loaded(pair(), MINE, 'compact')
    const mark = must(markOf(container))
    expect(mark.querySelector('svg')).not.toBeNull()
    expect(must(mark.querySelector('span'))).toHaveClass('sr-only')
  })

  it('is left out for a creature on its own', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = await loaded(pair({}, false), MINE)
    expect(container.querySelector('[data-slot="card-paired"]')).toBeNull()
  })
})

describe('the paired mark on a text face', () => {
  it('shows on both creatures when pictures are off', () => {
    setImageChoice('text')
    for (const id of [MINE, THEIRS]) {
      const { container, unmount } = renderPair(pair(), id)
      expect(must(container.querySelector('[data-slot="card-paired"]'))).toHaveTextContent('Paired')
      unmount()
    }
  })

  it('is left out of a creature on its own', () => {
    setImageChoice('text')
    const { container } = renderPair(pair({}, false), MINE)
    expect(container.querySelector('[data-slot="card-paired"]')).toBeNull()
  })
})

describe('the partner by name', () => {
  it('is in the spoken name of the card', () => {
    setImageChoice('text')
    const { container } = renderPair(pair(), MINE)
    expect(must(container.querySelector('button')).getAttribute('aria-label')).toContain(
      'paired with Grizzly Bears',
    )
  })

  it('is in the preview, in its own line', async () => {
    setImageChoice('text')
    const { container, user } = renderPair(pair(), MINE)
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${MINE}"]`)))
    const preview = screen.getByTestId('card-preview')
    expect(must(preview.querySelector('[data-slot="preview-paired"]'))).toHaveTextContent(
      'Paired with Grizzly Bears.',
    )
  })

  it('has no line in the preview of a creature on its own', async () => {
    setImageChoice('text')
    const { container, user } = renderPair(pair({}, false), MINE)
    await user.hover(must(container.querySelector<HTMLElement>(`button[data-object-id="${MINE}"]`)))
    expect(screen.getByTestId('card-preview').querySelector('[data-slot="preview-paired"]')).toBeNull()
  })

  it('says "a face-down creature" for a partner the viewer may not look at', () => {
    setImageChoice('text')
    const { container } = renderPair(pair({ name: '', face_down: true }), MINE)
    expect(must(container.querySelector('button')).getAttribute('aria-label')).toContain(
      'paired with a face-down creature',
    )
  })
})
