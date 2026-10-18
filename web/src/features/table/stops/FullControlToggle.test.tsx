// Goal: the table offers Full Control as a toggle that says its state in words and in aria-pressed, asks for Held (or Off) when pressed and for any mode from its menu with the keys printed beside them, shows only what the server holds, cannot be used while the connection is down, answers Control and Control with Shift, and is not drawn once the seat is out or the game is over.
import { act, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { FullControlMode } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'
import { renderLiveTable, renderTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const MODES: FullControlMode[] = ['Off', 'NextWindow', 'Held']
const toggle = (): HTMLElement | null => screen.queryByRole('button', { name: /^Full Control: / })
const trigger = (): HTMLElement => screen.getByRole('button', { name: 'Full Control options' })
const withMode =
  (fullControl: FullControlMode) =>
  (state: PlayState): PlayState => ({ ...state, fullControl })

describe('the toggle', () => {
  it('is a labelled group with a button that says Full Control is off, in words and in aria-pressed', () => {
    renderTable()
    expect(screen.getByRole('group', { name: 'Full Control' })).toHaveAttribute('data-mode', 'Off')
    const button = must(toggle())
    expect(button).toHaveAccessibleName('Full Control: off')
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button).toHaveTextContent('Full Control')
    expect(must(button.querySelector('[data-slot="full-control-state"]'))).toHaveTextContent('off')
  })

  it.each([
    ['Off', 'Full Control: off', 'off', 'false'],
    ['NextWindow', 'Full Control: next window', 'next window', 'true'],
    ['Held', 'Full Control: held', 'held', 'true'],
  ] satisfies [FullControlMode, string, string, string][])(
    'shows %s as "%s" with aria-pressed %s',
    (mode, name, word, pressed) => {
      renderTable({ state: withMode(mode) })
      const button = must(toggle())
      expect(button).toHaveAccessibleName(name)
      expect(button).toHaveAttribute('aria-pressed', pressed)
      expect(must(button.querySelector('[data-slot="full-control-state"]'))).toHaveTextContent(word)
      expect(screen.getByRole('group', { name: 'Full Control' })).toHaveAttribute('data-mode', mode)
    },
  )

  it('describes what pressing does and names the keys, to a screen reader and in the tooltip', () => {
    renderTable()
    const button = must(toggle())
    expect(button).toHaveAccessibleDescription(expect.stringContaining('Press to be asked at every step'))
    expect(button).toHaveAttribute('title', expect.stringContaining('Ctrl asks for the next window'))
    expect(button).toHaveAttribute('title', expect.stringContaining('Ctrl+Shift holds it'))
    expect(button).toHaveAttribute('aria-keyshortcuts', 'Control Control+Shift')
  })

  it('looks pressed when it is on', () => {
    renderTable({ state: withMode('Held') })
    expect(must(toggle())).toHaveClass('bg-primary')
  })

  it('follows the server when the mode changes underneath it', () => {
    const { update } = renderTable()
    update(() => ({ fullControl: 'NextWindow' }))
    expect(must(toggle())).toHaveAccessibleName('Full Control: next window')
    update(() => ({ fullControl: 'Off' }))
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')
  })
})

describe('pressing it', () => {
  it.each([
    ['Off', 'Held'],
    ['NextWindow', 'Off'],
    ['Held', 'Off'],
  ] satisfies [FullControlMode, FullControlMode][])('in %s asks the session for %s', async (mode, asked) => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user } = renderTable({ session: { setFullControl }, state: withMode(mode) })
    await user.click(must(toggle()))
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith(asked)
  })

  it('shows nothing new until the server says so', async () => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user, store } = renderTable({ session: { setFullControl } })
    await user.click(must(toggle()))
    expect(store.getState().fullControl).toBe('Off')
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')
    expect(must(toggle())).toHaveAttribute('aria-pressed', 'false')
  })

  it('shows the word the server answers with', async () => {
    const { user } = renderTable()
    await user.click(must(toggle()))
    expect(must(toggle())).toHaveAccessibleName('Full Control: held')
    await user.click(must(toggle()))
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')
  })

  it.each([
    ['Space', ' '],
    ['Enter', '{Enter}'],
  ])('works from the keyboard with %s', async (_name, key) => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user } = renderTable({ session: { setFullControl } })
    act(() => must(toggle()).focus())
    await user.keyboard(key)
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('Held')
  })
})

