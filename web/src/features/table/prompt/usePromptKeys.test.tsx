/** Goal: a letter or digit hotkey fires for a plain press (any case) with the keyboard free or on a button, never while a field, menu or dialog has it, never with Ctrl, Alt or Meta or on a repeat; it follows the latest callbacks, stops when disabled or unmounted; and the default focus lands on its target unless the player is typing. */
import { render, renderHook } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { hotkeyOf, isTyping, useDefaultFocus, usePromptKeys } from './usePromptKeys'

const press = (init: KeyboardEventInit): KeyboardEvent =>
  new KeyboardEvent('keydown', { bubbles: true, ...init })

describe('usePromptKeys', () => {
  it.each(['y', 'Y'])('fires the key %s once for a plain press with nothing focused', async (key) => {
    const y = vi.fn()
    renderHook(() => usePromptKeys({ y }))
    await userEvent.setup().keyboard(key)
    expect(y).toHaveBeenCalledTimes(1)
  })

  it('fires with a button focused, since a button takes no letters', async () => {
    const one = vi.fn()
    const { container } = render(<button type="button">x</button>)
    renderHook(() => usePromptKeys({ '1': one }))
    container.querySelector('button')?.focus()
    await userEvent.setup().keyboard('1')
    expect(one).toHaveBeenCalledTimes(1)
  })

  it.each([
    ['a field', '<input aria-label="f" />', 'input'],
    ['a text area', '<textarea aria-label="f"></textarea>', 'textarea'],
    ['editable text', '<div contenteditable="true" tabindex="0">t</div>', '[contenteditable]'],
    ['a dialog', '<div role="dialog"><button type="button">b</button></div>', 'button'],
    ['a menu', '<div role="menu"><div role="menuitem" tabindex="0">i</div></div>', '[role="menuitem"]'],
  ])('leaves the keys to %s', async (_what, html, focus) => {
    const y = vi.fn()
    const { container } = render(<div dangerouslySetInnerHTML={{ __html: html }} />)
    renderHook(() => usePromptKeys({ y }))
    container.querySelector<HTMLElement>(focus)?.focus()
    await userEvent.setup().keyboard('y')
    expect(y).not.toHaveBeenCalled()
  })

  it.each([
    ['Ctrl', '{Control>}y{/Control}'],
    ['Alt', '{Alt>}y{/Alt}'],
    ['Meta', '{Meta>}y{/Meta}'],
    ['another key', 'z'],
    ['Enter', '{Enter}'],
  ])('ignores %s', async (_what, keys) => {
    const y = vi.fn()
    renderHook(() => usePromptKeys({ y }))
    await userEvent.setup().keyboard(keys)
    expect(y).not.toHaveBeenCalled()
  })

  it('ignores a key held down, which repeats', () => {
    const y = vi.fn()
    renderHook(() => usePromptKeys({ y }))
    document.dispatchEvent(press({ key: 'y' }))
    document.dispatchEvent(press({ key: 'y', repeat: true }))
    expect(y).toHaveBeenCalledTimes(1)
  })

  it('calls the callbacks of the latest render, and stops while disabled or after unmounting', async () => {
    const first = vi.fn()
    const second = vi.fn()
    const user = userEvent.setup()
    const { rerender, unmount } = renderHook(({ keys, enabled }) => usePromptKeys(keys, enabled), {
      initialProps: { keys: { y: first }, enabled: true },
    })
    rerender({ keys: { y: second }, enabled: true })
    await user.keyboard('y')
    expect([first.mock.calls.length, second.mock.calls.length]).toEqual([0, 1])
    rerender({ keys: { y: second }, enabled: false })
    await user.keyboard('y')
    expect(second).toHaveBeenCalledTimes(1)
    rerender({ keys: { y: second }, enabled: true })
    unmount()
    await user.keyboard('y')
    expect(second).toHaveBeenCalledTimes(1)
  })
})

describe('hotkeyOf and isTyping', () => {
  it.each([
    [{ key: 'Y' }, 'y'],
    [{ key: '3' }, '3'],
    [{ key: 'Y', shiftKey: true }, 'y'],
    [{ key: 'Escape' }, null],
    [{ key: 'y', ctrlKey: true }, null],
    [{ key: 'y', repeat: true }, null],
  ])('%j is %s', (init, expected) => {
    expect(hotkeyOf(press(init))).toBe(expected)
  })

  it('is not typing on nothing, a body or a button', () => {
    expect(isTyping(null)).toBe(false)
    expect(isTyping(document.body)).toBe(false)
    expect(isTyping(document.createElement('button'))).toBe(false)
  })
})

describe('useDefaultFocus', () => {
  function Probe() {
    const target = useRef<HTMLButtonElement>(null)
    useDefaultFocus(target)
    return (
      <button type="button" ref={target}>
        safe
      </button>
    )
  }

  it('focuses its target when the prompt appears', () => {
    const { getByRole } = render(<Probe />)
    expect(getByRole('button', { name: 'safe' })).toHaveFocus()
  })

  it('leaves the cursor where the player is typing', () => {
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    const { getByRole } = render(<Probe />)
    expect(field).toHaveFocus()
    expect(getByRole('button', { name: 'safe' })).not.toHaveFocus()
    field.remove()
  })
})
