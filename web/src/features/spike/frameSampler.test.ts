// Goal: frame statistics are computed correctly since they decide the framework spike verdict.
import { describe, expect, it } from 'vitest'
import { summarizeFrames } from './frameSampler'

describe('summarizeFrames', () => {
  it('returns zeros for no samples', () => {
    expect(summarizeFrames([])).toEqual({ frames: 0, meanMs: 0, p95Ms: 0, maxMs: 0, fps: 0 })
  })

  it.each([
    [[16, 16, 16, 16], 62.5, 16],
    [[10, 10, 10, 30], 66.667, 30],
  ])('%j gives %f fps and max %f ms', (frames, fps, max) => {
    const stats = summarizeFrames(frames)
    expect(stats.fps).toBeCloseTo(fps, 2)
    expect(stats.maxMs).toBe(max)
  })
})
