// Goal: a fallen hero understands the Spirit: the haunt model reads the ectoplasm and says which actions can be taken now and why not; the Spirit panel shows every action as a button that activates a ready one and explains a closed one; the moment of falling shows for a batch that turns the viewer's seat into a Spirit, never for a snapshot or another seat, and closes; the log names haunt actions and a return to life.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { AbilityRef, ObjectView, PlayerView, PriorityOptions, StateView } from '@/protocol/generated'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import { batchOf, clone, must } from '@/test-utils/batches'
import { renderCueTable } from '@/test-utils/cueTable'
import { pass } from '@/test-utils/turnEvents'
import { describeEvent } from '../eventText'
import { HAUNT_ACTIONS, ectoplasmOf, hauntOptions, hauntWords, SPIRIT_EMBLEM, spiritEmblem } from './haunt'
import { SpiritPanel } from './SpiritPanel'
import { fellBetween, SpiritMoment } from './SpiritMoment'

const EMBLEM = 40

const spirit = (ectoplasm: number): PlayerView =>
  ({
    seat: 0,
    spirit: true,
    counters: ectoplasm === 0 ? [] : [[{ Other: 'ectoplasm' }, ectoplasm]],
    command: { count: 1, cards: [EMBLEM] },
  }) as unknown as PlayerView

const emblem = { id: EMBLEM, kind: 'Emblem', name: SPIRIT_EMBLEM } as unknown as ObjectView

const ref = (ability: number): AbilityRef => ({ object: EMBLEM, ability })

const offering = (abilities: number[]): PriorityOptions =>
  ({ abilities: abilities.map(ref) }) as unknown as PriorityOptions

describe('the haunt model', () => {
  it.each([
    [0, 0],
    [3, 3],
  ])('reads %i ectoplasm from the player counters', (held, read) => {
    expect(ectoplasmOf(spirit(held))).toBe(read)
  })

  it('finds the Restless Spirit among the command zone', () => {
    expect(spiritEmblem(spirit(1), { [EMBLEM]: emblem })).toBe(emblem)
    expect(spiritEmblem(spirit(1), {})).toBeUndefined()
  })

  it.each([
    ['one ectoplasm', 1, [1, 2, 3, 4], { Howl: 'Needs 3 ectoplasm', 'Rise Again': 'Needs 5 ectoplasm' }],
    ['five, out of the main phase', 5, [1, 2, 3, 4, 5], { 'Rise Again': 'Only in your main phase' }],
  ])('with %s, readies what the engine offers and says why not for the rest', (_, held, offered, closed) => {
    const options = hauntOptions(spirit(held), emblem, offering(offered))
    expect(options.map((o) => o.name)).toEqual(HAUNT_ACTIONS.map((a) => a.name))
    for (const option of options) {
      if (offered.includes(option.index)) expect(option.ready).toEqual(ref(option.index))
      else expect(option.why).toBe((closed as Record<string, string>)[option.name])
    }
  })

  it('says to wait while the Spirit has no priority', () => {
    const [wail] = hauntOptions(spirit(2), emblem, null)
    expect(wail?.why).toBe('Wait for your priority')
  })

  it.each([
    [SPIRIT_EMBLEM, 1, 'Wail'],
    [SPIRIT_EMBLEM, 6, 'Rise Again'],
    [SPIRIT_EMBLEM, 0, undefined],
    ['Serra Angel', 1, undefined],
  ])('names ability %i of %s as %s', (name, ability, words) => {
    expect(hauntWords(name, ability)).toBe(words)
  })
})

describe('the Spirit panel', () => {
  it('shows each action as a button, activates a ready one and explains a closed one', () => {
    const onActivate = vi.fn()
    render(
      <SpiritPanel player={spirit(1)} emblem={emblem} options={offering([1, 2])} onActivate={onActivate} />,
    )
    expect(screen.getByRole('region', { name: 'Haunt actions' })).toHaveTextContent('1 ectoplasm')
    const buttons = screen.getAllByRole('button')
    expect(buttons).toHaveLength(HAUNT_ACTIONS.length)
    const wail = screen.getByRole('button', { name: /Wail/ })
    expect(wail).toHaveAttribute('data-ready', 'true')
    expect(wail).toHaveTextContent('Enemy −2 life')
    fireEvent.click(wail)
    expect(onActivate).toHaveBeenCalledWith(ref(1))
    const howl = screen.getByRole('button', { name: /Howl/ })
    expect(howl).toHaveAttribute('aria-disabled', 'true')
    expect(howl).toHaveTextContent('Needs 3 ectoplasm')
    fireEvent.click(howl)
    expect(onActivate).toHaveBeenCalledTimes(1)
  })
})

describe('the moment of falling', () => {
  const seat = (spiritNow: boolean) => ({ seat: 0, spirit: spiritNow }) as PlayerView

  it.each([
    ['the viewer falls', [seat(false)], [seat(true)], 0, true],
    ['the viewer was already a Spirit', [seat(true)], [seat(true)], 0, false],
    ['another seat falls', [seat(false)], [seat(true)], 1, false],
    ['a spectator', [seat(false)], [seat(true)], null, false],
  ])('when %s, shows: %s', (_, before, after, viewer, shown) => {
    expect(fellBetween(before, after, viewer)).toBe(shown)
  })

  const fall = (view: StateView) => {
    const own = must(view.players.find((p) => p.seat === 0))
    own.spirit = true
    own.life = 0
  }

  it('stays away for a snapshot that turns the viewer into a Spirit', () => {
    const table = renderCueTable({}, { ui: <SpiritMoment /> })
    table.update((state) => {
      const view = clone(must(state.view))
      fall(view)
      return { view, seq: (state.seq ?? 0) + 40 }
    })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('opens on the batch, says what a Spirit does and closes on its button', () => {
    const table = renderCueTable({}, { ui: <SpiritMoment /> })
    const state = table.store.getState()
    const view = clone(must(state.view))
    fall(view)
    view.priority = { holder: 1, passed: [] }
    act(() =>
      applyBatchToStore(table.store, batchOf(view, { from_seq: must(state.seq), events: [pass(0)] })),
    )
    const dialog = screen.getByRole('dialog', { name: 'You are a Restless Spirit' })
    expect(dialog).toHaveTextContent('ectoplasm')
    expect(dialog).toHaveTextContent('Rise Again')
    fireEvent.click(screen.getByRole('button', { name: 'Haunt the fight' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})

describe('the log', () => {
  const names = { seat: (seat: number) => ['Ana', 'Bo'][seat] ?? 'someone', card: () => SPIRIT_EMBLEM }

  it('names a haunt action', () => {
    const line = describeEvent(
      { Public: { AbilityActivated: { player: 1, source: { id: EMBLEM, counter: 0 }, ability: 4 } } },
      names,
    )
    expect(line.text).toBe('Bo haunts the fight: Hinder')
  })
})
