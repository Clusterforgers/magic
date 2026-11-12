// Goal: every end-to-end run keeps the trace of a failed test and drops the traces of passing ones, so a flake that does not recur in solo runs leaves what names its cause.
import { describe, expect, it } from 'vitest'
import config from '../playwright.config'

describe('the Playwright config', () => {
  it('retains a trace for a failed test only', () => {
    expect(config.use?.trace).toBe('retain-on-failure')
  })

  it('writes the traces where git ignores them', () => {
    expect(config.outputDir).toBe('test-results')
  })
})
