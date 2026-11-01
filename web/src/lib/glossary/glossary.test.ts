// Goal: every keyword of the engine's list and of Scryfall's keyword catalogue (abilities, actions and ability words) has a reminder, and keyword words are found in card text, log lines and the glossary search.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import catalog from './catalog.json'
import { mentions, pieces, searchTerms, termFor, TERMS } from './index'

const RUST = resolve(process.cwd(), '../crates/mtg-cards/src/keyword.rs')

/** The `Keyword::NAMES` list of the engine's keyword file. */
function engineNames(): string[] {
  const source = readFileSync(RUST, 'utf8')
  const body = source.split('pub const NAMES: &[&str] = &[')[1]?.split('];')[0] ?? ''
  return [...body.matchAll(/"([^"]+)"/g)].map((m) => m[1] ?? '')
}

describe('the glossary covers the engine and Scryfall', () => {
  const engine = engineNames()

  it('reads the engine list', () => {
    expect(engine.length).toBeGreaterThan(100)
  })

  it.each(engine.map((name) => [name]))('has the engine keyword %s', (name) => {
    expect(termFor(name)?.text.length ?? 0).toBeGreaterThan(10)
  })

  it.each([
    ...catalog.keywordAbilities.map((name) => [name, 'keyword ability']),
    ...catalog.keywordActions.map((name) => [name, 'keyword action']),
    ...catalog.abilityWords.map((name) => [name, 'ability word']),
  ])('has %s (%s)', (name) => {
    expect(termFor(name as string)).toBeDefined()
  })

  it('writes every reminder as a sentence and never twice for a spelling', () => {
    const seen = new Set<string>()
    for (const term of TERMS) {
      expect(term.text).toMatch(/[.!)"]$/)
      for (const name of term.names) {
        expect(seen.has(name.toLowerCase()), name).toBe(false)
        seen.add(name.toLowerCase())
      }
    }
  })
})

describe('termFor', () => {
  it.each([
    ['ward', 'Ward'],
    ['Ward {2}', 'Ward'],
    ['First Strike', 'First strike'],
    ['Plainscycling', 'Cycling'],
    ['Jump start', 'Jump-start'],
  ])('finds %s', (spelling, title) => {
    expect(termFor(spelling)?.title).toBe(title)
  })

  it('knows nothing of a made-up word', () => {
    expect(termFor('Frobnicate')).toBeUndefined()
  })
})

describe('pieces', () => {
  it('marks whole keyword words and leaves the rest', () => {
    const marked = pieces(
      'Ninjutsu {1}{U} ({1}{U}, Return an unblocked attacker you control to hand: Discover 4.)',
    )
    expect(marked.filter((p) => p.term).map((p) => p.text)).toEqual(['Ninjutsu', 'Discover'])
    expect(marked.map((p) => p.text).join('')).toBe(
      'Ninjutsu {1}{U} ({1}{U}, Return an unblocked attacker you control to hand: Discover 4.)',
    )
  })

  it.each([['Unflyingly'], ['Wardrobe'], ['Bashing the cascades']])('does not mark a part of %s', (text) => {
    expect(mentions(text)).toEqual([])
  })

  it('does not mark everyday verbs', () => {
    expect(mentions('Destroy target creature. Counter that spell. Tap it.')).toEqual([])
  })

  it('lists each mentioned term once, longest name first', () => {
    expect(
      mentions('Partner with Kraum. Partner with Tevesh; first strike, first strike.').map((t) => t.title),
    ).toEqual(['Partner with', 'First strike'])
  })
})

describe('searchTerms', () => {
  it('lists everything for an empty query', () => {
    expect(searchTerms('  ')).toHaveLength(TERMS.length)
  })

  it('puts spellings before reminder text matches', () => {
    const found = searchTerms('ward')
    expect(found[0]?.title).toBe('Ward')
    expect(found.length).toBeGreaterThan(1)
  })

  it('finds a term by what it does', () => {
    expect(searchTerms('poison counters').map((t) => t.title)).toContain('Toxic')
  })
})
