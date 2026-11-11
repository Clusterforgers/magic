// Goal: the rules lexicon names every step, zone, type, layer, counter, casting way and face-down kind of the engine, and marks only the rules terms meant to be marked.
import { describe, expect, it } from 'vitest'
import type { CastWay, DayNight, DeathCause, FaceDown } from '@/protocol/generated'
import { mentions, searchTerms, termFor, TERMS } from './index'
import { MARKED_RULES, RULES } from './rules'
import { CARD_TYPES, COUNTERS, LAYERS, STEPS, SUPERTYPES, ZONES } from './vocabulary'

type FaceDownKind<F = FaceDown> = F extends string ? F : keyof F

// Keyed by engine enums, so a new casting way, face-down kind or death cause fails the typecheck until a term covers it.
const CAST_WAYS: Record<CastWay, string> = {
  Flashback: 'Flashback',
  Evoke: 'Evoke',
  Overload: 'Overload',
  Madness: 'Madness',
  Miracle: 'Miracle',
  Suspend: 'Suspend',
  Foretold: 'Foretell',
  Plotted: 'Plot',
  Escape: 'Escape',
  Dash: 'Dash',
  Bestow: 'Bestow',
  Blitz: 'Blitz',
  Prowl: 'Prowl',
  Spectacle: 'Spectacle',
  Freerunning: 'Freerunning',
  Mayhem: 'Mayhem',
  Disturb: 'Disturb',
  Awaken: 'Awaken',
  JumpStart: 'Jump-start',
  Retrace: 'Retrace',
  FaceDown: 'Face down',
  Mutate: 'Mutate',
  Warp: 'Warp',
  Surge: 'Surge',
  WebSlinging: 'Web-slinging',
  Sneak: 'Sneak',
  Prototype: 'Prototype',
  Impending: 'Impending',
  Free: 'Without paying its mana cost',
  Emerge: 'Emerge',
  Harmonize: 'Harmonize',
  Cleave: 'Cleave',
  Alternative: 'Alternative cost',
}

const FACE_DOWN: Record<FaceDownKind, string> = {
  Foretold: 'Foretell',
  Morph: 'Morph',
  Disguise: 'Disguise',
  Manifest: 'Manifest',
  Cloak: 'Cloak',
  Hideaway: 'Hideaway',
  Held: 'Face down',
  Sealed: 'Face down',
  Attraction: 'Attraction',
}

const DAY_NIGHT: Record<DayNight, string> = { Day: 'Day', Night: 'Night' }

const DEATHS: Record<DeathCause, string> = {
  LethalDamage: 'Lethal damage',
  Deathtouch: 'Deathtouch',
  ZeroToughness: 'Toughness',
}

const VOCABULARY = [STEPS, ZONES, CARD_TYPES, SUPERTYPES, LAYERS, COUNTERS].flatMap((table) =>
  Object.entries(table).map(([variant, [names]]) => [variant, names.split('|')[0] ?? names]),
)

describe('the rules lexicon covers the engine', () => {
  it.each(VOCABULARY)('has %s as %s, a rules term', (_variant, title) => {
    expect(termFor(title)?.kind).toBe('rule')
  })

  it.each([CAST_WAYS, FACE_DOWN, DAY_NIGHT, DEATHS].flatMap((table) => Object.entries(table)))(
    'names %s with the term %s',
    (_variant, name) => {
      expect(termFor(name)?.text.length ?? 0).toBeGreaterThan(10)
    },
  )

  it.each(Object.entries(RULES))('has a topic %s with terms of that topic', (topic, entries) => {
    expect(entries.length).toBeGreaterThan(5)
    const titled = TERMS.filter((term) => term.topic === topic)
    expect(titled).toHaveLength(entries.length)
  })

  it.each([...MARKED_RULES].map((title) => [title]))('marks %s, a rules title', (title) => {
    expect(termFor(title)?.title).toBe(title)
  })
})

describe('searching the lexicon', () => {
  it('finds text only from the start of a word', () => {
    const found = searchTerms('layer').map((term) => term.title)
    expect(found[0]).toBe('Layer 1: copy')
    expect(found).not.toContain('Monarch')
  })
})

describe('marking rules terms', () => {
  it.each([
    ['At the beginning of your upkeep, draw a card.', ['Upkeep']],
    ['At the beginning of the end step, sacrifice it.', ['End step']],
    ['Its mana value is 3 or less.', ['Mana value']],
    ['You may cast it without paying its mana cost.', ['Without paying its mana cost']],
    ['Put it onto the battlefield face down.', ['Face down']],
    ['You become the monarch.', ['Monarch']],
  ])('marks the rules term of "%s"', (text, titles) => {
    expect(mentions(text).map((term) => term.title)).toEqual(titles)
  })

  it.each([
    ['Return target creature card from your graveyard to your hand.'],
    ['Create a 1/1 white Soldier creature token.'],
    ['Target player loses 2 life.'],
    ['Each opponent sacrifices a land.'],
  ])('leaves the everyday words of "%s"', (text) => {
    expect(mentions(text)).toEqual([])
  })
})
