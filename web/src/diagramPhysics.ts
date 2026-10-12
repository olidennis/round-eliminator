import { forceCollide, forceLink, forceManyBody, forceSimulation, type SimulationNodeDatum } from 'd3-force'

export interface PhysicsNode extends SimulationNodeDatum {
  id: string
  x: number
  y: number
  radius: number
}
export type PhysicsLink = { source: PhysicsNode; target: PhysicsNode }

/** Local repulsion and edge springs, without gravity or component packing.
 * A distant unconnected node has no forces, so dragging it leaves it in place.
 */
export function createDiagramPhysics(nodes: PhysicsNode[], links: PhysicsLink[]) {
  return forceSimulation<PhysicsNode, PhysicsLink>(nodes)
    .alphaDecay(.08)
    .velocityDecay(.5)
    .force('repulsion', forceManyBody<PhysicsNode>().strength(-140).distanceMax(180))
    .force('collision', forceCollide<PhysicsNode>(node => node.radius).iterations(3))
    .force('links', forceLink<PhysicsNode, PhysicsLink>(links)
      .distance(link => link.source.radius + link.target.radius + 30).strength(.35))
}
