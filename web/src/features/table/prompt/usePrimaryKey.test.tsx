// Goal: a plain Space or Enter that nothing has focus for runs its own function once (Space and Enter apart), and every key that belongs to a field, a control, a menu or a dialog, or that carries a modifier, is left alone.
import { render, renderHook } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { isFocusFree, primaryKeyOf, usePrimaryKey } from './usePrimaryKey'
import type { PrimaryKeys } from './usePrimaryKey'

afterEach(() => {
  document.body.innerHTML = ''
})

/** Puts `html` in the page and returns the element it made. */
function make(html: string): HTMLElement {
  document.body.innerHTML = html
  const element = document.body.firstElementChild
  if (!(element instanceof HTMLElement)) throw new Error('no element was made')
  return element
}

describe('what has the keyboard', () => {
  it.each([
    ['nothing at all', null],
    ['the page', document.body],
    ['the document element', document.documentElement],
  ])('%s leaves it to the table', (_what, target) => {
    expect(isFocusFree(target)).toBe(true)
  })

  it.each([
    ['a heading with a tabindex', '<h2 tabindex="-1">Turn 1</h2>', true],
    ['a plain block', '<div>text</div>', true],
    ['an element made editable then switched off', '<div contenteditable="false">x</div>', true],
    ['a button', '<button>Pass</button>', false],
    ['a link', '<a href="#x">x</a>', false],
    ['a link without an address', '<a>x</a>', true],
    ['a text field', '<input type="text">', false],
    ['a checkbox', '<input type="checkbox">', false],
    ['a text area', '<textarea></textarea>', false],
    ['a select', '<select><option>a</option></select>', false],
    ['a summary', '<summary>more</summary>', false],
    ['an editable block', '<div contenteditable="true">x</div>', false],
    ['a block with a button role', '<div role="button">x</div>', false],
    ['a menu item', '<div role="menuitem">x</div>', false],
    ['a switch', '<div role="switch">x</div>', false],
    ['a tab', '<div role="tab">x</div>', false],
    ['a dialog', '<div role="dialog">x</div>', false],
    ['an alert dialog', '<div role="alertdialog">x</div>', false],
  ])('%s: free is %s', (_what, html, free) => {
    expect(isFocusFree(make(html))).toBe(free)
  })

  it('is not free inside a control, a dialog or a menu', () => {
    const inner = make('<div role="dialog"><p><span>deep</span></p></div>').querySelector('span')
    expect(isFocusFree(inner)).toBe(false)
    const label = make('<button><span>Next</span></button>').querySelector('span')
    expect(isFocusFree(label)).toBe(false)
  })
})

describe('which keys count', () => {
  const key = (init: KeyboardEventInit): KeyboardEvent => new KeyboardEvent('keydown', init)

  it.each([
    [{ key: ' ' }, 'space'],
    [{ key: 'Enter' }, 'enter'],
    [{ key: ' ', repeat: true }, null],
    [{ key: 'Enter', repeat: true }, null],
    [{ key: ' ', ctrlKey: true }, null],
    [{ key: ' ', altKey: true }, null],
    [{ key: ' ', metaKey: true }, null],
    [{ key: ' ', shiftKey: true }, null],
    [{ key: 'Enter', ctrlKey: true }, null],
    [{ key: 'Enter', altKey: true }, null],
    [{ key: 'Enter', metaKey: true }, null],
    [{ key: 'Enter', shiftKey: true }, null],
    [{ key: 'Escape' }, null],
    [{ key: 'a' }, null],
    [{ key: 'ArrowRight' }, null],
    [{ key: 'Tab' }, null],
    [{ key: 'Control' }, null],
    [{ key: 'Shift' }, null],
  ])('%j is %s', (init, expected) => {
    expect(primaryKeyOf(key(init))).toBe(expected)
  })
})

/** Two spies, one for each key. */
function spies() {
  const space = vi.fn()
  const enter = vi.fn()
  const keys: PrimaryKeys = { space, enter }
  return { space, enter, keys }
}

