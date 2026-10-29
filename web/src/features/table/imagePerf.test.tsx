// Goal: a crowded table with pictures on (12 seats of 40 identified permanents) mounts inside the same time budget as the text faces, every picture is lazy and decoded off the main thread, and no card on the battlefield asks for more than the `normal` size.
import { act } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice } from '@/features/cards/imageSetting'
import { scaled } from '@/test-utils/loadFactor'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import type { Fixture } from './dev/fixture'

/** As for the text faces in `perf.test.tsx`: five times what mounting takes here idle; `scaled` stretches it by the load. */
const MOUNT_BUDGET_MS = 1500

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
})

// Every card gets an oracle id made from its name: the URLs are fixtures and never reach a network.
function identified(fixture: Fixture): Fixture {
  const objects = fixture.view.objects.map((object) =>
    object.kind === undefined
      ? { ...object, identity: { oracle_id: `id-${object.name.replace(/\W/g, '')}` } }
      : object,
  )
  return { ...fixture, view: { ...fixture.view, objects } }
}

describe('a crowded table with pictures on', () => {
  it.each([4, 12])(`%i seats of 40 permanents mount inside ${MOUNT_BUDGET_MS} ms`, (seats) => {
    const fixture = identified(buildFixture({ seats }))
    const started = performance.now()
    const h = startSession()
    h.join(0, seats)
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    const { container } = renderLiveTable(h.session, fixtureLookup(fixture), { mode: 'grid', stacks: true })
    expect(performance.now() - started).toBeLessThan(scaled(MOUNT_BUDGET_MS))
    const pictures = [...container.querySelectorAll('[data-object-id] img')]
    expect(pictures.length).toBeGreaterThan(seats * 10)
    for (const img of pictures) {
      expect(img.getAttribute('loading')).toBe('lazy')
      expect(img.getAttribute('decoding')).toBe('async')
      expect(img.getAttribute('src')).toMatch(/\/(small|normal)$/)
    }
  })
})
