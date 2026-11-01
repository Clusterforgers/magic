// Goal: a keyword word in a text shows its reminder on press, text without keywords is left as it is, and the glossary button of the top bar opens a list that a search narrows.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GlossaryPanel } from './GlossaryPanel'
import { KeywordText } from './Keyword'

describe('KeywordText', () => {
  it('keeps the words of a text whole and marks its keywords', () => {
    const { container } = render(<KeywordText text="Ninjutsu {1}{U}, then Discover 4." />)
    expect(container.textContent).toBe('Ninjutsu {1}{U}, then Discover 4.')
    expect([...container.querySelectorAll('[data-slot="keyword-word"]')].map((n) => n.textContent)).toEqual([
      'Ninjutsu',
      'Discover',
    ])
  })

  it('marks nothing in a text without keywords', () => {
    const { container } = render(<KeywordText text="Draw a card." />)
    expect(container.querySelector('[data-slot="keyword-word"]')).toBeNull()
  })

  it('shows the reminder of a keyword on press', async () => {
    const user = userEvent.setup()
    render(<KeywordText text="Discover 4." />)
    await user.click(screen.getByRole('button', { name: 'Discover' }))
    expect(
      await screen.findByText(/Exile cards from the top of your library until you exile a nonland card/),
    ).toBeInTheDocument()
  })
})

describe('GlossaryPanel', () => {
  it('lists every term and narrows to the ones a search finds', async () => {
    const user = userEvent.setup()
    render(<GlossaryPanel />)
    await user.click(screen.getByRole('button', { name: 'Keyword glossary' }))
    const all = (await screen.findAllByRole('listitem')).length
    expect(all).toBeGreaterThan(300)
    await user.type(screen.getByRole('searchbox', { name: 'Search keywords' }), 'ninjutsu')
    const items = screen.getAllByRole('listitem')
    expect(items.length).toBeLessThan(all)
    expect(items[0]).toHaveTextContent('Ninjutsu')
  })

  it('says so when nothing matches', async () => {
    const user = userEvent.setup()
    render(<GlossaryPanel />)
    await user.click(screen.getByRole('button', { name: 'Keyword glossary' }))
    await user.type(await screen.findByRole('searchbox', { name: 'Search keywords' }), 'zzzzqq')
    expect(screen.getByText('Nothing matches.')).toBeInTheDocument()
  })
})