describe('the menu', () => {
  const items = async (): Promise<HTMLElement[]> => screen.findAllByRole('menuitemradio')

  it('is closed until it is asked for, and lists the three modes with the keys beside them', async () => {
    const { user } = renderTable()
    expect(screen.queryByRole('menu')).toBeNull()
    expect(trigger()).toHaveAttribute('aria-haspopup', 'menu')
    await user.click(trigger())
    const list = await items()
    expect(list.map((item) => item.textContent)).toEqual([
      'Off',
      'Next window onlyCtrl',
      'Held until I turn it offCtrl+Shift',
    ])
    expect(list.map((item) => item.getAttribute('data-mode'))).toEqual(['Off', 'NextWindow', 'Held'])
    expect(list.map((item) => item.querySelector('kbd')?.textContent ?? null)).toEqual([
      null,
      'Ctrl',
      'Ctrl+Shift',
    ])
  })

  it.each(MODES)('checks the mode the server holds: %s', async (mode) => {
    const { user } = renderTable({ state: withMode(mode) })
    await user.click(trigger())
    const checked = (await items()).filter((item) => item.getAttribute('aria-checked') === 'true')
    expect(checked.map((item) => item.getAttribute('data-mode'))).toEqual([mode])
  })

  it.each([
    ['Off', 'Held', 'Off'],
    ['Next window only', 'Off', 'NextWindow'],
    ['Held until I turn it off', 'Off', 'Held'],
  ] satisfies [string, FullControlMode, FullControlMode][])(
    '"%s" asks for the mode it names',
    async (label, before, asked) => {
      const setFullControl = vi.fn((_mode: FullControlMode) => true)
      const { user } = renderTable({ session: { setFullControl }, state: withMode(before) })
      await user.click(trigger())
      await user.click(await screen.findByRole('menuitemradio', { name: new RegExp(`^${label}`) }))
      expect(setFullControl).toHaveBeenCalledExactlyOnceWith(asked)
      await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    },
  )

  it('lists the keys to a screen reader too', async () => {
    const { user } = renderTable()
    await user.click(trigger())
    const list = await items()
    expect(list.map((item) => item.getAttribute('aria-keyshortcuts'))).toEqual([
      null,
      'Control',
      'Control+Shift',
    ])
  })

  it('works from the keyboard alone and gives focus back to the trigger on Escape', async () => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user } = renderTable({ session: { setFullControl } })
    act(() => trigger().focus())
    await user.keyboard('{Enter}')
    await screen.findByRole('menu')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(setFullControl).not.toHaveBeenCalled()
    expect(trigger()).toHaveFocus()

    await user.keyboard('{Enter}')
    await screen.findByRole('menu')
    await user.keyboard('{ArrowDown}{ArrowDown}{Enter}')
    await waitFor(() => expect(setFullControl).toHaveBeenCalledTimes(1))
    expect(['NextWindow', 'Held']).toContain(setFullControl.mock.calls[0]?.[0])
  })
})

describe('without a connection', () => {
  it.each(['connecting', 'reconnecting', 'stopped'] as const)(
    'cannot be used while the socket is %s',
    async (status) => {
      const setFullControl = vi.fn((_mode: FullControlMode) => true)
      const { user } = renderTable({ session: { setFullControl }, state: (state) => ({ ...state, status }) })
      expect(must(toggle())).toHaveAttribute('aria-disabled', 'true')
      expect(trigger()).toBeDisabled()
      await user.click(must(toggle()))
      expect(setFullControl).not.toHaveBeenCalled()
    },
  )

  it('can be used while the socket is open', () => {
    renderTable()
    expect(must(toggle())).not.toHaveAttribute('aria-disabled')
    expect(trigger()).toBeEnabled()
  })
})

