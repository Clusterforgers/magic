/** What `/dev/support` shows: the fixture, nothing counted yet, a page still loading or a failed load. */
export type DevSupportView = 'full' | 'empty' | 'loading' | 'error'

/** The address parameters of `/dev/support`. */
export interface DevSupportSearch {
  /** The view; anything else is `full`. */
  view: DevSupportView
}

const VIEWS: readonly DevSupportView[] = ['full', 'empty', 'loading', 'error']

/** Reads `view` from the address, leniently. */
export function parseDevSupportSearch(raw: Record<string, unknown>): DevSupportSearch {
  const view = VIEWS.find((known) => known === raw.view)
  return { view: view ?? 'full' }
}
