import assert from 'node:assert/strict'
import { createDiagramPhysics } from '../src/diagramPhysics.ts'

function settle(nodes, links = []) {
  const simulation = createDiagramPhysics(nodes, links).stop()
  simulation.tick(150)
}
const node = (id, x, y) => ({ id, x, y, radius: 25 })

// M is disconnected in the bipartite matching diagram; settling P--U must not
// pull it back, pack components, or add a global translation.
const m = node('M', 1000, 500), p = node('P', 0, 0), u = node('U', 100, 0)
settle([m, p, u], [{ source: p, target: u }])
assert.deepEqual([m.x, m.y], [1000, 500])
assert.ok(Math.hypot(p.x - u.x, p.y - u.y) > p.radius + u.radius)

// Different components must still repel when they are close or overlapping.
const a = node('isolated', 0, 0), b = node('other', 4, 4)
settle([a, b])
assert.ok(Math.hypot(a.x - b.x, a.y - b.y) >= a.radius + b.radius - .01)

// No edges, no nearby neighbors: nothing should rearrange the user's positions.
const left = node('left', -500, 0), right = node('right', 500, 0)
settle([left, right])
assert.deepEqual([left.x, left.y, right.x, right.y], [-500, 0, 500, 0])
settle([])
console.log('Physics checks passed: distant disconnected nodes stay put; nearby components separate without gravity or packing.')