describe('the keys', () => {
  it('Control asks for the next window and Control with Shift for Held, from anywhere on the table', async () => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user } = renderTable({ session: { setFullControl } })
    await user.keyboard('{Control}')
    await user.keyboard('{Control>}{Shift}{/Control}')
    expect(setFullControl.mock.calls.map(([asked]) => asked)).toEqual(['NextWindow', 'Held'])
  })

  it('work with no decision pending, since Full Control is for waiting too', async () => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user, store } = renderTable({
      session: { setFullControl },
      state: (state) => ({ ...state, pending: null }),
    })
    expect(store.getState().pending).toBeNull()
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('NextWindow')
  })

  it('are left alone once the seat is out', async () => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user } = renderTable({
      session: { setFullControl },
      state: (state) => ({ ...state, phase: 'over' }),
    })
    await user.keyboard('{Control}{Control>}{Shift}{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })
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
    expect(screen.queryByRole('group', { name: 'Full Control' })).toBeNull()
  })
})

describe('over a live session', () => {
  function mount(mode: FullControlMode = 'Off') {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    const fixture = buildFixture({ seats: 2 })
    h.deliver({ ...welcome, full_control: mode })
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    return { h, ...renderLiveTable(h.session, fixtureLookup(fixture)) }
  }

  it('sends FullControl over the socket and reads on only when the server says so', async () => {
    const { h, user } = mount()
    await user.click(must(toggle()))
    expect(h.sentOf('FullControl')).toEqual([{ type: 'FullControl', mode: 'Held' }])
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')

    act(() => h.deliver({ type: 'FullControl', mode: 'Held' }))
    expect(must(toggle())).toHaveAccessibleName('Full Control: held')
    await user.click(must(toggle()))
    expect(h.sentOf('FullControl')).toEqual([
      { type: 'FullControl', mode: 'Held' },
      { type: 'FullControl', mode: 'Off' },
    ])
    act(() => h.deliver({ type: 'FullControl', mode: 'Off' }))
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')
  })

  it('sends the keys the same way, and shows the mode going off by itself after the next window', async () => {
    const { h, user } = mount()
    await user.keyboard('{Control}')
    expect(h.sentOf('FullControl')).toEqual([{ type: 'FullControl', mode: 'NextWindow' }])
    act(() => h.deliver({ type: 'FullControl', mode: 'NextWindow' }))
    expect(must(toggle())).toHaveAccessibleName('Full Control: next window')
    act(() => h.deliver({ type: 'FullControl', mode: 'Off' }))
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')
  })

  it.each(MODES)('shows the mode the server reports in a Welcome: %s', (mode) => {
    mount(mode)
    expect(must(toggle())).toHaveAccessibleName(
      `Full Control: ${mode === 'NextWindow' ? 'next window' : mode.toLowerCase()}`,
    )
  })

  it('goes back to what the server holds after a reconnect', async () => {
    const { h, user } = mount('Held')
    expect(must(toggle())).toHaveAccessibleName('Full Control: held')
    act(() => h.net.last.fail())
    expect(must(toggle())).toHaveAttribute('aria-disabled', 'true')
    await user.click(must(toggle()))
    expect(h.sentOf('FullControl')).toEqual([])
    act(() => h.timers.advance(250))
    act(() => {
      h.open()
      h.deliver({ ...welcome, full_control: 'Off' })
    })
    expect(must(toggle())).not.toHaveAttribute('aria-disabled')
    expect(must(toggle())).toHaveAccessibleName('Full Control: off')
  })
})