describe('the shortcut', () => {
  it.each([
    ['Space', ' ', 'space'],
    ['Enter', '{Enter}', 'enter'],
  ] as const)(
    '%s runs its own function and not the other, when nothing is focused',
    async (_name, keys, which) => {
      const { keys: handlers, space, enter } = spies()
      renderHook(() => usePrimaryKey(handlers, true))
      await userEvent.setup().keyboard(keys)
      expect(which === 'space' ? space : enter).toHaveBeenCalledTimes(1)
      expect(which === 'space' ? enter : space).not.toHaveBeenCalled()
    },
  )

  it('runs each once for a key held down', async () => {
    const { keys, space, enter } = spies()
    renderHook(() => usePrimaryKey(keys, true))
    await userEvent.setup().keyboard('{Enter>3/}{ >3/}')
    expect(enter).toHaveBeenCalledTimes(1)
    expect(space).toHaveBeenCalledTimes(1)
  })

  it('stops the page from scrolling for the Space it takes and the Enter it takes, and leaves every other key alone', async () => {
    renderHook(() => usePrimaryKey(spies().keys, true))
    const seen: boolean[] = []
    const watch = (event: KeyboardEvent): void => {
      seen.push(event.defaultPrevented)
    }
    // Registered after the hook's listener, so it sees what the hook did to the event.
    document.addEventListener('keydown', watch)
    await userEvent.setup().keyboard(' a{Enter}')
    document.removeEventListener('keydown', watch)
    expect(seen).toEqual([true, false, true])
  })

  it.each([
    ['a text field', '<input type="text">'],
    ['a text area', '<textarea></textarea>'],
    ['a button', '<button>Concede</button>'],
    ['a link', '<a href="#x">x</a>'],
    ['a menu item', '<div role="menuitem" tabindex="0">x</div>'],
  ])('leaves the keys to %s when it has focus', async (_name, html) => {
    const { keys, space, enter } = spies()
    renderHook(() => usePrimaryKey(keys, true))
    const user = userEvent.setup()
    const element = make(html)
    element.focus()
    expect(document.activeElement).toBe(element)
    await user.keyboard(' {Enter}')
    expect(space).not.toHaveBeenCalled()
    expect(enter).not.toHaveBeenCalled()
  })

  it('does not take Space typed into a text field, so a chat line keeps its spaces', async () => {
    const { keys, space, enter } = spies()
    renderHook(() => usePrimaryKey(keys, true))
    const user = userEvent.setup()
    const field = make('<input type="text">')
    await user.click(field)
    await user.keyboard('good game')
    expect((field as HTMLInputElement).value).toBe('good game')
    expect(space).not.toHaveBeenCalled()
    expect(enter).not.toHaveBeenCalled()
  })

  it.each([
    ['Control', '{Control>}{key}{/Control}'],
    ['Alt', '{Alt>}{key}{/Alt}'],
    ['Meta', '{Meta>}{key}{/Meta}'],
    ['Shift', '{Shift>}{key}{/Shift}'],
  ])(
    'ignores both keys with %s held, so Shift+Enter is not the end-of-turn yield',
    async (_name, pattern) => {
      const { keys, space, enter } = spies()
      renderHook(() => usePrimaryKey(keys, true))
      const user = userEvent.setup()
      await user.keyboard(pattern.replace('{key}', ' '))
      await user.keyboard(pattern.replace('{key}', '{Enter}'))
      expect(space).not.toHaveBeenCalled()
      expect(enter).not.toHaveBeenCalled()
    },
  )

  it('does nothing while it is disabled, and listens again once it is enabled', async () => {
    const { keys, space, enter } = spies()
    const { rerender } = renderHook(({ on }) => usePrimaryKey(keys, on), { initialProps: { on: false } })
    const user = userEvent.setup()
    await user.keyboard(' {Enter}')
    expect(space).not.toHaveBeenCalled()
    expect(enter).not.toHaveBeenCalled()
    rerender({ on: true })
    await user.keyboard(' {Enter}')
    expect(space).toHaveBeenCalledTimes(1)
    expect(enter).toHaveBeenCalledTimes(1)
    rerender({ on: false })
    await user.keyboard(' {Enter}')
    expect(space).toHaveBeenCalledTimes(1)
    expect(enter).toHaveBeenCalledTimes(1)
  })

  it('calls the newest functions it was given, not the ones from the first render', async () => {
    const first = spies()
    const second = spies()
    const { rerender } = renderHook(({ keys }) => usePrimaryKey(keys, true), {
      initialProps: { keys: first.keys },
    })
    rerender({ keys: second.keys })
    await userEvent.setup().keyboard(' {Enter}')
    expect(first.space).not.toHaveBeenCalled()
    expect(first.enter).not.toHaveBeenCalled()
    expect(second.space).toHaveBeenCalledTimes(1)
    expect(second.enter).toHaveBeenCalledTimes(1)
  })

  it('stops listening when it unmounts', async () => {
    const { keys, space, enter } = spies()
    const { unmount } = renderHook(() => usePrimaryKey(keys, true))
    unmount()
    await userEvent.setup().keyboard(' {Enter}')
    expect(space).not.toHaveBeenCalled()
    expect(enter).not.toHaveBeenCalled()
  })

  it('leaves a key that something else already handled', async () => {
    const { keys, space } = spies()
    renderHook(() => usePrimaryKey(keys, true))
    const handled = (event: KeyboardEvent): void => event.preventDefault()
    // Registered first on the window, which the document hears after: the event arrives marked as handled.
    window.addEventListener('keydown', handled, true)
    await userEvent.setup().keyboard(' ')
    window.removeEventListener('keydown', handled, true)
    expect(space).not.toHaveBeenCalled()
  })

  it('works with the page of a real render too', async () => {
    const { keys, space, enter } = spies()
    function Page() {
      usePrimaryKey(keys, true)
      return <button>Elsewhere</button>
    }
    const view = render(<Page />)
    const user = userEvent.setup()
    await user.keyboard(' {Enter}')
    expect(space).toHaveBeenCalledTimes(1)
    expect(enter).toHaveBeenCalledTimes(1)
    await user.tab()
    expect(view.getByRole('button', { name: 'Elsewhere' })).toHaveFocus()
    await user.keyboard(' {Enter}')
    expect(space).toHaveBeenCalledTimes(1)
    expect(enter).toHaveBeenCalledTimes(1)
  })
})
