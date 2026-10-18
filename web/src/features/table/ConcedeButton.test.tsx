// Goal: the table offers Concede only to a seat that is still in a running game, asks for confirmation before sending anything, sends exactly one Concede on confirm, sends none on cancel or Escape, keeps the question up when the send is refused, and shows the concession in the seat panel and the log once the server reports it.
import { act, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Session } from '@/play/session'
import { appendLog } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { LossReason } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable, renderTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { LogPanel } from './LogPanel'

const concedeButton = () => screen.queryByRole('button', { name: 'Concede' })
const dialog = () => screen.queryByRole('alertdialog')

function setup(seats = 2, state?: (state: PlayState) => PlayState, concede: Session['concede'] = () => true) {
  const spy = vi.fn(concede)
  const table = renderTable({ seats, state, session: { concede: spy } })
  return { ...table, spy }
}

describe('asking first', () => {
  it('opens a question and sends nothing yet', async () => {
    const { user, spy } = setup()
    await user.click(must(concedeButton()))
    const question = within(must(dialog()))
    expect(question.getByText('Concede the game?')).toBeInTheDocument()
    expect(spy).not.toHaveBeenCalled()
  })

  it('moves focus to the safe choice', async () => {
    const { user } = setup()
    await user.click(must(concedeButton()))
    await waitFor(() =>
      expect(within(must(dialog())).getByRole('button', { name: 'Keep playing' })).toHaveFocus(),
    )
  })

  it.each([
    ['2 seats', 2, 'Your opponent wins.'],
    ['3 seats', 3, 'The other players play on.'],
    ['12 seats', 12, 'The other players play on.'],
  ])('says what follows at a table of %s', async (_label, seats, words) => {
    const { user } = setup(seats)
    await user.click(must(concedeButton()))
    expect(must(dialog())).toHaveTextContent(words)
  })

  it('sends one Concede when confirmed and closes the question', async () => {
    const { user, spy } = setup()
    await user.click(must(concedeButton()))
    await user.click(within(must(dialog())).getByRole('button', { name: 'Concede' }))
    expect(spy).toHaveBeenCalledTimes(1)
    await waitFor(() => expect(dialog()).toBeNull())
  })

  it('sends nothing when the player keeps playing', async () => {
    const { user, spy } = setup()
    await user.click(must(concedeButton()))
    await user.click(within(must(dialog())).getByRole('button', { name: 'Keep playing' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(spy).not.toHaveBeenCalled()
    expect(concedeButton()).toBeInTheDocument()
  })

  it('sends nothing on Escape', async () => {
    const { user, spy } = setup()
    await user.click(must(concedeButton()))
    await user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    expect(spy).not.toHaveBeenCalled()
  })

  it('asks again from the start after a cancel', async () => {
    const { user, spy } = setup()
    await user.click(must(concedeButton()))
    await user.click(within(must(dialog())).getByRole('button', { name: 'Keep playing' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await user.click(must(concedeButton()))
    expect(must(dialog())).toBeInTheDocument()
    expect(spy).not.toHaveBeenCalled()
  })

  it('keeps the question up with a note when the send is refused, and a retry can go through', async () => {
    let open = false
    const { user, spy } = setup(2, undefined, () => open)
    await user.click(must(concedeButton()))
    await user.click(within(must(dialog())).getByRole('button', { name: 'Concede' }))
    expect(within(must(dialog())).getByRole('alert')).toHaveTextContent('Could not reach the game')
    open = true
    await user.click(within(must(dialog())).getByRole('button', { name: 'Concede' }))
    expect(spy).toHaveBeenCalledTimes(2)
    await waitFor(() => expect(dialog()).toBeNull())
  })
})

describe('who is offered it', () => {
  it('offers it to a seat in a running game', () => {
    setup()
    expect(concedeButton()).toBeInTheDocument()
  })

  it.each<[string, (state: PlayState) => PlayState]>([
    ['the game is over', (state) => ({ ...state, phase: 'over', result: 'Draw' })],
    [
      'the view says the game finished',
      (state) => {
        const view = clone(must(state.view))
        view.stage = 'Finished'
        return { ...state, view }
      },
    ],
    [
      'the view carries a result',
      (state) => {
        const view = clone(must(state.view))
        view.result = { Winner: 1 }
        return { ...state, view }
      },
    ],
    [
      'the seat is already out',
      (state) => {
        const view = clone(must(state.view))
        must(view.players[0]).lost = 'ZeroLife'
        return { ...state, view }
      },
    ],
    [
      'the seat has already conceded',
      (state) => {
        const view = clone(must(state.view))
        must(view.players[0]).lost = 'Conceded'
        return { ...state, view }
      },
    ],
    ['the client holds no seat (a spectator)', (state) => ({ ...state, seat: null })],
  ])('hides it when %s', (_why, state) => {
    setup(2, state)
    expect(concedeButton()).toBeNull()
  })

  it('stays out of the way of a seat that lost, but not of the others', () => {
    const fixture = buildFixture({ seats: 3 })
    const lost = clone(fixture)
    must(lost.view.players[0]).lost = 'EmptyLibrary'
    renderTable({ fixture: lost })
    expect(concedeButton()).toBeNull()
  })

  it('takes the button and the open question away when the game ends underneath', async () => {
    const { user, update } = setup()
    await user.click(must(concedeButton()))
    update(() => ({ phase: 'over', result: 'Draw' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(concedeButton()).toBeNull()
  })
})

describe('after the server reports it', () => {
  const conceded = (reason: LossReason) => (state: PlayState) => {
    const view = clone(must(state.view))
    must(view.players[1]).lost = reason
    return {
      view,
      log: appendLog(state.log, 900, [{ Public: { PlayerLost: { player: 1, reason } } }]),
    }
  }

  it('marks the conceded seat in its panel and names who conceded in the log', () => {
    const { update, container } = renderTable({ seats: 2 })
    update(conceded('Conceded'))
    const panel = must(container.querySelector('[data-seat="1"]'))
    expect(panel).toHaveAttribute('data-eliminated', 'true')
    expect(panel).toHaveTextContent('Eliminated: conceded')
  })

  it('writes "Bo conceded" in the log', () => {
    const { update } = renderTable({ seats: 2, ui: <LogPanel /> })
    update(conceded('Conceded'))
    expect(screen.getByRole('log', { name: 'Game log' })).toHaveTextContent('Bo conceded')
  })

  it('removes the button of the seat that conceded', () => {
    const { update } = setup()
    update((state) => {
      const view = clone(must(state.view))
      must(view.players[0]).lost = 'Conceded'
      return { view }
    })
    expect(concedeButton()).toBeNull()
  })
})

describe('over a real session', () => {
  function mount() {
    const fixture = buildFixture({ seats: 2 })
    const h = startSession()
    h.join(0, 2)
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    return { h, fixture, ...renderLiveTable(h.session, fixtureLookup(fixture)) }
  }

  it('sends one Concede message on confirm, and the button goes once the server reports the loss', async () => {
    const { h, fixture, user } = mount()
    await user.click(must(concedeButton()))
    await user.click(within(must(dialog())).getByRole('button', { name: 'Concede' }))
    expect(h.sentOf('Concede')).toEqual([{ type: 'Concede' }])
    expect(concedeButton()).not.toBeNull()
    const lost = clone(fixture.view)
    must(lost.players[0]).lost = 'Conceded'
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: lost }))
    expect(concedeButton()).toBeNull()
    expect(screen.getByRole('region', { name: 'Alice (you)' })).toHaveTextContent('Eliminated: conceded')
  })

  it('sends nothing while the connection is down and says so', async () => {
    const { h, user } = mount()
    act(() => h.session.disconnect())
    await user.click(must(concedeButton()))
    await user.click(within(must(dialog())).getByRole('button', { name: 'Concede' }))
    expect(h.sentOf('Concede')).toEqual([])
    expect(within(must(dialog())).getByRole('alert')).toBeInTheDocument()
  })
})
