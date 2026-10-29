// Goal: the card texts are read once per card and four at a time in the order asked, shared between users of one client, and a card that cannot be read is left out without holding the list back.
import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CardsApi } from '@/net/cards'
import type { CardDetail } from '@/protocol/generated'
import { cardDetail } from '../../builder/fakeCards'
import { useCardTexts } from './useCardTexts'

interface Gate {
  id: string
  resolve: (text: string) => void
  reject: () => void
}

/** A card endpoint whose answers wait for the test; `open` holds the requests in flight. */
function gatedCards() {
  const asked: string[] = []
  const open: Gate[] = []
  const api: Pick<CardsApi, 'get'> = {
    get: (id) =>
      new Promise<CardDetail>((resolve, reject) => {
        asked.push(id)
        open.push({
          id,
          resolve: (text) =>
            resolve(cardDetail(id, { faces: [{ ...cardDetail(id).faces[0]!, oracle_text: text }] })),
          reject: () => reject(new Error('offline')),
        })
      }),
  }
  const answer = (id: string, text = `text of ${id}`) =>
    act(async () => {
      const at = open.findIndex((gate) => gate.id === id)
      open.splice(at, 1)[0]?.resolve(text)
      await Promise.resolve()
    })
  return { api, asked, open, answer }
}

const ids = (count: number, prefix = 'card'): string[] =>
  Array.from({ length: count }, (_, index) => `${prefix}-${index + 1}`)

describe('useCardTexts', () => {
  it('asks four cards at once in the order given and the next one as soon as one answers', async () => {
    const cards = gatedCards()
    const wanted = ids(6, 'a')
    const { result } = renderHook(() => useCardTexts(cards.api, wanted))
    await waitFor(() => expect(cards.asked).toEqual(wanted.slice(0, 4)))
    expect(result.current.pending).toBe(true)
    await cards.answer('a-1')
    await waitFor(() => expect(cards.asked).toEqual(wanted.slice(0, 5)))
    expect(cards.open).toHaveLength(4)
    expect(result.current.faces.get('a-1')).toEqual(['text of a-1'])
  })

  it('is pending until every card has answered, then holds each card’s text by face', async () => {
    const cards = gatedCards()
    const wanted = ids(2, 'b')
    const { result } = renderHook(() => useCardTexts(cards.api, wanted))
    await waitFor(() => expect(cards.open).toHaveLength(2))
    await cards.answer('b-1')
    expect(result.current.pending).toBe(true)
    await cards.answer('b-2', 'Flying')
    expect(result.current.pending).toBe(false)
    expect([...result.current.faces]).toEqual([
      ['b-1', ['text of b-1']],
      ['b-2', ['Flying']],
    ])
  })

  it('asks for a card once however many lists want it, even from another list of the same client', async () => {
    const cards = gatedCards()
    const first = renderHook(() => useCardTexts(cards.api, ['shared', 'c-1']))
    await waitFor(() => expect(cards.open).toHaveLength(2))
    const second = renderHook(() => useCardTexts(cards.api, ['shared', 'c-2']))
    await waitFor(() => expect(cards.open).toHaveLength(3))
    await cards.answer('shared')
    await waitFor(() => expect(second.result.current.faces.has('shared')).toBe(true))
    expect(first.result.current.faces.has('shared')).toBe(true)
    expect(cards.asked.filter((id) => id === 'shared')).toHaveLength(1)
  })

  it('leaves out a card that cannot be read, and is no longer pending for it', async () => {
    const cards = gatedCards()
    const { result } = renderHook(() => useCardTexts(cards.api, ['ok', 'broken']))
    await waitFor(() => expect(cards.open).toHaveLength(2))
    await cards.answer('ok')
    await act(async () => {
      cards.open.find((gate) => gate.id === 'broken')?.reject()
      await Promise.resolve()
    })
    await waitFor(() => expect(result.current.pending).toBe(false))
    expect([...result.current.faces.keys()]).toEqual(['ok'])
  })

  it('asks again for a card that failed when a later list wants it', async () => {
    const cards = gatedCards()
    const first = renderHook(() => useCardTexts(cards.api, ['flaky']))
    await waitFor(() => expect(cards.open).toHaveLength(1))
    await act(async () => {
      cards.open[0]?.reject()
      await Promise.resolve()
    })
    await waitFor(() => expect(first.result.current.pending).toBe(false))
    renderHook(() => useCardTexts(cards.api, ['flaky']))
    await waitFor(() => expect(cards.asked).toEqual(['flaky', 'flaky']))
  })

  it('asks nothing for no cards', () => {
    const cards = gatedCards()
    const { result } = renderHook(() => useCardTexts(cards.api, []))
    expect(result.current).toEqual({ faces: new Map(), pending: false })
    expect(cards.asked).toEqual([])
  })
})
