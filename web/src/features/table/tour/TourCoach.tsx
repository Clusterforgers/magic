import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { CoachTour } from '@/features/learn/CoachTour'
import type { CardLookup } from '@/features/learn/resolve'
import { endTour, moveTour } from './tourStore'
import { TOUR } from './tourSteps'
import type { TourStep } from './tourSteps'

const forward = (): void => moveTour(1)
const back = (): void => moveTour(-1)
const NO_CARDS: ReadonlySet<number> = new Set()

/**
 * One step of the table tour on the coach (`CoachTour`): the step's anchor lit and a card with Skip tour, Back and
 * Next (Done on the last); the arrow keys and Escape do the same. It takes every press, so the table under it takes
 * none.
 */
export function TourCoach({ step, index }: { step: TourStep; index: number }) {
  const objects = usePlay((state) => state.objectsById)
  const lookup = useMemo<CardLookup>(() => ({ objects, usable: NO_CARDS }), [objects])
  return (
    <CoachTour
      slot="table-tour"
      name="Table tour"
      step={step}
      index={index}
      count={TOUR.length}
      onNext={forward}
      nextLabel={index === TOUR.length - 1 ? 'Done' : 'Next'}
      onBack={back}
      onSkip={endTour}
      skipLabel="Skip tour"
      lookup={lookup}
    />
  )
}
