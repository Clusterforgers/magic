// Goal: the handlers of a card show its preview on hover (mouse only) and focus and hide it on leaving, pin it on a click that only looks, pin nothing on a click the caller handles (an act: a card played, tapped or chosen), since a pinned preview covers part of the table, the prompt among it, until it is closed.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { createPreviewStore, PreviewContext } from './preview'
import type { PreviewStore } from './preview'
import { usePreviewHandlers } from './usePreviewHandlers'

function Probe({ act }: { act?: () => void }) {
  const handlers = usePreviewHandlers(7, act)
  return (
    <button type="button" {...handlers}>
      a card
    </button>
  )
}

function mount(act?: () => void): { store: PreviewStore; user: ReturnType<typeof userEvent.setup> } {
  const store = createPreviewStore()
  render(
    <PreviewContext value={store}>
      <Probe act={act} />
    </PreviewContext>,
  )
  return { store, user: userEvent.setup() }
}

const card = () => screen.getByRole('button', { name: 'a card' })

describe('a click that only looks', () => {
  it('pins the preview of the card, and a second click takes it away', async () => {
    const { store, user } = mount()
    await user.click(card())
    expect(store.get()).toMatchObject({ id: 7, pinned: true })
    await user.click(card())
    expect(store.get()).toBeNull()
  })

  it('pins on Enter and on Space too', async () => {
    const { store, user } = mount()
    card().focus()
    await user.keyboard('{Enter}')
    expect(store.get()?.pinned).toBe(true)
    await user.keyboard(' ')
    expect(store.get()).toBeNull()
  })
})

describe('a click that acts', () => {
  it('runs the act and pins nothing, however often it is pressed', async () => {
    const act = vi.fn()
    const { store, user } = mount(act)
    await user.click(card())
    await user.click(card())
    expect(act).toHaveBeenCalledTimes(2)
    expect(store.get()?.pinned).not.toBe(true)
  })

  it('does the same from the keyboard', async () => {
    const act = vi.fn()
    const { store, user } = mount(act)
    card().focus()
    await user.keyboard('{Enter} ')
    expect(act).toHaveBeenCalledTimes(2)
    expect(store.get()?.pinned).not.toBe(true)
  })
})

describe('hover and focus', () => {
  it('show the preview, unpinned, and leaving hides it', async () => {
    const { store, user } = mount()
    await user.hover(card())
    expect(store.get()).toMatchObject({ id: 7, pinned: false })
    await user.unhover(card())
    expect(store.get()).toBeNull()
    card().focus()
    expect(store.get()).toMatchObject({ id: 7, pinned: false })
    card().blur()
    expect(store.get()).toBeNull()
  })

  it('go on showing the card while it is acted on, until the pointer leaves', async () => {
    const { store, user } = mount(vi.fn())
    await user.click(card())
    expect(store.get()).toMatchObject({ id: 7, pinned: false })
    await user.unhover(card())
    expect(store.get()).toBeNull()
  })
})
