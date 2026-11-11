import { useEffect, useMemo, useRef, useState } from 'react'
import { useStore } from 'zustand'
import { TourCard } from '@/components/tour/TourCard'
import { useGame } from '@/features/game/gameContext'
import { startTour, tourStore } from '@/features/table/tour/tourStore'
import { advance, lessonFacts, lessonOf } from './lesson'
import type { LessonId } from './lesson'
import { markDone } from './tutorials'
import type { TutorialId } from './tutorials'

/** How often a step that reads the page (an open panel) looks again, in ms. */
const LOOK_MS = 500

/**
 * The coach of a guided lesson on the tutorial table: the current step's card over the table, which stays usable, so
 * the player does what the step asks. A step moves on when the game shows it done (`advance`), a read step with Next.
 * Finishing marks the tutorial done; Skip ends the lesson and leaves the game as it is.
 */
export function LessonCoach({ lesson: id }: { lesson: LessonId }) {
  const lesson = lessonOf(id)
  const state = useStore(useGame().session.store)
  const [, setTick] = useState(0)
  const facts = lessonFacts(state)
  const [index, setIndex] = useState(0)
  const [ended, setEnded] = useState(false)
  const at = advance(lesson.steps, index, facts)
  if (at !== index) setIndex(at)

  useEffect(() => {
    if (ended) return
    const timer = window.setInterval(() => setTick((value) => value + 1), LOOK_MS)
    return () => window.clearInterval(timer)
  }, [ended])

  const step = lesson.steps[at]
  // Targets are worked out from the facts, but the card follows them only when they change.
  const targets = step?.targets(facts) ?? []
  const key = targets.join('|')
  const stop = useMemo(
    () => (step === undefined ? undefined : { id: step.id, title: step.title, text: step.text, targets }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` stands for the targets' contents
    [step, key],
  )
  if (ended || stop === undefined || step === undefined) return null
  const finish = (): void => {
    markDone(id)
    setEnded(true)
  }
  const last = at === lesson.steps.length - 1
  return (
    <TourCard
      stop={stop}
      index={at}
      count={lesson.steps.length}
      onNext={step.done === undefined ? (last ? finish : () => setIndex(at + 1)) : undefined}
      nextLabel={last ? 'Finish' : 'Next'}
      waiting={step.waiting}
      onSkip={() => setEnded(true)}
      skipLabel="Skip lesson"
      passThrough
      keys={false}
      slot="lesson-coach"
    />
  )
}

/** Starts the table tour once the table shows, and marks the tutorial done when the tour ends. */
function TableTourLesson() {
  const step = useStore(tourStore, (state) => state.step)
  const started = useRef(false)
  useEffect(() => {
    if (!started.current) {
      started.current = true
      startTour()
      return
    }
    if (step === null) markDone('table')
  }, [step])
  return null
}

/** The coach of the tutorial a game of this tab plays: a lesson, or the table tour. */
export function TutorialCoach({ tutorial }: { tutorial: TutorialId }) {
  if (tutorial === 'game' || tutorial === 'manual') return <LessonCoach lesson={tutorial} />
  if (tutorial === 'table') return <TableTourLesson />
  return null
}
