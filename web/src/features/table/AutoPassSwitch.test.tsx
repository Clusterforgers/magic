// Goal: the table offers "Skip turns when I can't act" to a seat in a running game, on by default and following the server's word, sends the setting when it is flipped by mouse or keyboard, cannot be flipped while the connection is down, and is not drawn once the seat is out or the game is over.
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'
import { renderLiveTable, renderTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { AUTO_PASS_LABEL } from './AutoPassSwitch'

const toggle = () => screen.queryByRole('switch', { name: AUTO_PASS_LABEL })

describe('the switch', () => {
  it('says what it does, in words, and starts on', () => {
    renderTable()
    expect(AUTO_PASS_LABEL).toBe("Skip turns when I can't act")
    expect(must(toggle())).toBeChecked()
    expect(must(toggle())).not.toHaveAttribute('aria-disabled', 'true')
  })

  it('sends the setting when clicked and shows what it sent', async () => {
    const setAutoPass = vi.fn(() => true)
    const { user, store } = renderTable({ session: { setAutoPass } })
    await user.click(must(toggle()))
    expect(setAutoPass).toHaveBeenCalledExactlyOnceWith(false)
    // The fixture session stands in for the server: it applies what it is told only with its own setter.
    expect(store.getState().autoPass).toBe(true)
  })

  it('can be turned off and on again with the keyboard', async () => {
    const { user, store } = renderTable()
    await user.tab()
    const target = must(toggle())
    target.focus()
    await user.keyboard(' ')
    expect(store.getState().autoPass).toBe(false)
    expect(must(toggle())).not.toBeChecked()
    await user.keyboard(' ')
    expect(store.getState().autoPass).toBe(true)
    expect(must(toggle())).toBeChecked()
  })

  it('can be flipped by clicking its words too', async () => {
    const { user, store } = renderTable()
    await user.click(screen.getByText(AUTO_PASS_LABEL))
    expect(store.getState().autoPass).toBe(false)
  })

  it('shows off when the server says it is off', () => {
    renderTable({ state: (state) => ({ ...state, autoPass: false }) })
    expect(must(toggle())).not.toBeChecked()
  })

  it.each(['connecting', 'reconnecting', 'stopped'] as const)(
    'cannot be flipped while the socket is %s',
    (status) => {
      renderTable({ state: (state) => ({ ...state, status }) })
      expect(must(toggle())).toHaveAttribute('aria-disabled', 'true')
    },
  )
})

describe('who sees it', () => {
  const lost = (state: PlayState): PlayState => {
    const view = structuredClone(must(state.view))
    const me = must(view.players.find((player) => player.seat === state.seat))
    me.lost = 'Conceded'
    return { ...state, view }
  }

  it.each([
    ['a seat that is out', lost],
    ['a game that is over', (state: PlayState): PlayState => ({ ...state, phase: 'over' })],
    ['a table with no view yet', (state: PlayState): PlayState => ({ ...state, view: null })],
  ])('is not drawn for %s', (_label, change) => {
    renderTable({ state: change })
    expect(toggle()).toBeNull()
  })
})

describe('over a live session', () => {
  function mount(autoPass: boolean) {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    const fixture = buildFixture({ seats: 2 })
    h.deliver({ ...welcome, auto_pass: autoPass })
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    return { h, ...renderLiveTable(h.session, fixtureLookup(fixture)) }
  }

  it('sends AutoPass over the socket and then reads on', async () => {
    const { h, user } = mount(true)
    await user.click(must(toggle()))
    expect(h.sentOf('AutoPass')).toEqual([{ type: 'AutoPass', enabled: false }])
    expect(must(toggle())).not.toBeChecked()
    await user.click(must(toggle()))
    expect(h.sentOf('AutoPass')).toEqual([
      { type: 'AutoPass', enabled: false },
      { type: 'AutoPass', enabled: true },
    ])
    expect(must(toggle())).toBeChecked()
  })

  it('shows the setting the server reports in a Welcome', () => {
    mount(false)
    expect(must(toggle())).not.toBeChecked()
  })

  it('goes back to what the server holds after a reconnect', async () => {
    const { h, user } = mount(true)
    await user.click(must(toggle()))
    expect(must(toggle())).not.toBeChecked()
    act(() => h.net.last.fail())
    expect(must(toggle())).toHaveAttribute('aria-disabled', 'true')
    act(() => h.timers.advance(250))
    act(() => {
      h.open()
      h.deliver({ ...welcome, auto_pass: true })
    })
    expect(must(toggle())).not.toHaveAttribute('aria-disabled', 'true')
    expect(must(toggle())).toBeChecked()
  })
})
