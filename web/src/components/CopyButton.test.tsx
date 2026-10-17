// Goal: a copy button puts its text on the clipboard and says so on screen and to screen readers, works from the keyboard, falls back to the legacy copy path, and says plainly when the browser refuses both.
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { CopyButton } from './CopyButton'

/** Replaces `navigator.clipboard`; call it after `userEvent.setup()`, which installs a stub of its own. */
function stubClipboard(value: Pick<Clipboard, 'writeText'> | undefined): void {
  Object.defineProperty(navigator, 'clipboard', { value, configurable: true, writable: true })
}

function stubExecCommand(result: boolean): ReturnType<typeof vi.fn> {
  const execCommand = vi.fn(() => result)
  Object.defineProperty(document, 'execCommand', { value: execCommand, configurable: true, writable: true })
  return execCommand
}

afterEach(() => {
  Reflect.deleteProperty(navigator, 'clipboard')
  Reflect.deleteProperty(document, 'execCommand')
  vi.useRealTimers()
})

describe('CopyButton', () => {
  it('copies its text and shows and announces that it did', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn(() => Promise.resolve())
    stubClipboard({ writeText })
    render(<CopyButton text="https://play.example/game/g#seat=0&token=t" label="Copy link for seat 1" />)
    await user.click(screen.getByRole('button', { name: 'Copy link for seat 1' }))
    expect(writeText).toHaveBeenCalledExactlyOnceWith('https://play.example/game/g#seat=0&token=t')
    expect(screen.getByRole('button')).toHaveTextContent('Copied')
    expect(screen.getByRole('status')).toHaveTextContent('Copied to the clipboard.')
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('is reached with Tab and pressed with %s', async (_name, key) => {
    const user = userEvent.setup()
    const writeText = vi.fn(() => Promise.resolve())
    stubClipboard({ writeText })
    render(<CopyButton text="hello" label="Copy greeting" />)
    await user.tab()
    expect(screen.getByRole('button', { name: 'Copy greeting' })).toHaveFocus()
    await user.keyboard(key)
    expect(writeText).toHaveBeenCalledExactlyOnceWith('hello')
  })

  it('falls back to the legacy copy when the clipboard API refuses', async () => {
    const user = userEvent.setup()
    stubClipboard({ writeText: () => Promise.reject(new DOMException('denied', 'NotAllowedError')) })
    const execCommand = stubExecCommand(true)
    render(<CopyButton text="hello" label="Copy greeting" />)
    await user.click(screen.getByRole('button', { name: 'Copy greeting' }))
    expect(execCommand).toHaveBeenCalledWith('copy')
    expect(screen.getByRole('button')).toHaveTextContent('Copied')
    expect(document.querySelector('textarea')).toBeNull()
  })

  it('falls back to the legacy copy when there is no clipboard API, as over plain http', async () => {
    const user = userEvent.setup()
    stubClipboard(undefined)
    const execCommand = stubExecCommand(true)
    render(<CopyButton text="hello" label="Copy greeting" />)
    await user.click(screen.getByRole('button', { name: 'Copy greeting' }))
    expect(execCommand).toHaveBeenCalledWith('copy')
    expect(screen.getByRole('button')).toHaveTextContent('Copied')
  })

  it('says it failed, and how to carry on, when the browser refuses every way', async () => {
    const user = userEvent.setup()
    stubClipboard(undefined)
    stubExecCommand(false)
    render(<CopyButton text="hello" label="Copy greeting" />)
    await user.click(screen.getByRole('button', { name: 'Copy greeting' }))
    expect(screen.getByRole('button')).toHaveTextContent('Copy failed')
    expect(screen.getByRole('status')).toHaveTextContent('Select the link and copy it by hand.')
  })

  it('says it failed when even the legacy path throws', async () => {
    const user = userEvent.setup()
    stubClipboard(undefined)
    Object.defineProperty(document, 'execCommand', {
      value: () => {
        throw new Error('not allowed')
      },
      configurable: true,
    })
    render(<CopyButton text="hello" label="Copy greeting" />)
    await user.click(screen.getByRole('button', { name: 'Copy greeting' }))
    expect(screen.getByRole('button')).toHaveTextContent('Copy failed')
  })

  it('goes back to its label after a moment', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) })
    stubClipboard({ writeText: () => Promise.resolve() })
    render(<CopyButton text="hello" label="Copy greeting" />)
    await user.click(screen.getByRole('button', { name: 'Copy greeting' }))
    expect(screen.getByRole('button')).toHaveTextContent('Copied')
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000)
    })
    expect(screen.getByRole('button')).toHaveTextContent(/^Copy$/)
  })
})
