// Goal: the lexicon lists every term, narrows by search and by kind, opens a term named in another's text, and the help menu opens it and the tour.
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { TERMS } from '@/lib/glossary'
import { HelpMenu } from '../HelpMenu'
import { tourStore } from '../tour/tourStore'
import { LexiconDialog } from './LexiconDialog'
import { showLexicon } from './lexiconStore'

const entries = (): HTMLElement[] =>
  within(screen.getByRole('list', { name: 'Lexicon entries' })).getAllByRole('listitem')

describe('LexiconDialog', () => {
  it('lists every term while the search is empty', () => {
    render(<LexiconDialog />)
    act(() => showLexicon())
    expect(entries()).toHaveLength(TERMS.length)
  })

  it.each([
    ['Keywords', 'keyword'],
    ['Actions', 'action'],
    ['Ability words', 'ability word'],
    ['Rules', 'rule'],
  ])('shows only the %s', async (label, kind) => {
    const user = userEvent.setup()
    render(<LexiconDialog />)
    act(() => showLexicon())
    const button = screen.getByRole('button', { name: new RegExp(`^${label}`) })
    await user.click(button)
    expect(button).toHaveAttribute('aria-pressed', 'true')
    expect(entries()).toHaveLength(TERMS.filter((term) => term.kind === kind).length)
  })

  it.each([
    ['ninjutsu', 'Ninjutsu'],
    ['upkeep', 'Upkeep'],
    ['command zone', 'Command zone'],
    ['replacement', 'Replacement effect'],
    ['ring-bearer', 'Ring-bearer'],
    ['the ring tempts you', 'The Ring tempts you'],
    ['city', "City's blessing"],
    ['rad counter', 'Rad counter'],
  ])('puts %s first for a search', async (query, title) => {
    const user = userEvent.setup()
    render(<LexiconDialog />)
    act(() => showLexicon())
    await user.type(screen.getByRole('searchbox', { name: 'Search the lexicon' }), query)
    expect(entries()[0]).toHaveTextContent(title)
  })

  it('files the Ring under its kinds: the action, and the Ring-bearer and its abilities as rules', async () => {
    const user = userEvent.setup()
    render(<LexiconDialog />)
    act(() => showLexicon('ring'))
    const titles = (): string[] =>
      entries().map((entry) => entry.querySelector('span.font-medium')?.textContent ?? '')
    expect(titles()).toEqual(
      expect.arrayContaining([
        'The Ring tempts you',
        'Ring-bearer',
        'Ring ability 1: legendary, evasive',
        'Ring ability 2: loot',
        'Ring ability 3: sacrifice the blocker',
        'Ring ability 4: drain',
      ]),
    )
    await user.click(screen.getByRole('button', { name: /^Actions/ }))
    expect(titles()).toContain('The Ring tempts you')
    expect(titles()).not.toContain('Ring-bearer')
    await user.click(screen.getByRole('button', { name: /^Rules/ }))
    expect(titles()).toContain('Ring-bearer')
    expect(entries().find((entry) => entry.textContent?.includes('Ring-bearer'))).toHaveTextContent(
      'rule, players',
    )
  })

  it('opens the term another entry names', async () => {
    const user = userEvent.setup()
    render(<LexiconDialog />)
    act(() => showLexicon('Ending phase'))
    const first = entries()[0]
    expect(first).toHaveTextContent('Ending phase')
    await user.click(within(first as HTMLElement).getAllByRole('button')[0] as HTMLElement)
    expect(screen.getByRole('searchbox', { name: 'Search the lexicon' })).toHaveValue('End step')
    expect(entries()[0]).toHaveTextContent('End step')
  })

  it('says so when nothing matches', async () => {
    const user = userEvent.setup()
    render(<LexiconDialog />)
    act(() => showLexicon())
    await user.type(screen.getByRole('searchbox', { name: 'Search the lexicon' }), 'zzzzqq')
    expect(screen.getByText('Nothing matches.')).toBeInTheDocument()
  })
})

describe('HelpMenu', () => {
  it('opens the lexicon', async () => {
    const user = userEvent.setup()
    render(
      <>
        <HelpMenu />
        <LexiconDialog />
      </>,
    )
    await user.click(screen.getByRole('button', { name: 'Help' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Rules lexicon' }))
    expect(await screen.findByRole('dialog', { name: 'Rules lexicon' })).toBeInTheDocument()
  })

  it('starts the tour', async () => {
    const user = userEvent.setup()
    render(<HelpMenu />)
    await user.click(screen.getByRole('button', { name: 'Help' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Table tour' }))
    expect(tourStore.getState().step).toBe(0)
  })
})
