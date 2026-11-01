// Goal: the card menu offers to mark a card as broken (a fix mark made from the table), shows whether it is marked and confirms each press, leaves out a card without a printed identity and a visitor without marks, and keeps its entries a plain list.
import { act, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FixMarksApiContext } from '@/features/fixmarks/apiContext'
import { fixMarksClient } from '@/features/fixmarks/client'
import { DeckError } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
import type { FixMarkView } from '@/protocol/generated'
import { indexById } from '@/play/view'
import { card } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from '../Card'
import { buildFixture } from '../dev/fixture'
import { fixMarkGroup, fixMarkWords } from './fixMark'
import { ManualMenuProvider } from './ManualMenu'

const ID = 900
const SOL = 'oracle-sol'

const view = (id: string): FixMarkView => ({ oracle_id: id, name: 'Sol Ring', note: null, created_at: 't' })

function fakeApi(marks: FixMarkView[], over: Partial<FixMarksApi> = {}): FixMarksApi {
  return {
    list: () => Promise.resolve(marks),
    mark: vi.fn((id: string) => Promise.resolve(view(id))),
    unmark: vi.fn(() => Promise.resolve()),
    ...over,
  }
}

function open(api: FixMarksApi, identity: boolean = true): void {
  const object = card({ name: 'Sol Ring', types: ['Artifact'], subtypes: [] })
  renderTable({
    fixture: buildFixture({ seats: 2, permanents: 4, hand: 2 }),
    ui: (
      <FixMarksApiContext value={api}>
        <ManualMenuProvider>
          <Card id={ID} />
        </ManualMenuProvider>
      </FixMarksApiContext>
    ),
    state: (first) => {
      const next = clone(must(first.view))
      next.objects = [
        ...next.objects,
        { ...object, id: ID, ...(identity ? { identity: { oracle_id: SOL } } : {}) },
      ]
      return { ...first, view: next, objectsById: indexById(next.objects) }
    },
  })
  const button = document.querySelector<HTMLElement>(`button[data-object-id="${ID}"]`) as HTMLElement
  act(() => {
    button.dispatchEvent(
      new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 40, clientY: 50 }),
    )
  })
}

/** The entry once the marks have loaded and it can be pressed. */
async function ready(name: RegExp): Promise<HTMLElement> {
  const found = await screen.findByRole('menuitem', { name })
  await waitFor(() => expect(found).not.toHaveAttribute('aria-disabled'))
  return found
}

afterEach(() => {
  fixMarksClient.clear()
})

describe('the fix mark entry of the card menu', () => {
  it('marks an unmarked card and confirms it', async () => {
    const api = fakeApi([])
    open(api)
    const entry = await ready(/Mark as broken/)
    expect(entry).toHaveTextContent('Play by hand')
    act(() => entry.click())
    await waitFor(() => expect(api.mark).toHaveBeenCalledWith(SOL, null))
    expect(await screen.findByRole('status')).toHaveTextContent(fixMarkWords('Sol Ring', true))
    expect(await screen.findByRole('menuitem', { name: /Remove broken mark/ })).toHaveTextContent('Marked')
  })

  it('takes the mark back from a marked card', async () => {
    const api = fakeApi([view(SOL)])
    open(api)
    const entry = await ready(/Remove broken mark/)
    act(() => entry.click())
    await waitFor(() => expect(api.unmark).toHaveBeenCalledWith(SOL))
    expect(await screen.findByRole('status')).toHaveTextContent(fixMarkWords('Sol Ring', false))
  })

  it('says why a refused mark failed', async () => {
    const api = fakeApi([], { mark: () => Promise.reject(new DeckError('not_found', 404, 'x')) })
    open(api)
    const entry = await ready(/Mark as broken/)
    act(() => entry.click())
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('does not know this card'))
  })

  it.each([
    ['a card without a printed identity', fakeApi([]), false],
    [
      'a visitor without a session',
      fakeApi([], { list: () => Promise.reject(new DeckError('unauthenticated', 401, 'x')) }),
      true,
    ],
  ])('is left out for %s', async (_name, api, identity) => {
    open(api, identity)
    await waitFor(() => expect(screen.getByRole('menu')).toBeInTheDocument())
    await new Promise((done) => setTimeout(done, 20))
    expect(within(screen.getByRole('menu')).queryByRole('menuitem', { name: /broken/ })).toBeNull()
  })
})

describe('fixMarkGroup', () => {
  it.each([
    [false, 'Mark as broken'],
    [true, 'Remove broken mark'],
  ])('is one entry for marked=%s', (marked, label) => {
    const group = fixMarkGroup(marked)
    expect(group.items.map((item) => item.label)).toEqual([label])
  })
})
