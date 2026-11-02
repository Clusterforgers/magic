/** Keeps the pace preferences on the signed-in account: loaded at sign-in, saved on every change, local when signed out. */
import { useEffect } from 'react'
import type { PrefsApi } from '@/net/prefs'
import { prefsApi } from '@/net/prefs'
import type { FlowPrefs } from './flowPrefs'
import { attachAccount, detachAccount } from './flowPrefs'

/**
 * While `enabled` (an account is signed in), loads the account's preferences and makes them the ones in use; every
 * later change is saved to the server, the latest last. A load that fails leaves this browser's preferences in place.
 * When `enabled` ends, the preferences return to this browser's storage.
 */
export function usePrefsSync(enabled: boolean, api: PrefsApi = prefsApi): void {
  useEffect(() => {
    if (!enabled) return
    let stale = false
    let queue: Promise<unknown> = Promise.resolve()
    const save = (prefs: FlowPrefs): void => {
      queue = queue.then(() =>
        api
          .save({ default_stops: [...prefs.defaultStops], end_turn_by_hand: prefs.endTurnByHand })
          .catch(() => {
            // The change stays for this page; the next one tries again.
          }),
      )
    }
    api.load().then(
      (loaded) => {
        if (stale) return
        attachAccount({ defaultStops: loaded.default_stops, endTurnByHand: loaded.end_turn_by_hand }, (prefs) => {
          if (!stale) save(prefs)
        })
      },
      () => {
        // Without the server's copy this browser's preferences stay in use.
      },
    )
    return () => {
      stale = true
      detachAccount()
    }
  }, [enabled, api])
}
