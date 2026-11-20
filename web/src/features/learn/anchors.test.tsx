// Goal: every anchor of the registry is carried by exactly one place in the app's source and no source names an anchor outside it, every opener is itself an anchor, card refs split into a zone and a name, and the dev table shows each anchor that is always drawn exactly once.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '@/features/table/dev/fixture'
import { ANCHORS, FIXED_ANCHORS, STOP_STEPS, cardRef, isAnchor } from './anchors'

const SRC = join(import.meta.dirname, '..', '..')

function sourcesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return name === 'generated' ? [] : sourcesIn(path)
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })
}

/** Every `anchor('…')` of the sources outside the registry, with the file it is in. */
function anchorCalls(): { id: string; file: string }[] {
  return sourcesIn(SRC)
    .filter((path) => !path.endsWith(join('learn', 'anchors.ts')))
    .flatMap((path) =>
      [...readFileSync(path, 'utf8').matchAll(/\banchor\('([^']+)'\)/g)].map((match) => ({
        id: match[1] ?? '',
        file: relative(SRC, path),
      })),
    )
}

describe('the anchor registry', () => {
  const calls = anchorCalls()

  it.each(FIXED_ANCHORS)('%s is carried by exactly one place', (id) => {
    expect(calls.filter((call) => call.id === id).map((call) => call.file)).toHaveLength(1)
  })

  it('names no anchor outside the registry', () => {
    expect(calls.filter((call) => !isAnchor(call.id))).toEqual([])
  })

  it('carries every stop step through the one templated anchor of the step ladder', () => {
    const source = readFileSync(join(SRC, 'features', 'table', 'turn', 'StepLadder.tsx'), 'utf8')
    expect(source).toContain('anchor(`stops.step.${step}`')
    for (const step of STOP_STEPS) expect(isAnchor(`stops.step.${step}`)).toBe(true)
  })

  it.each(Object.entries(ANCHORS).filter(([, info]) => info.opener !== undefined))(
    "%s's opener is an anchor",
    (_, info) => {
      for (const opener of [info.opener ?? ''].flat()) expect(isAnchor(opener)).toBe(true)
    },
  )

  it.each([
    ['hand:Forest', { zone: 'hand', name: 'Forest' }],
    ['board:Giada, Font of Hope', { zone: 'board', name: 'Giada, Font of Hope' }],
    ['opp:Dregscape Zombie', { zone: 'opp', name: 'Dregscape Zombie' }],
    ['pass.primary', null],
    ['nowhere:Forest', null],
  ])('reads %s as a card ref', (ref, want) => {
    expect(cardRef(ref)).toEqual(want)
  })
})

describe('the dev table', () => {
  // Drawn on every two-seat table at the desktop layout whatever the decision.
  const ALWAYS = [
    'own.pod',
    'opp.pod',
    'own.life',
    'opp.life',
    'own.board',
    'opp.board',
    'turn.badge',
    'ask',
    'hand',
  ]

  it.each(ALWAYS)('draws %s once', (id) => {
    const { container, unmount } = renderTable({ fixture: buildFixture({ seats: 2, permanents: 2 }) })
    expect(container.ownerDocument.querySelectorAll(`[data-coach="${id}"]`)).toHaveLength(1)
    unmount()
  })
})
