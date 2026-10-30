// Goal: the preview draws the picture of the face in use at the size the table draws the card at (normal, never a large one that waits for the CDN behind every other picture), by the printing a deck line chose or else by the card, its oracle text as selectable text, and turns a card of several faces over to the other face's picture.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { imageSizeFor } from '@/features/cards/images'
import type { CardsApi } from '@/net/cards'
import type { CardDetail, CardFaceView, CardIdentity } from '@/protocol/generated'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { SIZE_PX } from './ImageFace'
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

async function draw(
  faces: CardFaceView[],
  identity: CardIdentity = { oracle_id: 'dfc' },
  size: 'medium' | 'large' = 'large',
) {
  const view = renderTable({
    ui: <PreviewCard object={permanent({}, { identity })} role={null} size={size} api={api(faces)} />,
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
    expect(srcs(container)).toEqual(['/api/images/oracle/dfc/0/normal'])
    expect(screen.getByText(faces[0]?.oracle_text ?? '')).toHaveClass('select-text')
    const flip = screen.queryByRole('button', { name: /^Show / })
    expect(flip !== null).toBe(flips)
    if (flip === null) return
    act(() => fireEvent.click(flip))
    expect(srcs(container)).toContain('/api/images/oracle/dfc/1/normal')
    expect(screen.getByText('Flying')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Show Delver of Secrets' })).toBeInTheDocument()
  })
})

describe('the picture the preview asks for', () => {
  it.each<[string, CardIdentity, 'medium' | 'large', string]>([
    [
      'a card of an imported deck, which names no printing',
      { oracle_id: 'ea' },
      'large',
      '/api/images/oracle/ea/0/normal',
    ],
    [
      'a card of a precon, by the printing its line names',
      { oracle_id: 'ea', printing_id: 'pr' },
      'large',
      '/api/images/printing/pr/0/normal',
    ],
    ['a narrow preview beside the card', { oracle_id: 'ea' }, 'medium', '/api/images/oracle/ea/0/normal'],
  ])('is the normal one for %s', async (_what, identity, size, url) => {
    const { container } = await draw([face('Ethereal Armor', 'Enchant creature')], identity, size)
    expect(srcs(container)).toEqual([url])
  })

  it('is the size the table gives a card of the preview width', () => {
    for (const width of [SIZE_PX.medium, SIZE_PX.large]) expect(imageSizeFor(width)).toBe('normal')
  })
})
