import { useStore } from 'zustand'
import { useFx } from './hooks'
import type { Spotlight } from './spotlight'
import type { Flight } from './travel'

/** The moves of the table that are flying now; the same list while no batch starts or lands one. */
export function useFlights(): readonly Flight[] {
  return useStore(useFx().travel.store, (state) => state.flights)
}

/** The spell that stands large over the shared strip now, if one does. */
export function useSpotlight(): Spotlight | null {
  return useStore(useFx().travel.store, (state) => state.spotlight)
}
