// Goal: the preview draws the large picture of the face in use, its oracle text as selectable text, and turns a card of several faces over to the other face's picture.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import type { CardsApi } from '@/net/cards'
import type { CardDetail, CardFaceView } from '@/protocol/generated'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { PreviewCard } from './PreviewCard'

afterEach(() => resetImageMemory())

const face = (name: string, oracle_text: string): CardFaceView => ({
  name,
  mana_cost: '',
  type_line: 'Creature',
  oracle_text,
  power: '2',
  toughness: '2',
  loyalty: null,
  defense: null,
  colors: [],
})

const api = (faces: CardFaceView[]): CardsApi => ({
  search: () => Promise.reject(new Error('unused')),
  get: () => Promise.resolve({ faces } as unknown as CardDetail),
})

async function draw(faces: CardFaceView[]) {
  const view = renderTable({
    ui: (
      <PreviewCard
        object={permanent({}, { identity: { oracle_id: 'dfc' } })}
        role={null}
        size="large"
        api={api(faces)}
      />
    ),
  })
  await act(async () => {
    await Promise.resolve()
  })
  return view
}

const srcs = (container: HTMLElement): string[] =>
  [...container.querySelectorAll('img')].map((img) => img.getAttribute('src') ?? '')

describe('PreviewCard', () => {
  it.each<[string, CardFaceView[], boolean]>([
    ['one face', [face('Grizzly Bears', 'Bears.')], false],
    ['two faces', [face('Delver of Secrets', 'Look.'), face('Insectile Aberration', 'Flying')], true],
  ])('%s', async (_, faces, flips) => {
    const { container } = await draw(faces)
    expect(srcs(container)).toEqual(['/api/images/oracle/dfc/0/large'])
    expect(screen.getByText(faces[0]?.oracle_text ?? '')).toHaveClass('select-text')
    const flip = screen.queryByRole('button', { name: /^Show / })
    expect(flip !== null).toBe(flips)
    if (flip === null) return
    act(() => fireEvent.click(flip))
    expect(srcs(container)).toContain('/api/images/oracle/dfc/1/large')
    expect(screen.getByText('Flying')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Show Delver of Secrets' })).toBeInTheDocument()
  })
})
