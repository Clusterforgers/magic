// Goal: a deck of the list shows each of its commanders (a partner or Background too) as a picture and a name, a deck without any shows none, and the client refuses a list whose commanders are not cards.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createDecksApi } from '@/net/decks'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { cardHit, deckItem } from './fakeDecks'
import { deckRows, renderDecks } from './renderDecks'

const TYMNA = cardHit('Tymna the Weaver', { color_identity: ['W', 'B'] })
const THRASIOS = cardHit('Thrasios, Triton Hero', { color_identity: ['U', 'G'] })

describe('the commanders of a deck in the list', () => {
  it('are a picture and a name each, in the order of the command zone', async () => {
    await renderDecks({
      decks: [
        deckItem({ id: 'deck-1', name: 'Partners', commanders: [TYMNA, THRASIOS] }),
        deckItem({ id: 'deck-2', name: 'Burn', format: 'modern' }),
      ],
    })
    await screen.findByRole('list', { name: 'Your decks' })
    const [partners, burn] = deckRows() as [HTMLElement, HTMLElement]
    const names = within(partners)
      .getByRole('list', { name: 'Commanders' })
      .querySelectorAll('[data-commander] [data-slot="card-name"]')
    expect([...names].map((item) => item.textContent)).toEqual([TYMNA.name, THRASIOS.name])
    expect(within(burn).queryByRole('list', { name: 'Commanders' })).toBeNull()
  })
})

describe('reading the list', () => {
  const read = (commanders: unknown) =>
    createDecksApi({
      fetch: fakeFetch(() => jsonResponse(200, { decks: [{ ...deckItem(), commanders }] })).fetch,
    }).list()

  it('keeps the commanders as cards', async () => {
    expect((await read([TYMNA]))[0]?.commanders.map((card) => card.name)).toEqual([TYMNA.name])
  })

  it.each([
    ['a missing list', undefined],
    ['a name for a card', ['Tymna']],
    ['one object', TYMNA],
  ])('refuses %s', async (_name, commanders) => {
    await expect(read(commanders)).rejects.toMatchObject({ code: 'bad_response' })
  })
})
