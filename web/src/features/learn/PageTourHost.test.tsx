// Goal: every page tour stop names registry anchors within the coach's word limits, and the host draws a stop on the coach's light with its first drawn anchor lit and the page under it usable, falls back to the next anchor while the first is absent, and shows the lead to the part's page, waiting, on any other page.
import { act, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import { anchor } from './anchors'
import { PageTourHost } from './PageTourHost'
import { endPageTour, PAGE_TOURS, pageTourStore } from './pageTours'
import { TEXT_MAX, TITLE_MAX, knownTarget } from './step'

const stops = PAGE_TOURS.flatMap((tour) =>
  tour.parts.flatMap((part) => part.stops.map((stop) => [`${tour.id}.${stop.id}`, stop] as const)),
)

describe('the page tour stops', () => {
  it.each(stops)('%s names registry anchors in few words', (_, stop) => {
    expect(stop.title.length).toBeLessThanOrEqual(TITLE_MAX)
    expect(stop.text.length).toBeLessThanOrEqual(TEXT_MAX)
    for (const target of stop.at) expect(knownTarget(target)).toBe(true)
  })
})

/** A lobby page with the anchors of the lobby part, the invite only for a host. */
function Lobby({ host }: { host: boolean }) {
  return (
    <>
      {host ? <section {...anchor('lobby.invite')}>Invite</section> : null}
      <section {...anchor('lobby.game')}>
        <button type="button">Leave game</button>
      </section>
      <PageTourHost />
    </>
  )
}

describe('the page tour host', () => {
  beforeEach(() => {
    // jsdom lays nothing out: every element is drawn inside the window, so the anchor's presence decides.
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
      () => new DOMRect(10, 60, 120, 80),
    )
  })
  afterEach(() => {
    act(endPageTour)
    vi.restoreAllMocks()
  })

  it.each([
    [true, 'lobby.invite'],
    [false, 'lobby.game'],
  ])('with a host seat %s lights %s and leaves the page usable', async (host, lit) => {
    pageTourStore.setState({ tour: 'lobby', part: 1, stop: 0 })
    await renderInRouter(<Lobby host={host} />, { routed: true, url: '/game/abc' })
    const card = await screen.findByRole('dialog', { name: 'Invite' })
    const root = card.closest<HTMLElement>('[data-slot="page-tour"]')
    await waitFor(() => expect(root?.dataset['coachLit']).toBe(lit))
    expect(root?.className).toContain('pointer-events-none')
    expect(card.getAttribute('aria-modal')).toBeNull()
    expect(document.querySelectorAll('[data-slot="coach-ring"]')).toHaveLength(1)
    expect(screen.getByText('Choosing a deck and seats')).toBeTruthy()
  })

  it('shows the lead, waiting for the page, while another page is open', async () => {
    pageTourStore.setState({ tour: 'lobby', part: 0, stop: 0 })
    await renderInRouter(<Lobby host />, { routed: true, url: '/game/abc' })
    const card = await screen.findByRole('dialog', { name: 'Continue the tour' })
    expect(card.querySelector('[data-slot="tour-waiting"]')?.textContent).toContain('Waiting for the page')
    expect(screen.queryByRole('button', { name: 'Next' })).toBeNull()
    expect(document.querySelectorAll('[data-slot="coach-ring"]')).toHaveLength(0)
  })

  it('walks on with Next and ends with Done past the last stop', async () => {
    pageTourStore.setState({ tour: 'lobby', part: 1, stop: 3 })
    await renderInRouter(<Lobby host />, { routed: true, url: '/game/abc' })
    const done = await screen.findByRole('button', { name: 'Done' })
    act(() => done.click())
    expect(pageTourStore.getState().tour).toBeNull()
    await waitFor(() => expect(document.querySelector('[data-slot="page-tour"]')).toBeNull())
  })
})
