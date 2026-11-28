import { useLocation } from '@tanstack/react-router'
import { Suspense, lazy, useEffect, useMemo } from 'react'
import { useStore } from 'zustand'
import { setLexiconHeld, showLexicon } from '@/features/table/lexicon/lexiconStore'
import { endPageTour, movePageTour, pageTourStore, placeOf } from './pageTours'
import type { TourStop } from './pageTours'
import { TUTORIALS, markDone } from './tutorials'

// The coach loads with a tour's first stop, so a page that never tours never loads it.
const CoachTour = lazy(() => import('./CoachTour').then((module) => ({ default: module.CoachTour })))

const forward = (): void => {
  const finished = movePageTour(1)
  if (finished !== null) markDone(finished)
}
const back = (): void => {
  movePageTour(-1)
}

/** Keeps the lexicon open and modeless while its tour runs, and lets it go after. */
function useLexiconTour(on: boolean): void {
  useEffect(() => {
    if (!on) return
    showLexicon()
    setLexiconHeld(true)
    return () => setLexiconHeld(false, true)
  }, [on])
}

/**
 * The page tour on show (`pageTourStore`), drawn on the Learn coach (`CoachTour`) over whatever page the router
 * shows: the part's stop where the page is the part's own, else a card that says how to get there. The page stays
 * usable under it, so the player can follow the tour from page to page. Lives once in the root layout.
 */
export function PageTourHost() {
  const { tour, part, stop } = useStore(pageTourStore)
  const path = useLocation({ select: (location) => location.pathname })
  useLexiconTour(tour === 'lexicon')
  const place = tour === null ? null : placeOf(tour, part, stop, path)
  const lead = place?.kind === 'lead' ? place.lead : null
  const leadStop = useMemo<TourStop | null>(
    () => (lead === null ? null : { id: 'lead', title: 'Continue the tour', text: lead, at: [] }),
    [lead],
  )
  if (place === null || tour === null) return null
  const name = TUTORIALS.find((each) => each.id === tour)?.title ?? 'Tour'
  const shared = {
    slot: 'page-tour',
    name,
    index: place.index,
    count: place.count,
    onBack: back,
    onSkip: endPageTour,
    skipLabel: 'End tour',
    passThrough: true,
    keys: false,
    pageBar: true,
  } as const
  return (
    <Suspense fallback={null}>
      {place.kind === 'lead' ? (
        leadStop === null ? null : (
          <CoachTour {...shared} step={leadStop} waiting="Waiting for the page" />
        )
      ) : (
        <CoachTour {...shared} step={place.stop} onNext={forward} nextLabel={place.last ? 'Done' : 'Next'} />
      )}
    </Suspense>
  )
}
