// Goal: each open game's card shows its commanders' art crops grouped by seat in seat order, each seat's band edged in its seat's hue and its commanders named by the seat's dot; a game without commanders, a commander the database does not know and a picture that fails fall back to the felt with the format's mark or the seat's ground, and the card still names everything.
import { fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { installFakeClock, removeFakeClock, settle } from '@/test-utils/fakeClock'
import { fakeLobbies, lobbyRow } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { seatHue } from '@/features/table/podFigures'
import type { LobbyCommander, LobbyGame } from '@/protocol/generated'
import { commanderNames, commandersBySeat } from './format'
import { PlayPage } from './PlayPage'

beforeEach(installFakeClock)
afterEach(removeFakeClock)

const lead = (seat: number, name: string, oracle?: string): LobbyCommander =>
  oracle === undefined ? { seat, name } : { seat, name, identity: { oracle_id: oracle } }

async function cardOf(game: LobbyGame): Promise<HTMLElement> {
  await renderInRouter(<PlayPage api={fakeLobbies([game]).api} />)
  await settle()
  return screen.getByRole('article', { name: game.host })
}

const bands = (card: HTMLElement) => [...card.querySelectorAll<HTMLElement>('[data-slot="seat-art"]')]

describe('commanders by seat', () => {
  it.each([
    [[], []],
    [
      [lead(2, 'C'), lead(0, 'A')],
      [
        [0, 'A'],
        [2, 'C'],
      ],
    ],
    [
      [lead(1, 'Tymna'), lead(1, 'Thrasios'), lead(0, 'Atraxa')],
      [
        [0, 'Atraxa'],
        [1, 'Tymna and Thrasios'],
      ],
    ],
  ] as const)('groups %j by seat in seat order', (commanders, expected) => {
    const seats = commandersBySeat(lobbyRow({ commanders: [...commanders] }))
    expect(seats.map((seat) => [seat.seat, commanderNames(seat)])).toEqual(expected)
  })
})

describe('the art of a card', () => {
  it("draws each seat's commanders as art crops in its seat's hue, partners side by side, and names them", async () => {
    const card = await cardOf(
      lobbyRow({
        host: 'Bo',
        format: 'commander',
        commanders: [
          lead(0, 'Atraxa', 'o-atraxa'),
          lead(2, 'Tymna', 'o-tymna'),
          lead(2, 'Thrasios', 'o-thrasios'),
        ],
      }),
    )
    const [first, third] = bands(card)
    expect(bands(card).map((band) => band.dataset.seat)).toEqual(['0', '2'])
    expect(first?.style.getPropertyValue('--seat-hue')).toBe(String(seatHue(0)))
    expect(third?.style.getPropertyValue('--seat-hue')).toBe(String(seatHue(2)))
    expect([...(third?.querySelectorAll('img') ?? [])].map((img) => img.getAttribute('src'))).toEqual([
      '/api/images/oracle/o-tymna/0/art_crop',
      '/api/images/oracle/o-thrasios/0/art_crop',
    ])
    expect(card.querySelector('[data-slot="game-art"]')).toHaveAttribute('aria-hidden', 'true')
    const names = within(card).getByRole('list', { name: 'Commanders' })
    expect(
      within(names)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['Atraxa', 'Tymna and Thrasios'])
  })

  it.each([
    ['commander', 'lucide-crown'],
    ['normal', 'lucide-swords'],
  ])(
    'shows a %s game without commanders as the felt with its mark and a lit band per held seat',
    async (format, mark) => {
      const card = await cardOf(lobbyRow({ host: 'Bo', format, filled: 1, total: 3 }))
      const art = card.querySelector('[data-slot="game-art"]')
      expect(art?.querySelector(`.${mark}`)).not.toBeNull()
      expect(bands(card)).toHaveLength(0)
      const strip = [...(art?.querySelector('span')?.children ?? [])]
      expect(strip.map((seat) => seat.classList.contains('opacity-25'))).toEqual([false, true, true])
      expect(within(card).queryByRole('list', { name: 'Commanders' })).toBeNull()
    },
  )

  it("keeps the seat's ground for a commander the database does not know and for a picture that fails", async () => {
    const card = await cardOf(
      lobbyRow({
        host: 'Bo',
        format: 'commander',
        commanders: [lead(0, 'Mystery'), lead(1, 'Krenko', 'o-krenko')],
      }),
    )
    const [unknown, krenko] = bands(card)
    expect(unknown?.querySelector('img')).toBeNull()
    expect(unknown).not.toHaveAttribute('data-pictured')
    expect(krenko).toHaveAttribute('data-pictured')
    const picture = krenko?.querySelector('img')
    if (picture === null || picture === undefined) throw new Error('no picture')
    fireEvent.error(picture)
    expect(krenko?.querySelector('img')).toBeNull()
    expect(krenko).not.toHaveAttribute('data-pictured')
    expect(within(card).getByText('Krenko')).toBeInTheDocument()
  })
})
