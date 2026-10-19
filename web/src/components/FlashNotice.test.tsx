// Goal: a notice raised on one page shows under the shell's header on the next, announced as an alert, goes when it is dismissed or after 10 seconds, is replaced by a newer one (restarting the clock), and shows nothing while there is none.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clearFlash, flashStore, showFlash } from '@/lib/flash'
import { FLASH_MS, FlashNotice } from './FlashNotice'

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  act(() => clearFlash())
  vi.useRealTimers()
})

describe('the flash store', () => {
  it('holds one message, a newer one replaces it, and each show has its own id', () => {
    showFlash('first')
    const first = flashStore.getState()
    showFlash('first')
    expect(flashStore.getState().message).toBe('first')
    expect(flashStore.getState().id).toBe(first.id + 1)
    showFlash('second')
    expect(flashStore.getState().message).toBe('second')
    clearFlash()
    expect(flashStore.getState().message).toBeNull()
  })
})

describe('the flash notice', () => {
  it('shows nothing while there is no message', () => {
    render(<FlashNotice />)
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('shows a message raised before it mounted, as an alert with a Dismiss button', () => {
    showFlash('You left the game.')
    render(<FlashNotice />)
    expect(screen.getByRole('alert')).toHaveTextContent('You left the game.')
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeEnabled()
  })

  it('shows a message raised while it is mounted', () => {
    render(<FlashNotice />)
    act(() => showFlash('You closed the game.'))
    expect(screen.getByRole('alert')).toHaveTextContent('You closed the game.')
  })

  it('goes when it is dismissed', () => {
    showFlash('You left the game.')
    render(<FlashNotice />)
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('alert')).toBeNull()
    expect(flashStore.getState().message).toBeNull()
  })

  it('goes by itself after 10 seconds, and not before', () => {
    showFlash('You left the game.')
    render(<FlashNotice />)
    act(() => {
      vi.advanceTimersByTime(FLASH_MS - 1)
    })
    expect(screen.getByRole('alert')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('starts the clock again for a newer message, even the same words', () => {
    showFlash('You left the game.')
    render(<FlashNotice />)
    act(() => {
      vi.advanceTimersByTime(FLASH_MS - 1000)
    })
    act(() => showFlash('You left the game.'))
    act(() => {
      vi.advanceTimersByTime(FLASH_MS - 1000)
    })
    expect(screen.getByRole('alert')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(screen.queryByRole('alert')).toBeNull()
  })
})
