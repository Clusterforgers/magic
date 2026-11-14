/** Goal: the top card of your library that a rule lets you see (Sigarda, Future Sight, Bolas's Citadel) is a chip beside your library chip with its picture, in your pod alone; while a permission lets you cast or play it the chip wears the legal ring and says so, and its card, opened in a dialog, casts on a click, and the dialog closes once the card leaves the top; nothing is drawn while no top card is seen. The same card stands at the head of the hand dock with a "Top" mark and casts on a click like a card of the hand. */
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import type { PlayerView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './dev/devGame'
import { TOP_CARD_NAME } from './dev/topCardScene'
import { headLabel, seenTop, TOP_WORDS, topChipName, topDialogLine } from './libraryTop'

const sceneOf = (seats: number, prompt = 'priority-top') => {
  const game = fixtureGame({ seats, prompt })
  const { pending, view } = game.session.store.getState()
  return { game, view: must(view), pending }
}

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

const chips = () => [...document.querySelectorAll<HTMLElement>('[data-zone="library-top"]')]

describe('the words of the top card chip', () => {
  it.each<[string, boolean, string, string]>([
    [
      'a card to cast',
      true,
      'Top of your library: Serra Angel, can be cast or played now, opens the card',
      'Only you see it. Click the card to cast or play it.',
    ],
    ['a card only seen', false, 'Top of your library: Serra Angel, opens the card', 'Only you see it.'],
  ])('names %s', (_name, legal, chip, line) => {
    expect(topChipName('Serra Angel', legal)).toBe(chip)
    expect(topDialogLine(legal)).toBe(line)
  })

  it('takes the first card the owner sees, and none while it sees none', () => {
    const player = { known_top: [7, 8] } as unknown as PlayerView
    expect(seenTop(player)).toBe(7)
    expect(seenTop({} as PlayerView)).toBeUndefined()
  })
})

describe('?prompt=priority-top', () => {
  it.each([2, 4])(
    'offers the seen top card as castable at %i seats, in a decision the wire takes',
    (seats) => {
      const { view, pending } = sceneOf(seats)
      const { decision, id } = must(pending)
      if (!('Priority' in decision.kind)) throw new Error('not a priority decision')
      const top = must(seenTop(must(view.players.find((player) => player.seat === view.viewer))))
      expect(decision.kind.Priority.castable).toContain(top)
      expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
    },
  )

  it('draws one chip, in your own pod, that casts the card from its dialog, which closes once the card leaves', async () => {
    const { game, view } = sceneOf(2)
    const user = table(game)
    const [chip, ...more] = chips()
    expect(more).toHaveLength(0)
    expect(must(chip).closest(`[data-seat="${view.viewer}"]`)).not.toBeNull()
    expect(chip).toHaveAccessibleName(topChipName(TOP_CARD_NAME, true))
    expect(chip).toHaveAttribute('data-legal', '1')
    await user.click(must(chip))
    const dialog = screen.getByRole('dialog', { name: 'Top of your library' })
    expect(dialog).toHaveTextContent(topDialogLine(true))
    const top = must(seenTop(must(view.players.find((player) => player.seat === view.viewer))))
    const card = must(dialog.querySelector<HTMLElement>(`button[data-object-id="${top}"]`))
    expect(card).toHaveAttribute('data-highlight', 'legal')
    await user.click(card)
    expect(game.session.store.getState().pending).toBeNull()
    // The card leaves the top for the stack: the dialog it was opened for closes.
    act(() =>
      game.session.store.setState((state) => ({
        view:
          state.view === null
            ? null
            : {
                ...state.view,
                players: state.view.players.map((player) =>
                  player.seat === view.viewer ? { ...player, known_top: [] } : player,
                ),
              },
      })),
    )
    expect(screen.queryByRole('dialog', { name: 'Top of your library' })).toBeNull()
    expect(chips()).toHaveLength(0)
  })

  it('is a plain chip without the ring while the card is only seen', () => {
    const { game, pending } = sceneOf(2)
    const { decision, id } = must(pending)
    if (!('Priority' in decision.kind)) throw new Error('not a priority decision')
    // The same table, the permission gone: the card is seen and not castable.
    const options = { ...decision.kind.Priority, castable: [] }
    game.session.store.setState({ pending: { id, decision: { ...decision, kind: { Priority: options } } } })
    table(game)
    const [chip] = chips()
    expect(chip).toBeDefined()
    expect(chip).not.toHaveAttribute('data-legal')
  })

  it('draws nothing while no top card is seen', () => {
    const { game } = sceneOf(2, 'priority-main')
    table(game)
    expect(chips()).toHaveLength(0)
  })
})

describe('the top card at the head of the hand dock', () => {
  const dockTop = () => [...document.querySelectorAll<HTMLElement>('[data-region="hand"] [data-library-top]')]

  it.each([2, 4])('stands marked "Top" beside the hand at %i seats and casts on a click', async (seats) => {
    const { game, view } = sceneOf(seats)
    const user = table(game)
    const top = must(seenTop(must(view.players.find((player) => player.seat === view.viewer))))
    const [slot, ...more] = dockTop()
    expect(more).toHaveLength(0)
    expect(must(slot).querySelector('[data-slot="hand-library-top"]')).toHaveTextContent('Top')
    const card = must(must(slot).querySelector<HTMLElement>(`button[data-object-id="${top}"]`))
    expect(card).toHaveAttribute('data-highlight', 'legal')
    expect(card).toHaveAccessibleName(expect.stringContaining(TOP_WORDS))
    expect(card.closest('[role="list"]')).toHaveAccessibleName(
      expect.stringContaining('top card of the library'),
    )
    await user.click(card)
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('is not there while no top card is seen', () => {
    const { game } = sceneOf(2, 'priority-main')
    table(game)
    expect(dockTop()).toHaveLength(0)
  })
})

describe('the words of the head of the dock', () => {
  const entry = (top: boolean) => ({ tax: 0, words: top ? TOP_WORDS : 'in the command zone', top })
  it.each<[string, [number, boolean][], string]>([
    ['commanders', [[1, false]], "Alice's commanders in the command zone"],
    ['the top card', [[2, true]], "Alice's top card of the library"],
    [
      'both',
      [
        [1, false],
        [2, true],
      ],
      "Alice's commanders in the command zone and the top card of the library",
    ],
  ])('names %s', (_name, entries, label) => {
    expect(headLabel('Alice', new Map(entries.map(([id, top]) => [id, entry(top)])))).toBe(label)
  })
})
