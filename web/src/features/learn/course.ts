import { COMBAT } from './lessons/combat'
import { COMMANDER } from './lessons/commander'
import { HELP } from './lessons/help'
import { MANUAL } from './lessons/manual'
import { MANA } from './lessons/mana'
import { SEAT } from './lessons/seat'
import { STACK } from './lessons/stack'
import { TURN } from './lessons/turn'
import type { Lesson, LessonId } from './step'

/** The course in the order a new player takes it. */
export const COURSE: readonly Lesson[] = [SEAT, MANA, COMBAT, STACK, TURN, COMMANDER, HELP]

/** Whether `id` names a lesson of the course. */
export function isLessonId(id: string | null): id is LessonId {
  return COURSE.some((lesson) => lesson.id === id)
}

/** The lesson called `id`: one of the course, or the manual-panel lesson. */
export function lessonOf(id: LessonId | 'manual'): Lesson {
  if (id === 'manual') return MANUAL
  return COURSE.find((lesson) => lesson.id === id) ?? SEAT
}

/** The lesson after `id`, or null after the last. */
export function nextLesson(id: LessonId | 'manual'): Lesson | null {
  if (id === 'manual') return null
  const at = COURSE.findIndex((lesson) => lesson.id === id)
  return COURSE[at + 1] ?? null
}

/** The `sessionStorage` key of the step a lesson's game is at, so a reload returns to it. */
const cursorKey = (game: string): string => `lesson-step:${game}`

/** The step the lesson of game `game` was at in this tab, 0 when none was kept. */
export function loadCursor(game: string): number {
  try {
    const value = Number(window.sessionStorage.getItem(cursorKey(game)))
    return Number.isInteger(value) && value > 0 ? value : 0
  } catch {
    return 0
  }
}

/** Keeps the step the lesson of game `game` is at, in this tab. */
export function saveCursor(game: string, index: number): void {
  try {
    window.sessionStorage.setItem(cursorKey(game), String(index))
  } catch {
    // Storage refused: a reload starts the lesson's steps again.
  }
}
