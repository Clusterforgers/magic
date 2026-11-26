// Goal: at two and four seats a press on a hand card the offer leaves out says why at the card in one line (One Voice with the boss's modifier chip lit, sorcery speed, too little mana), nothing is said over the hand before a press, a press elsewhere or Escape hushes it, and a press on a hand card while another seat decides says whom the game waits for, but never to a spectator or a replay.
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import type { UnplayableCard } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { useBlockedTell } from './blockedTell'
import { fixtureGame } from './dev/devGame'
import { ONE_VOICE } from './dev/blockedScene'

afterEach(() => useBlockedTell.getState().hush())

const table = (game: ReturnType<typeof fixtureGame>) => {
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{}}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return userEvent.setup()
}

/** The first button the table draws for the card `id`. */
const cardButton = (id: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))

const tell = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-slot="blocked-tell"]')

function blockedScene(seats: number) {
  const game = fixtureGame({ seats, prompt: 'priority-blocked', boss: 0 })
  const { pending } = game.session.store.getState()
  const kind = must(pending).decision.kind
  if (!('Priority' in kind)) throw new Error('a priority decision')
  const entries = kind.Priority.unplayable ?? []
  const by = (reason: UnplayableCard['reason']): number =>
    must(entries.find((entry) => entry.reason === reason)).card
  return { game, held: by('Restricted'), slow: by('Timing'), dear: by('Mana') }
}

describe.each([2, 4])('a press on a blocked hand card at %i seats', (seats) => {
  it('says nothing over the hand before a press', () => {
    const { game } = blockedScene(seats)
    table(game)
    expect(tell()).toBeNull()
  })

  it('names One Voice and lights its chip', async () => {
    const { game, held } = blockedScene(seats)
    const user = table(game)
    await user.click(cardButton(held))
    expect(must(tell())).toHaveTextContent(`${ONE_VOICE}: you have cast your spell this turn.`)
    expect(must(tell())).toHaveAttribute('role', 'status')
    const chip = [...document.querySelectorAll('[data-slot="boss-ability"]')].find((each) =>
      each.textContent?.includes(ONE_VOICE),
    )
    expect(must(chip)).toHaveAttribute('data-blocking', 'true')
  })

  it.each<['slow' | 'dear', RegExp]>([
    ['slow', /^Sorcery speed: only in your main phase with an empty stack\.$/],
    ['dear', /^Needs .+: you have 1 mana available\.$/],
  ])('says the %s card’s reason in one line', async (which, line) => {
    const scene = blockedScene(seats)
    const user = table(scene.game)
    await user.click(cardButton(scene[which]))
    expect(must(tell()).textContent).toMatch(line)
  })

  it('is hushed by a press elsewhere and by Escape', async () => {
    const { game, held } = blockedScene(seats)
    const user = table(game)
    await user.click(cardButton(held))
    expect(tell()).not.toBeNull()
    fireEvent.pointerDown(document.body)
    expect(tell()).toBeNull()
    await user.click(cardButton(held))
    expect(tell()).not.toBeNull()
    act(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    })
    expect(tell()).toBeNull()
  })
})

describe.each([2, 4])('a press on a hand card while another seat decides at %i seats', (seats) => {
  it('says whom the game waits for', async () => {
    const game = fixtureGame({ seats })
    const { view } = game.session.store.getState()
    const other = must(view).players.find((player) => player.seat !== must(view).viewer)
    act(() => {
      game.session.store.setState((state) => ({
        pending: null,
        view:
          state.view === null ? null : { ...state.view, priority: { holder: must(other).seat, passed: [] } },
      }))
    })
    const user = table(game)
    const card = must(must(view).players[must(view).viewer]?.hand.cards?.[0])
    await user.click(cardButton(card))
    expect(must(tell()).textContent).toMatch(/^Waiting for .+: you can play once you have priority\.$/)
    expect(screen.queryByText(/you have cast your spell/)).toBeNull()
  })

  it('says no wait to a spectator or a replay, who plays nothing', async () => {
    const game = fixtureGame({ seats })
    const { view } = game.session.store.getState()
    const other = must(view).players.find((player) => player.seat !== must(view).viewer)
    act(() => {
      game.session.store.setState((state) => ({
        pending: null,
        view:
          state.view === null
            ? null
            : { ...state.view, spectator: true, priority: { holder: must(other).seat, passed: [] } },
      }))
    })
    const user = table(game)
    const card = must(must(view).players[must(view).viewer]?.hand.cards?.[0])
    await user.click(cardButton(card))
    expect(tell()).toBeNull()
  })
})
