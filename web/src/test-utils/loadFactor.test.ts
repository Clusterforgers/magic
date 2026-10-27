// Goal: the load factor is 1 when strict or idle, grows as 1 + load / cores, takes a valid override, ignores an invalid one, never leaves [1, MAX_LOAD_FACTOR], and scaled budgets round up.
import { describe, expect, it } from 'vitest'
import { MAX_LOAD_FACTOR, loadFactor, loadFactorOf, scaled } from './loadFactor'

describe('loadFactorOf', () => {
  it.each([
    ['idle', { load: 0, cores: 24 }, 1],
    ['half busy', { load: 12, cores: 24 }, 1.5],
    ['four times oversubscribed', { load: 96, cores: 24 }, 5],
    ['beyond the cap', { load: 10_000, cores: 24 }, MAX_LOAD_FACTOR],
    ['no cores reported', { load: 3, cores: 0 }, 4],
    ['negative load', { load: -5, cores: 4 }, 1],
    ['strict on a busy machine', { strict: '1', load: 96, cores: 24 }, 1],
    ['strict beats an override', { strict: '1', override: '6', load: 0, cores: 24 }, 1],
    ['strict only on 1', { strict: '0', load: 24, cores: 24 }, 2],
    ['an override', { override: '3', load: 96, cores: 24 }, 3],
    ['an override under 1', { override: '0.2', load: 0, cores: 24 }, 1],
    ['an override over the cap', { override: '50', load: 0, cores: 24 }, MAX_LOAD_FACTOR],
    ['an empty override', { override: ' ', load: 24, cores: 24 }, 2],
    ['a word as override', { override: 'fast', load: 24, cores: 24 }, 2],
  ])('%s', (_name, input, factor) => {
    expect(loadFactorOf(input)).toBe(factor)
  })
})

describe('scaled', () => {
  it.each([
    [1500, 1, 1500],
    [1500, 2.5, 3750],
    [5, 1.01, 6],
    [20_000, MAX_LOAD_FACTOR, 240_000],
  ])('%i ms at %d is %i ms', (ms, factor, out) => {
    expect(scaled(ms, factor)).toBe(out)
  })

  it('uses the machine factor by default, which stays in range', () => {
    const factor = loadFactor()
    expect(factor).toBeGreaterThanOrEqual(1)
    expect(factor).toBeLessThanOrEqual(MAX_LOAD_FACTOR)
    expect(scaled(1000)).toBeGreaterThanOrEqual(1000)
  })
})
