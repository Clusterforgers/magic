import { useLocation } from '@tanstack/react-router'
import { useEffect, useMemo } from 'react'
import { useStore } from 'zustand'
import { TourCard } from '@/components/tour/TourCard'
import { setLexiconHeld, showLexicon } from '@/features/table/lexicon/lexiconStore'
import { endPageTour, movePageTour, pageTourStore, placeOf } from './pageTours'
import { markDone } from './tutorials'

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
 * The page tour on show (`pageTourStore`), drawn over whatever page the router shows: the part's stop where the page
 * is the part's own, else a card that says how to get there. The page stays usable under it, so the player can follow
 * the tour from page to page. Lives once in the root layout.
 */
export function PageTourHost() {
  const { tour, part, stop } = useStore(pageTourStore)
  const path = useLocation({ select: (location) => location.pathname })
  useLexiconTour(tour === 'lexicon')
  const place = tour === null ? null : placeOf(tour, part, stop, path)
  const lead = place?.kind === 'lead' ? place.lead : null
  const leadStop = useMemo(
    () => (lead === null ? null : { id: 'lead', title: 'Continue the tour', text: lead, targets: [] }),
    [lead],
  )
  if (place === null) return null
  if (place.kind === 'lead' && leadStop !== null)
    return (
      <TourCard
        stop={leadStop}
        index={place.index}
        count={place.count}
        onBack={back}
        waiting="Waiting for the page"
        onSkip={endPageTour}
        skipLabel="End tour"
        passThrough
        keys={false}
        slot="page-tour"
      />
    )
  if (place.kind !== 'stop') return null
  return (
    <TourCard
      stop={place.stop}
      index={place.index}
      count={place.count}
      onBack={back}
      onNext={forward}
      nextLabel={place.last ? 'Done' : 'Next'}
      onSkip={endPageTour}
      skipLabel="End tour"
      passThrough
      keys={false}
      slot="page-tour"
    />
  )
}
