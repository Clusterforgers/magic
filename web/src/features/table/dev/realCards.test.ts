// Goal: `/dev/table?cards=real` identifies the fixture's cards by the exact names the card search answers (a double-faced card by its front), leaves tokens and abilities alone, and puts the showcase on the viewer's battlefield.
import { describe, expect, it } from 'vitest'
import type { CardsApi } from '@/net/cards'
import type { CardHit, CardSearchPage } from '@/protocol/generated'
import { buildFixture } from './fixture'
import { fetchRealIds, withRealCards } from './realCards'

const hit = (name: string, oracle_id: string): CardHit => ({ name, oracle_id }) as unknown as CardHit

const api: CardsApi = {
  search: ({ text }) =>
    Promise.resolve({
      cards: [
        hit(`${text ?? ''} Jr.`, 'wrong'),
        hit(
          text === 'Delver of Secrets' ? 'Delver of Secrets // Insectile Aberration' : (text ?? ''),
          `id:${text ?? ''}`,
        ),
      ],
    } as unknown as CardSearchPage),
  get: () => Promise.reject(new Error('unused')),
}

describe('real cards for the dev table', () => {
  it.each([
    ['Grizzly Bears', 'id:Grizzly Bears'],
    ['Delver of Secrets', 'id:Delver of Secrets'],
  ])('finds %s', async (name, id) => {
    expect(await fetchRealIds(api, [name, name])).toEqual({ [name]: id })
  })

  it('identifies the cards and adds the showcase to the battlefield', () => {
    const fixture = buildFixture({ seats: 2 })
    const name = fixture.view.objects[0]?.name ?? ''
    const real = withRealCards(fixture, { [name]: 'x', 'Jace Beleren': 'jace' }, 0)
    expect(
      real.view.objects.filter((object) => object.name === name).every((o) => o.identity?.oracle_id === 'x'),
    ).toBe(true)
    const added = real.view.objects.slice(fixture.view.objects.length)
    expect(added.map((object) => object.name)).toEqual([
      'Soldier',
      'Jace Beleren',
      'Insectile Aberration',
      'Grizzly Bears',
    ])
    expect(added.find((object) => object.kind === 'Token')?.identity).toBeUndefined()
    expect(real.view.battlefield.cards).toEqual(expect.arrayContaining(added.map((object) => object.id)))
  })
})
