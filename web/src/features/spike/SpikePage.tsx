import { DndContext, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core'
import { useCallback, useEffect, useRef, useState, type RefObject } from 'react'
import { CardPreview } from '@/features/board/CardPreview'
import { Seat } from '@/features/board/Seat'
import { useGameStore } from '@/game/store'
import type { ObjectId } from '@/game/types'
import { interpretActivation } from './activation'
import { startFrameSampler, summarizeFrames, type FrameStats } from './frameSampler'
import { createBoard, createRng, nextPatches, type SimConfig } from './simulator'

const TICK_MS = 100

declare global {
  interface Window {
    __spike?: {
      resetFrames: () => void
      frameStats: () => FrameStats
      lastResolution: () => Resolution
    }
  }
}

function useSimulation(config: SimConfig): void {
  const load = useGameStore((state) => state.load)
  const applyPatches = useGameStore((state) => state.applyPatches)
  useEffect(() => {
    load(createBoard(config))
    const rng = createRng(config.seed + 1)
    let tick = 0
    const timer = setInterval(() => {
      tick += 1
      applyPatches(nextPatches(Object.values(useGameStore.getState().objects), rng, tick))
    }, TICK_MS)
    return () => clearInterval(timer)
  }, [config, load, applyPatches])
}

type Resolution = { source: ObjectId; target: ObjectId } | null

function useSpikeProbe(resolution: RefObject<Resolution>): void {
  useEffect(() => {
    const frames: number[] = []
    const stop = startFrameSampler(frames)
    window.__spike = {
      resetFrames: () => {
        frames.length = 0
      },
      frameStats: () => summarizeFrames(frames),
      lastResolution: () => resolution.current,
    }
    return () => {
      stop()
      delete window.__spike
    }
  }, [resolution])
}

/** Spike board: N players, many permanents, 10 Hz patches, click and drag targeting. */
export function SpikePage({ config }: { config: SimConfig }) {
  useSimulation(config)
  const resolution = useRef<Resolution>(null)
  useSpikeProbe(resolution)
  const [resolved, setResolved] = useState<string>('none')
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }))
  const decision = useGameStore((state) => state.decision)

  const finish = useCallback((source: ObjectId, target: ObjectId) => {
    setResolved(`${source}->${target}`)
    useGameStore.getState().setDecision(null)
    resolution.current = { source, target }
  }, [])

  const onActivate = useCallback(
    (id: ObjectId) => {
      const { objects, decision: current, setDecision } = useGameStore.getState()
      const result = interpretActivation(objects, current, id)
      if (result.kind === 'begin') setDecision(result.decision)
      else if (result.kind === 'resolve') finish(result.source, result.target)
      else if (result.kind === 'cancel') setDecision(null)
    },
    [finish],
  )

  const onDragEnd = useCallback(
    ({ active, over }: DragEndEvent) => {
      const { decision: current } = useGameStore.getState()
      if (over && current?.legal.has(Number(over.id))) finish(Number(active.id), Number(over.id))
    },
    [finish],
  )

  const onDragStart = useCallback(({ active }: { active: { id: string | number } }) => {
    const { objects, setDecision } = useGameStore.getState()
    const result = interpretActivation(objects, null, Number(active.id))
    if (result.kind === 'begin') setDecision(result.decision)
  }, [])

  return (
    <DndContext sensors={sensors} onDragStart={onDragStart} onDragEnd={onDragEnd}>
      <main className="grid gap-2 p-2 md:grid-cols-2">
        <p data-testid="status" className="col-span-full text-sm">
          {decision ? 'Choose a target' : 'Your priority'} · last: {resolved}
        </p>
        {Array.from({ length: config.players }, (_, player) => (
          <Seat key={player} player={player} onActivate={onActivate} />
        ))}
      </main>
      <CardPreview />
    </DndContext>
  )
}
