// Goal: a tap of Control asks the session for the next window and Control with Shift for Full Control held (each again taking it back), decided from the mode the server holds, only when nothing has the keyboard and the keys were used for nothing else, and not at all when disabled or unmounted.
import { act, fireEvent, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { FullControlMode } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { useFullControlKeys } from './useFullControlKeys'

function Probe({ enabled = true }: { enabled?: boolean }) {
  useFullControlKeys(enabled)
  return (
    <>
      <input aria-label="A text field" />
      <textarea aria-label="A text area" />
      <button>A button</button>
      <div role="dialog" aria-label="A dialog">
        <span tabIndex={0}>inside the dialog</span>
      </div>
      <div role="menuitem" tabIndex={0}>
        A menu item
      </div>
    </>
  )
}

/** Listens only once its button has been pressed, and stops when it is pressed again. */
function Switched() {
  const [on, setOn] = useState(false)
  useFullControlKeys(on)
  return <button onClick={() => setOn((value) => !value)}>Switch the keys</button>
}

/** The Probe on a table, with the mode the server holds and a spy for what the session is asked. */
function mount(mode: FullControlMode = 'Off', enabled = true) {
  const setFullControl = vi.fn((_mode: FullControlMode) => true)
  const table = renderTable({
    ui: <Probe enabled={enabled} />,
    session: { setFullControl },
    state: (state) => ({ ...state, fullControl: mode }),
  })
  return { ...table, setFullControl }
}

const MODES: FullControlMode[] = ['Off', 'NextWindow', 'Held']

describe('Control tapped alone', () => {
  it.each([
    ['Off', ['NextWindow']],
    ['NextWindow', ['Off']],
    ['Held', []],
  ] satisfies [FullControlMode, FullControlMode[]][])('in %s asks for %j', async (mode, asked) => {
    const { setFullControl, user } = mount(mode)
    await user.keyboard('{Control}')
    expect(setFullControl.mock.calls.map(([sent]) => sent)).toEqual(asked)
  })

  it('asks when the key comes up, not when it goes down', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}')
    expect(setFullControl).not.toHaveBeenCalled()
    await user.keyboard('{/Control}')
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('NextWindow')
  })

  it('asks once for a key held down and repeating', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>4/}{/Control}')
    expect(setFullControl).toHaveBeenCalledTimes(1)
  })

  it('reads the mode the server holds each time, not the one from the first render', async () => {
    const { setFullControl, update, user } = mount('Off')
    await user.keyboard('{Control}')
    update(() => ({ fullControl: 'NextWindow' }))
    await user.keyboard('{Control}')
    update(() => ({ fullControl: 'Off' }))
    await user.keyboard('{Control}')
    expect(setFullControl.mock.calls.map(([sent]) => sent)).toEqual(['NextWindow', 'Off', 'NextWindow'])
  })
})

describe('Control and Shift together', () => {
  it.each([
    ['Control first, Shift tapped', '{Control>}{Shift}{/Control}'],
    ['Shift first, Control tapped', '{Shift>}{Control}{/Shift}'],
    ['both held, then let go', '{Control>}{Shift>}{/Shift}{/Control}'],
    ['both held, Control let go first', '{Control>}{Shift>}{/Control}{/Shift}'],
  ])('ask for Held with %s, from every mode but Held', async (_how, keys) => {
    for (const mode of MODES.filter((each) => each !== 'Held')) {
      const { setFullControl, user, unmount } = mount(mode)
      await user.keyboard(keys)
      expect(setFullControl).toHaveBeenCalledExactlyOnceWith('Held')
      unmount()
    }
  })

  it('ask for Off from Held', async () => {
    const { setFullControl, user } = mount('Held')
    await user.keyboard('{Control>}{Shift>}{/Shift}{/Control}')
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('Off')
  })

  it('ask once, however often Shift is pressed while Control stays down', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}{Shift}{Shift}{Shift}{/Control}')
    expect(setFullControl).toHaveBeenCalledTimes(1)
  })

  it('leave a tap of Control after them to be a tap', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}{Shift}{/Control}{Control}')
    expect(setFullControl.mock.calls.map(([sent]) => sent)).toEqual(['Held', 'NextWindow'])
  })
})

