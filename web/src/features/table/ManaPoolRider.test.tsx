// Goal: mana of a pool whose uses are riders alone (Path of Ancestry's) is drawn as plain mana with a small rider mark and the rider's words in its tooltip and for a screen reader, never as locked restricted mana; restricted mana keeps its lock, a unit with a rider beside a use is restricted, and the pool of a seat of the game words the rider from the permanent that made the mana.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ManaPool as PoolValue, ObjectView, Restricted, SpendOn } from '@/protocol/generated'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { LiveManaPool } from './LiveManaPool'
import { ManaPool } from './ManaPool'

const SOURCE = 930
const WHEN: SpendOn = {
  When: {
    spell: { types: ['Creature'], facts: ['SharesCommanderType'] },
    effects: [{ Scry: { count: 1 } }],
    source: SOURCE,
  },
}
const GENERIC =
  'When this mana is spent to cast a creature spell that shares a creature type with your commander, an ability of the permanent that made it triggers.'

const pool = (counts: string, ...restricted: Restricted[]): PoolValue => ({
  counts: ['W', 'U', 'B', 'R', 'G', 'C'].map(
    (letter) => counts.split(letter).length - 1,
  ) as PoolValue['counts'],
  restricted,
})

const kinds = (container: HTMLElement) =>
  [...container.querySelectorAll('[data-mana]')].map((el) => [
    el.getAttribute('data-mana'),
    el.querySelector('.font-semibold')?.textContent,
    el.hasAttribute('data-restricted'),
    el.hasAttribute('data-rider'),
  ])

describe('mana that carries a rider', () => {
  it.each<[string, PoolValue, Array<[string, string, boolean, boolean]>]>([
    ['alone', pool('', { mana: 'White', only: [WHEN] }), [['White', '×1', false, true]]],
    [
      'after the plain mana of its kind, apart from it',
      pool('WW', { mana: 'White', only: [WHEN] }),
      [
        ['White', '×2', false, false],
        ['White', '×1', false, true],
      ],
    ],
    [
      'two units of a kind together, before restricted mana',
      pool(
        'G',
        { mana: 'Red', only: ['CostWithX'] },
        { mana: 'White', only: [WHEN] },
        { mana: 'White', only: [WHEN] },
      ),
      [
        ['Green', '×1', false, false],
        ['White', '×2', false, true],
        ['Red', '×1', true, false],
      ],
    ],
    [
      'beside a use is restricted, with its lock',
      pool('', { mana: 'White', only: [WHEN, 'CostWithX'] }),
      [['White', '×1', true, false]],
    ],
  ])('draws mana %s', (_case, given, expected) => {
    const { container } = render(<ManaPool pool={given} />)
    expect(kinds(container)).toEqual(expected)
  })

  it('has no lock and a mark of its own', () => {
    const { container } = render(<ManaPool pool={pool('', { mana: 'White', only: [WHEN] })} />)
    const mana = must(container.querySelector('[data-rider]'))
    expect(mana.querySelector('.lucide-lock')).toBeNull()
    expect(mana.querySelector('.lucide-sparkles')).not.toBeNull()
    expect(mana.className).not.toContain('border')
  })

  it('names the rider in its tooltip and for a screen reader', () => {
    const { container } = render(<ManaPool pool={pool('', { mana: 'White', only: [WHEN] })} />)
    const mana = must(container.querySelector('[data-rider]'))
    expect(mana).toHaveAttribute('title', GENERIC)
    expect(mana.querySelector('.sr-only')).toHaveTextContent(`1 white, with a rider: ${GENERIC}`)
  })

  it('takes the words it is given by rider', () => {
    const said =
      'When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1.'
    const { container } = render(
      <ManaPool
        pool={pool('', { mana: 'White', only: [WHEN] })}
        words={new Map([[JSON.stringify(WHEN.When), said]])}
      />,
    )
    expect(must(container.querySelector('[data-rider]'))).toHaveAttribute('title', said)
  })

  it('draws nothing for a pool with nothing in it', () => {
    const { container } = render(<ManaPool pool={pool('')} />)
    expect(container).toBeEmptyDOMElement()
  })
})

describe('the pool of a seat of the game', () => {
  const path: ObjectView = card({
    id: SOURCE,
    name: 'Path of Ancestry',
    types: ['Land'],
    mana_cost: null,
    activated: [
      {
        cost: '{T}',
        effect:
          'Add one mana of any color in your commander’s color identity. When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1.',
        mana: true,
      },
    ],
  })

  it('words the rider with the sentence of the permanent that made the mana', () => {
    const { container } = renderTable({
      ui: <LiveManaPool pool={pool('', { mana: 'White', only: [WHEN] })} />,
      state: (state) => {
        const view = clone(must(state.view))
        view.objects = [...view.objects, path]
        return { ...state, view, objectsById: indexById(view.objects) }
      },
    })
    expect(must(container.querySelector('[data-rider]'))).toHaveAttribute(
      'title',
      'When that mana is spent to cast a creature spell that shares a creature type with your commander, scry 1.',
    )
    expect(screen.getByRole('group', { name: 'Mana pool' })).toBeInTheDocument()
  })

  it('words it by the kind of spell when the permanent is gone', () => {
    const { container } = renderTable({
      ui: <LiveManaPool pool={pool('', { mana: 'White', only: [WHEN] })} />,
    })
    expect(must(container.querySelector('[data-rider]'))).toHaveAttribute('title', GENERIC)
  })
})
