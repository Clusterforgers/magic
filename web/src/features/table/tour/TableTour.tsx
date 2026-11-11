import { useContext, useEffect, useRef } from 'react'
import { useStore } from 'zustand'
import { TourCard } from '@/components/tour/TourCard'
import { usePlay } from '@/features/game/gameContext'
import { useSide } from '../sideContext'
import { TableOptionsContext, TOUR_BY_DEFAULT } from '../tableOptions'
import { endTour, moveTour, startTour, tourSeen, tourStore } from './tourStore'
import { TOUR } from './tourSteps'
import type { TourStep } from './tourSteps'

/** Brings up the side tab a step points at, and closes a rail or sheet panel the tour opened when it ends. */
function useStepTab(step: TourStep | undefined): void {
  const side = useSide()
  const opened = useRef(false)
  const { kind, tab, open, activate, setOpen } = side
  useEffect(() => {
    if (step?.tab === undefined) return
    if (tab === step.tab && open) return
    if (!open && kind !== 'column') opened.current = true
    if (tab === step.tab) setOpen(true)
    else activate(step.tab)
  }, [step, kind, tab, open, activate, setOpen])
  useEffect(() => {
    if (step !== undefined || !opened.current) return
    opened.current = false
    if (kind !== 'column') setOpen(false)
  }, [step, kind, setOpen])
}

const forward = (): void => moveTour(1)
const back = (): void => moveTour(-1)

/**
 * The table tour for a new player: one part of the table lit at a time with a card that says what it is for, Back, Next
 * and Skip (`TourCard`). It starts by itself the first time a seat sits at a table in this browser (`tourSeen`) and
 * again from the help menu (`TableOptions.tour`). While it is up the table under it takes no presses.
 */
export function TableTour() {
  const index = useStore(tourStore, (state) => state.step)
  const seated = usePlay((state) => state.seat !== null)
  const step = index === null ? undefined : TOUR[index]
  useStepTab(step)

  const auto = useContext(TableOptionsContext).tour ?? TOUR_BY_DEFAULT
  useEffect(() => {
    // A browser under automation (end-to-end tests, screenshots) is no new player.
    if (auto && seated && !navigator.webdriver && !tourSeen() && tourStore.getState().step === null)
      startTour()
  }, [auto, seated])

  if (step === undefined || index === null) return null
  return (
    <TourCard
      stop={step}
      index={index}
      count={TOUR.length}
      onBack={back}
      onNext={forward}
      nextLabel={index === TOUR.length - 1 ? 'Done' : 'Next'}
      onSkip={endTour}
      skipLabel="Skip tour"
      slot="table-tour"
    />
  )
}
