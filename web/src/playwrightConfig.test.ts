// Goal: every end-to-end run keeps the trace of a failed test and drops the traces of passing ones, so a flake that does not recur in solo runs leaves what names its cause; and every spec takes `test` from `e2e/test.ts`, which closes the browser contexts a test leaves open, so no spec slows the ones after it in a full run.
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import config from '../playwright.config'

const E2E = join(import.meta.dirname, '..', 'e2e')

describe('the Playwright config', () => {
  it('retains a trace for a failed test only', () => {
    expect(config.use?.trace).toBe('retain-on-failure')
  })

  it('writes the traces where git ignores them', () => {
    expect(config.outputDir).toBe('test-results')
  })
})

describe('the end-to-end specs', () => {
  const specs = readdirSync(E2E).filter((name) => name.endsWith('.spec.ts'))

  it('are found', () => {
    expect(specs.length).toBeGreaterThan(0)
  })

  it('take test from e2e/test.ts, which closes the contexts a test leaves open', () => {
    const elsewhere = specs.filter((name) => {
      const text = readFileSync(join(E2E, name), 'utf8')
      return !/^import \{[^}]*\btest\b[^}]*\} from '\.\/test'$/m.test(text)
    })
    expect(elsewhere).toEqual([])
  })
})
