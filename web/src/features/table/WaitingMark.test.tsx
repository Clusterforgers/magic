// Goal: at a table of three or more, every form of a seat (the pod panel, the one-line pod, the tile) whose player the game waits on carries a mark named like the strip's line ("Waiting for Cy to scry"), for a spectator too and for several seats at once; a seat the game does not wait on, the viewer's own decision and a duel carry none.
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup } from '@testing-library/react'
import type { PlayState } from '@/play/store'
import type { DecidingView, PlayerId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture, nameOf } from './dev/fixture'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'

afterEach(cleanup)

type Form = 'panel' | 'line' | 'tile'
const FORMS: Form[] = ['panel', 'line', 'tile']

interface Scene {
  seats: number
  deciding: DecidingView[]
  /** The viewer; null for a spectator. */
  viewer?: PlayerId | null
}

function show(form: Form, seat: PlayerId, scene: Scene) {
  const fixture = buildFixture({ seats: scene.seats, permanents: 2 })
  const state = (first: PlayState): PlayState => ({
    ...first,
    pending: null,
    deciding: scene.deciding,
    ...(scene.viewer === undefined ? {} : { seat: scene.viewer }),
  })
  const ui =
    form === 'tile' ? (
      <SeatTile seat={seat} focused={false} onFocusSeat={() => undefined} />
    ) : (
      <SeatPanel seat={seat} variant={form} />
    )
  renderTable({ fixture, ui, state })
  const root = must(
    document.querySelector<HTMLElement>(
      form === 'tile' ? `[data-tile-seat="${seat}"]` : `section[data-seat="${seat}"]`,
    ),
  )
  return {
    mark: root.querySelector<HTMLElement>('[data-slot="waiting-mark"]')?.getAttribute('aria-label') ?? null,
    label: root.getAttribute('aria-label') ?? '',
  }
}

const scry = (seat: PlayerId): DecidingView => ({ seat, kind: 'Scry' })

describe.each(FORMS)('the %s form', (form) => {
  it('marks the seat the game waits on at a table of four, with the strip’s words', () => {
    const shown = show(form, 2, { seats: 4, deciding: [scry(2)] })
    expect(shown.mark).toBe(`Waiting for ${nameOf(2)} to scry`)
    // A tile is one button: its name carries the words.
    if (form === 'tile') expect(shown.label).toContain(`waiting for ${nameOf(2)} to scry`)
  })

  it('marks every seat asked at once', () => {
    for (const seat of [1, 3]) {
      expect(show(form, seat, { seats: 4, deciding: [scry(1), scry(3)] }).mark).toBe(
        `Waiting for ${nameOf(seat)} to scry`,
      )
      cleanup()
    }
  })

  it('marks the seat for a spectator too', () => {
    expect(show(form, 0, { seats: 3, deciding: [scry(0)], viewer: null }).mark).toBe(
      `Waiting for ${nameOf(0)} to scry`,
    )
  })

  it.each<[string, PlayerId, Scene]>([
    ['a seat the game does not wait on', 1, { seats: 4, deciding: [scry(2)] }],
    ['a duel', 1, { seats: 2, deciding: [scry(1)] }],
  ])('leaves %s unmarked', (_, seat, scene) => {
    expect(show(form, seat, scene).mark).toBeNull()
  })
})

describe.each<Form>(['panel', 'line'])('the viewer’s own %s pod', (form) => {
  it('carries no mark: the prompt is the viewer’s', () => {
    expect(show(form, 0, { seats: 4, deciding: [scry(0)], viewer: 0 }).mark).toBeNull()
  })
})
