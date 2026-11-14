// Goal: the pages wear the table's look: the header is a plaque under a brass hairline with the brand in the display face, the page has a warm light from its top in its own colours, panels lift a little, and a precon row's Play is a quiet button so the page keeps one main action.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter } from '@/test-utils/cssBlocks'

const read = (path: string): string => readFileSync(join(import.meta.dirname, path), 'utf8')

describe('the look of the pages', () => {
  it('draws the header as a plaque under a brass hairline, the brand in the display face', () => {
    const shell = read('components/AppShell.tsx')
    expect(shell).toContain('<header className="plaque border-b border-brass/40">')
    expect(shell).toMatch(/font-display[^"]*font-bold/)
  })

  it('lights the page warmly from its top with its own tokens, and lifts a panel', () => {
    const css = read('index.css')
    const body = blockAfter(blockAfter(css, '@layer base'), 'body {')
    expect(body).toMatch(/radial-gradient\([\s\S]*var\(--brass\)[\s\S]*transparent/)
    expect(body).not.toMatch(/oklch\(/)
    expect(blockAfter(css, '@utility surface')).toContain('shadow-e1')
  })

  it("keeps a precon row's Play quiet", () => {
    expect(read('features/precons/PreconRows.tsx')).toMatch(/variant="secondary"\s+data-play=/)
  })
})
