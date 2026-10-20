import { FlashNotice } from '@/components/FlashNotice'

/** Stands in for Play in a routed test router: its heading, and the notice the shell would show under the header. */
export function PlayStub() {
  return (
    <>
      <FlashNotice />
      <h1>Play</h1>
    </>
  )
}
