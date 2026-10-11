import type { ProblemText } from './generated/ProblemText'

export type ProblemPreset = {
  name: string
  description: string
  definition: ProblemText
}

// These are editable text definitions, sent through the same Start request as
// user-written problems. Ordinary graphs use vertices as active nodes and
// edges as degree-two passive nodes.
const inferredDegrees = { active: '', passive: '' }
const edgeGrabbing = { active: 'G N^2', passive: 'N GN' }
const listInput = { active: 'r^3\ng^3\nb^3', passive: 'rgb^2' }
const coloring = { active: 'A^3\nB^3\nC^3', passive: 'A B\nA C\nB C' }

export const problemPresets: ProblemPreset[] = [
  {
    name: 'MIS',
    description: 'Maximal independent set on 3-regular graphs. M marks a selected vertex; each unselected vertex uses P toward a selected neighbor and U on its other edges.',
    definition: {
      kind: 'plain', degrees: inferredDegrees,
      constraints: { active: 'M^3\nP U^2', passive: 'M UP\nU U' },
    },
  },
  {
    name: 'Maximal matching',
    description: 'Maximal matching on 3-regular graphs. M marks a matching edge, U the other ports of a matched vertex, and P an unmatched vertex. Two unmatched vertices cannot be adjacent.',
    definition: {
      kind: 'plain', degrees: inferredDegrees,
      constraints: { active: 'M U^2\nP^3', passive: 'M M\nU UP' },
    },
  },
  {
    name: 'Bipartite maximal matching',
    description: 'Maximal matching on a bipartite graph of degree 3 on both sides. A matched node has one M edge; an unmatched node has only matched neighbors.',
    definition: {
      kind: 'plain', degrees: inferredDegrees,
      constraints: { active: 'M U^2\nP^3', passive: 'M UP^2\nU^3' },
    },
  },
  {
    name: '3-coloring (3-regular)',
    description: 'Proper vertex coloring with A, B, C on 3-regular graphs. Each vertex uses one color on every port; adjacent vertices have different colors.',
    definition: { kind: 'plain', degrees: inferredDegrees, constraints: coloring },
  },
  {
    name: 'Edge grabbing',
    description: 'On a 3-regular graph, each vertex grabs exactly one incident edge (G). An edge cannot be grabbed from both ends; N means not grabbed.',
    definition: { kind: 'plain', degrees: inferredDegrees, constraints: edgeGrabbing },
  },
  {
    name: 'Edge grabbing from sinkless orientation',
    description: 'Edge grabbing on a 3-regular graph, given a sinkless orientation: every vertex has an outgoing edge (O); I denotes an incoming edge.',
    definition: {
      kind: 'independent', degrees: inferredDegrees, output: edgeGrabbing,
      input: { active: 'O IO^2', passive: 'I O' },
    },
  },
  {
    name: 'Edge grabbing from sinkless and sourceless orientation',
    description: 'Edge grabbing on a 3-regular graph, given an orientation with both an incoming (I) and an outgoing (O) edge at every vertex.',
    definition: {
      kind: 'independent', degrees: inferredDegrees, output: edgeGrabbing,
      input: { active: 'I O IO', passive: 'I O' },
    },
  },
  {
    name: 'Sinkless and sourceless orientation from edge grabbing',
    description: 'Find an orientation with an incoming (I) and an outgoing (O) edge at every vertex of a 3-regular graph, given an edge-grabbing solution (G marks the grabbed edge).',
    definition: {
      kind: 'independent', degrees: inferredDegrees, input: edgeGrabbing,
      output: { active: 'I O IO', passive: 'I O' },
    },
  },
  {
    name: 'Edge grabbing from 3-edge coloring',
    description: 'Edge grabbing on a 3-regular graph, given a proper 3-edge coloring. Each vertex sees A, B, C once, and the two ends of each edge agree on its color.',
    definition: {
      kind: 'independent', degrees: inferredDegrees, output: edgeGrabbing,
      input: { active: 'A B C', passive: 'A A\nB B\nC C' },
    },
  },
  {
    name: 'List coloring (mapped)',
    description: 'Proper 3-coloring on a 3-regular graph with a two-color list at each vertex: input r forbids A, g forbids B, and b forbids C. The paired preset describes the same problem.',
    definition: {
      kind: 'mapped', degrees: inferredDegrees, input: listInput, output: coloring,
      mapping: 'r -> BC\ng -> AC\nb -> AB',
    },
  },
  {
    name: 'List coloring (paired)',
    description: 'The same list-coloring problem encoded by input/output pairs. Each vertex has one input and one output color; adjacent vertices have different output colors.',
    definition: {
      kind: 'paired', degrees: inferredDegrees,
      constraints: {
        active: '(r,B)^3\n(r,C)^3\n(g,A)^3\n(g,C)^3\n(b,A)^3\n(b,B)^3',
        passive: '(g,A)(b,A) (r,B)(b,B)\n(g,A)(b,A) (r,C)(g,C)\n(r,B)(b,B) (r,C)(g,C)',
      },
    },
  },
]
