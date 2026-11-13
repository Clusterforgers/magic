// Goal: every movement of the Delves pages (entrances, pulses, lifts) runs only under `prefers-reduced-motion: no-preference`, and a life bar does not slide under reduce.
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, it } from 'vitest'

const DIR = import.meta.dirname
const SOURCES = readdirSync(DIR)
  .filter((name) => name.endsWith('.tsx') && !name.endsWith('.test.tsx'))
  .map((name) => [name, readFileSync(join(DIR, name), 'utf8')] as const)

const MOVING = /(?<![\w:-])(?:[\w-]+:)*(?:animate-[\w-]+|slide-in-[\w-]+|fade-in|-?translate-y-0\.5)(?![\w-])/g

it.each(SOURCES)('%s moves only under motion-safe', (_, source) => {
  const unsafe = [...source.matchAll(MOVING)].map((m) => m[0]).filter((cls) => !cls.includes('motion-safe:'))
  expect(unsafe).toEqual([])
})

it('keeps the life bar still under reduced motion', () => {
  const header = SOURCES.find(([name]) => name === 'RunHeader.tsx')?.[1] ?? ''
  expect(header).toContain('motion-reduce:transition-none')
})
