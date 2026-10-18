// Goal: a plain Space or Enter that nothing has focus for presses the primary button once, and every key that belongs to a field, a control, a menu or a dialog, or that carries a modifier, is left alone.
import { render, renderHook } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { isFocusFree, isPrimaryKey, usePrimaryKey } from './usePrimaryKey'

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
    [{ key: ' ' }, true],
    [{ key: 'Enter' }, true],
    [{ key: ' ', repeat: true }, false],
    [{ key: 'Enter', repeat: true }, false],
    [{ key: ' ', ctrlKey: true }, false],
    [{ key: ' ', altKey: true }, false],
    [{ key: ' ', metaKey: true }, false],
    [{ key: ' ', shiftKey: true }, false],
    [{ key: 'Enter', shiftKey: true }, false],
    [{ key: 'Escape' }, false],
    [{ key: 'a' }, false],
    [{ key: 'ArrowRight' }, false],
    [{ key: 'Tab' }, false],
  ])('%j counts: %s', (init, counts) => {
    expect(isPrimaryKey(key(init))).toBe(counts)
  })
})

describe('the shortcut', () => {
  it.each([
    ['Space', ' '],
    ['Enter', '{Enter}'],
  ])('%s presses the primary button when nothing is focused', async (_name, keys) => {
    const press = vi.fn()
    renderHook(() => usePrimaryKey(press, true))
    await userEvent.setup().keyboard(keys)
    expect(press).toHaveBeenCalledTimes(1)
  })

  it('presses it once for a key held down', async () => {
    const press = vi.fn()
    renderHook(() => usePrimaryKey(press, true))
    await userEvent.setup().keyboard('{Enter>3/}')
    expect(press).toHaveBeenCalledTimes(1)
  })

  it('stops the page from scrolling for the Space it takes, and leaves every other key alone', async () => {
    renderHook(() => usePrimaryKey(vi.fn(), true))
    const seen: boolean[] = []
    const watch = (event: KeyboardEvent): void => {
      seen.push(event.defaultPrevented)
    }
    // Registered after the hook's listener, so it sees what the hook did to the event.
    document.addEventListener('keydown', watch)
    await userEvent.setup().keyboard(' a')
    document.removeEventListener('keydown', watch)
    expect(seen).toEqual([true, false])
  })

  it.each([
    ['a text field', '<input type="text">'],
    ['a text area', '<textarea></textarea>'],
    ['a button', '<button>Concede</button>'],
    ['a link', '<a href="#x">x</a>'],
    ['a menu item', '<div role="menuitem" tabindex="0">x</div>'],
  ])('leaves the keys to %s when it has focus', async (_name, html) => {
    const press = vi.fn()
    renderHook(() => usePrimaryKey(press, true))
    const user = userEvent.setup()
    const element = make(html)
    element.focus()
    expect(document.activeElement).toBe(element)
    await user.keyboard(' {Enter}')
    expect(press).not.toHaveBeenCalled()
  })

  it('does not take Space typed into a text field, so a chat line keeps its spaces', async () => {
    const press = vi.fn()
    renderHook(() => usePrimaryKey(press, true))
    const user = userEvent.setup()
    const field = make('<input type="text">')
    await user.click(field)
    await user.keyboard('good game')
    expect((field as HTMLInputElement).value).toBe('good game')
    expect(press).not.toHaveBeenCalled()
  })

  it('does nothing while it is disabled, and listens again once it is enabled', async () => {
    const press = vi.fn()
    const { rerender } = renderHook(({ on }) => usePrimaryKey(press, on), { initialProps: { on: false } })
    const user = userEvent.setup()
    await user.keyboard(' ')
    expect(press).not.toHaveBeenCalled()
    rerender({ on: true })
    await user.keyboard(' ')
    expect(press).toHaveBeenCalledTimes(1)
    rerender({ on: false })
    await user.keyboard(' ')
    expect(press).toHaveBeenCalledTimes(1)
  })

  it('calls the newest function it was given, not the one from the first render', async () => {
    const first = vi.fn()
    const second = vi.fn()
    const { rerender } = renderHook(({ press }) => usePrimaryKey(press, true), {
      initialProps: { press: first },
    })
    rerender({ press: second })
    await userEvent.setup().keyboard(' ')
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledTimes(1)
  })

  it('stops listening when it unmounts', async () => {
    const press = vi.fn()
    const { unmount } = renderHook(() => usePrimaryKey(press, true))
    unmount()
    await userEvent.setup().keyboard(' {Enter}')
    expect(press).not.toHaveBeenCalled()
  })

  it('leaves a key that something else already handled', async () => {
    const press = vi.fn()
    renderHook(() => usePrimaryKey(press, true))
    const handled = (event: KeyboardEvent): void => event.preventDefault()
    // Registered first on the window, which the document hears after: the event arrives marked as handled.
    window.addEventListener('keydown', handled, true)
    await userEvent.setup().keyboard(' ')
    window.removeEventListener('keydown', handled, true)
    expect(press).not.toHaveBeenCalled()
  })

  it('works with the page of a real render too', async () => {
    const press = vi.fn()
    function Page() {
      usePrimaryKey(press, true)
      return <button>Elsewhere</button>
    }
    const view = render(<Page />)
    const user = userEvent.setup()
    await user.keyboard(' ')
    expect(press).toHaveBeenCalledTimes(1)
    await user.tab()
    expect(view.getByRole('button', { name: 'Elsewhere' })).toHaveFocus()
    await user.keyboard(' ')
    expect(press).toHaveBeenCalledTimes(1)
  })
})
