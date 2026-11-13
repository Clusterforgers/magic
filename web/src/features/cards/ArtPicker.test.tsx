// Goal: the art picker lists a card's printings newest first with lazy pictures by printing id, filters them by set, moves focus with the arrow keys, chooses with a click or a key and closes once the choice is saved, offers Default, and says when the list does not load.
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ArtPicker } from './ArtPicker'
import { printingView, startPrintings } from './fakePrintings'

const PRINTINGS = [
  printingView('p1', 'sld', { set_name: 'Secret Lair Drop' }),
  printingView('p2', 'm10', { set_name: 'Magic 2010' }),
  printingView('p3', 'lea', { set_name: 'Limited Edition Alpha' }),
]

function open(chosen: string | null = null, api = startPrintings({ bolt: PRINTINGS }).api) {
  const onChoose = vi.fn((_printing: string | null, _everywhere: boolean) => Promise.resolve())
  const onClose = vi.fn()
  render(
    <ArtPicker
      target={{ oracleId: 'bolt', name: 'Lightning Bolt', chosen }}
      api={api}
      onChoose={onChoose}
      onClose={onClose}
    />,
  )
  return { onChoose, onClose, user: userEvent.setup() }
}

const options = () => within(screen.getByRole('list', { name: 'Printings' })).getAllByRole('button')

describe('the art picker', () => {
  it('lists the printings in the order given, each drawn by its printing id, the chosen one pressed', async () => {
    open('p2')
    await screen.findByRole('list', { name: 'Printings' })
    expect(options().map((b) => b.getAttribute('data-printing'))).toEqual(['p1', 'p2', 'p3'])
    expect(options()[1]).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Default' })).toHaveAttribute('aria-pressed', 'false')
    const img = options()[0]?.querySelector('img')
    expect(img?.getAttribute('src') ?? img?.getAttribute('data-src') ?? '').toContain(
      '/api/images/printing/p1/0/small',
    )
  })

  it('filters by set code or name', async () => {
    const { user } = open()
    await screen.findByRole('list', { name: 'Printings' })
    await user.type(screen.getByRole('searchbox', { name: 'Filter by set' }), 'alpha')
    expect(options().map((b) => b.getAttribute('data-printing'))).toEqual(['p3'])
    await user.clear(screen.getByRole('searchbox', { name: 'Filter by set' }))
    await user.type(screen.getByRole('searchbox', { name: 'Filter by set' }), 'nothing')
    expect(screen.getByRole('status')).toHaveTextContent('No printing of a set like “nothing”.')
  })

  it('moves focus with the arrow keys and chooses with Enter, then closes', async () => {
    const { user, onChoose, onClose } = open()
    await screen.findByRole('list', { name: 'Printings' })
    options()[0]?.focus()
    await user.keyboard('{ArrowRight}')
    expect(options()[1]).toHaveFocus()
    await user.keyboard('{End}')
    expect(options()[2]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(onChoose).toHaveBeenCalledWith('p3', false)
    expect(onClose).toHaveBeenCalled()
  })

  it('resets to the default art', async () => {
    const { user, onChoose } = open('p1')
    await user.click(screen.getByRole('button', { name: 'Default' }))
    expect(onChoose).toHaveBeenCalledWith(null, false)
  })

  it('says when the printings do not load', async () => {
    const api = { ...startPrintings({}).api, printingsOf: () => Promise.reject(new Error('No connection.')) }
    open(null, api)
    expect(await screen.findByRole('alert')).toHaveTextContent('No connection.')
  })
})