describe('the same keys used for anything else', () => {
  it.each([
    ['Shift alone', '{Shift}'],
    ['Space', ' '],
    ['Enter', '{Enter}'],
    ['Alt', '{Alt}'],
    ['Meta', '{Meta}'],
    ['Control with a letter', '{Control>}a{/Control}'],
    ['Control with Enter', '{Control>}{Enter}{/Control}'],
    ['Control and Shift with a letter (Ctrl+Shift+I)', '{Control>}{Shift>}i{/Shift}{/Control}'],
    ['Control with Alt', '{Alt>}{Control}{/Alt}'],
    ['Control with Meta', '{Meta>}{Control}{/Meta}'],
    ['Control and Shift with Alt', '{Control>}{Shift>}{Alt}{/Shift}{/Control}'],
  ])('ask for nothing: %s', async (_what, keys) => {
    const { setFullControl, user } = mount()
    await user.keyboard(keys)
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('ask for nothing when Control is used for a click', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}')
    await user.pointer({ keys: '[MouseLeft]', target: document.body })
    await user.keyboard('{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('ask for nothing when Control is used with the wheel (a zoom)', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}')
    fireEvent.wheel(document.body, { ctrlKey: true, deltaY: -100 })
    await user.keyboard('{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('ask for nothing when the window lost focus while Control was down', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}')
    act(() => {
      window.dispatchEvent(new Event('blur'))
    })
    await user.keyboard('{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('count a tap again after a gesture that was something else', async () => {
    const { setFullControl, user } = mount()
    await user.keyboard('{Control>}a{/Control}')
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('NextWindow')
  })
})

describe('what has the keyboard', () => {
  it.each([
    ['a text field', 'A text field'],
    ['a text area', 'A text area'],
  ])('leaves the keys to %s, so a line can be typed', async (_what, name) => {
    const { setFullControl, user } = mount()
    await user.click(screen.getByLabelText(name))
    await user.keyboard('{Control}{Control>}{Shift}{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('leaves the keys to a button, as Space and Enter do', async () => {
    const { setFullControl, user } = mount()
    act(() => screen.getByRole('button', { name: 'A button' }).focus())
    await user.keyboard('{Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it.each([
    ['inside a dialog', () => screen.getByText('inside the dialog')],
    ['on a menu item', () => screen.getByRole('menuitem', { name: 'A menu item' })],
  ])('leaves the keys alone %s', async (_where, find) => {
    const { setFullControl, user } = mount()
    act(() => find().focus())
    await user.keyboard('{Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('takes them again once focus has left the field', async () => {
    const { setFullControl, user } = mount()
    const field = screen.getByLabelText('A text field')
    await user.click(field)
    await user.keyboard('{Control}')
    expect(setFullControl).not.toHaveBeenCalled()
    act(() => field.blur())
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('NextWindow')
  })

  it('does not count a Control pressed in a field and let go outside it', async () => {
    const { setFullControl, user } = mount()
    const field = screen.getByLabelText('A text field')
    await user.click(field)
    await user.keyboard('{Control>}')
    act(() => field.blur())
    await user.keyboard('{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('leaves a key that something else already handled', async () => {
    const { setFullControl, user } = mount()
    const handled = (event: KeyboardEvent): void => event.preventDefault()
    window.addEventListener('keydown', handled, true)
    await user.keyboard('{Control}')
    window.removeEventListener('keydown', handled, true)
    expect(setFullControl).not.toHaveBeenCalled()
  })
})

describe('when it is off', () => {
  it('does nothing while disabled', async () => {
    const { setFullControl, user } = mount('Off', false)
    await user.keyboard('{Control}{Control>}{Shift}{/Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('stops listening when it unmounts', async () => {
    const { setFullControl, user, unmount } = mount()
    unmount()
    await user.keyboard('{Control}')
    expect(setFullControl).not.toHaveBeenCalled()
  })

  it('starts and stops listening as `enabled` changes', async () => {
    const setFullControl = vi.fn((_mode: FullControlMode) => true)
    const { user } = renderTable({ ui: <Switched />, session: { setFullControl } })
    const button = screen.getByRole('button', { name: 'Switch the keys' })
    await user.keyboard('{Control}')
    expect(setFullControl).not.toHaveBeenCalled()

    await user.click(button)
    act(() => button.blur())
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenCalledTimes(1)

    await user.click(button)
    act(() => button.blur())
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenCalledTimes(1)
  })
})
