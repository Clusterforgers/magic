import { useStore } from 'zustand'
import { useFx } from './hooks'
import type { Flight } from './travel'

/** The moves of the table that are flying now; the same list while no batch starts or lands one. */
export function useFlights(): readonly Flight[] {
  return useStore(useFx().travel.store, (state) => state.flights)
}
