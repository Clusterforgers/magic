/**
 * The name a guest last joined under, kept in `localStorage` so the next join starts with it filled in. Every access can
 * fail (blocked storage, private windows, a full quota) and the page works without it.
 */
import type { KeyValueStore } from '@/lib/invites'
import { MAX_NAME_CHARS } from '@/lib/limits'

const KEY = 'magic.guestName'

function localStore(): KeyValueStore | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

/** The remembered name; empty when none was kept or the storage cannot be read. Never longer than a name may be. */
export function loadGuestName(store: KeyValueStore | null = localStore()): string {
  try {
    const kept = store?.getItem(KEY)
    return typeof kept === 'string' ? [...kept].slice(0, MAX_NAME_CHARS).join('') : ''
  } catch {
    return ''
  }
}

/** Keeps `name` for the next join; false when the browser refused. */
export function saveGuestName(name: string, store: KeyValueStore | null = localStore()): boolean {
  try {
    store?.setItem(KEY, name)
    return store !== null
  } catch {
    return false
  }
}
