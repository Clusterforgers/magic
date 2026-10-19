/** A one-line notice that outlives the page that raised it, such as "You left the game." on the page the visitor lands on. */
import { createStore } from 'zustand'

/** The notice now on screen, if any; `id` changes with every `showFlash`, so the same words twice restart the timer. */
export interface FlashState {
  message: string | null
  id: number
}

/** The one notice of the app; `FlashNotice` in the shell draws it. */
export const flashStore = createStore<FlashState>(() => ({ message: null, id: 0 }))

/** Shows `message` on whichever page is open, replacing an earlier notice. */
export function showFlash(message: string): void {
  flashStore.setState((state) => ({ message, id: state.id + 1 }))
}

/** Takes the notice away. */
export function clearFlash(): void {
  flashStore.setState({ message: null })
}
