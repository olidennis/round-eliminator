# Unimplemented feature inventory

Source: `../round-eliminator/www/gui.js` on `llm-enhanced`, with `api.js` and
`round-eliminator-lib/src/serial.rs` used to identify behavior. Every feature
below is **unimplemented** in `re3`; this is an inventory, not a design choice.
Buttons that share a label but perform different operations have separate entries.

## Problem entry, history, and sharing

| Feature | Short description |
| --- | --- |
| Active and Passive editors | Enter the two side constraints as text; lines describe allowed configurations. |
| Start | Parse a new problem in Rust and display the returned problem and any diagnostics. |
| Clear | Clear the displayed result history. |
| Edit | Copy a displayed problem back into the input editors. |
| Expand/collapse cards | Open or close sections of a displayed problem. |
| Close result, warning, or error | Remove an item from the displayed history. |
| New / Old / Both | Switch between current labels, source labels, or both in constraints. |
| All / Gen | Switch passive constraint display between all label sets and generators. |
| Export Link To Clipboard | Encode the current input and history in a shareable URL fragment. |
| Save | Download the current input and history as a `.re` file. |
| Load | Restore input and history from a `.re` file. |

## Operations and analyses

| Feature | Short description |
| --- | --- |
| Speedup | Apply one round-elimination step to a problem. |
| Speedup with Star Relaxation | Apply speedup using one-coordinate-star relaxation in the universal phase. |
| Inverse Speedup | Apply the inverse round-elimination transformation. |
| Maximize | Maximize the passive constraint, then recompute derived information. |
| Full Diagram | Compute the complete label-strength diagram without displaying a maximized passive side. |
| Merge (equivalent labels) | Merge labels identified as equivalent in the computed diagram. |
| All Different Labels | Split or distinguish labels that currently denote the same choices. |
| Delta Edge Coloring | Transform labels using an assumed degree-size edge coloring. |
| Speedup+Maximize | Run speedup followed by passive maximization. |
| Speedup+Maximize+Rename | Also rename the resulting labels by generators. |
| Rename by generators | Rename labels using generators of their diagram sets. |
| Rename | Apply names entered in the renaming table. |
| Remove `< >` | Strip generator brackets from proposed label names. |
| Fix Outdegree | Add an orientation of the specified outdegree and recompute related analyses. |
| Coloring | Compute zero-round solvability with a strong coloring, including hypergraphs. |
| Edge Coloring Solvability | Compute the feasible edge-color palettes and corresponding output label sets. |
| Apply Marks | Test whether Marks' technique yields a lower bound. |
| Logstar Reversible Relaxations | Find relaxations reversible in O(log* n) rounds. |
| Logstar Reversible Relaxations (old) | Run the earlier reversible-relaxation search. |
| Logstar Reversible Edge Additions | Search for separately verified edges that can be added reversibly. |
| Recursive logstar reversible edge additions | Repeatedly apply verified reversible edge additions. |
| Reversible search limits | Configure the time limit and worker count for the native search. |
| Verify and apply | Recheck a reversible-edge certificate and apply its proposed additions. |
| Show reverse mapping | Expand a certificate's annotated-context to output mapping. |
| Copy certificate | Copy the reversible-edge certificate as JSON. |
| Add Active Predecessors | Add predecessor labels or configurations on the active side. |
| Add Active Pred & Flip | Add active predecessors and flip the problem sides. |
| Remove Trivial Lines | Remove configurations deemed trivial. |

## Manual simplification and hardening

| Feature | Short description |
| --- | --- |
| From diagram selection (Simplify) | Fill the source and target labels from a selected diagram edge. |
| Merge (Simplify) | Merge one selected label into another as a relaxation. |
| Add Arrow | Add a selected relation to the diagram as a simplification. |
| Remove (Harden) | Remove a chosen label, optionally replacing it with predecessors. |
| From diagram selection (Group Simplify) | Select a label group from highlighted diagram nodes. |
| Merge (simplify) | Merge a selected group of labels into a target label. |
| Unlabeled SubDiagram preset button | Fill the subdiagram editor with a built-in merge recipe. |
| Merge (SubDiagram) | Apply the typed subdiagram merge recipe; optionally recompute the diagram. |
| From diagram selection (Group Harden) | Select labels to harden from highlighted diagram nodes. |
| Remove (harden) | Remove the selected group of labels. |
| Keep (harden) | Keep only the selected group of labels. |
| Replace With Predecessors | Choose whether hardening substitutes predecessor labels. |
| Harden (Critical Sets) | Harden using critical sets, with coloring and search-step options. |
| Relax (Critical Sets) | Relax using critical sets, with coloring and search-step options. |

## Fixed-point tools

| Feature | Short description |
| --- | --- |
| Partial Fixpointing | Restrict fixed-point generation to a selected subset of labels. |
| Only determine triviality | Check triviality during fixed-point search without constructing the full result. |
| From diagram selection (Fixed Point) | Select partial-fixpoint labels from the strength diagram. |
| Basic | Generate a fixed-point candidate using the default diagram. |
| Loop | Generate a candidate while automatically repairing the default diagram. |
| Make fixed point good | Build a nontrivial lattice-based fixed-point relaxation; native only in the old UI. |
| Generate Default Diagram | Build the default fixed-point diagram, with optional extra arrows. |
| Generate Larger Default Diagram | Build the larger default diagram variant. |
| Generate Fixed Point (custom diagram) | Use a user-edited diagram to generate a fixed point. |
| Find best add arrow | Search for a useful diagram arrow to add. |
| Add from diagram selection (duplication) | Add selected diagram labels to a duplication group. |
| `?` duplication help | Show guidance for choosing labels to duplicate. |
| Delete (duplication group) | Remove one selected duplication group. |
| Track Expressions | Include tracked expressions in the duplication result. |
| Generate Fixed Point (duplication) | Build a candidate after duplicating chosen label groups. |

## Automated bounds, inputs, and duality

| Feature | Short description |
| --- | --- |
| Automatic Lower Bound | Search through problem transformations for a lower bound or fixed point. |
| Automatic Upper Bound | Search for an upper-bound sequence of transformations. |
| Bound search options | Set coloring assumptions, maximum labels, branching, and maximum steps. |
| Check (Zero-Round Solvability with Input) | Decide zero-round solvability when a typed input problem is provided. |
| Reverse Check | Run the zero-round input check in the reverse direction. |
| Input-check options | Choose SAT solving and smallest obstructing subinput search. |
| Dual | Compute a dual relative to a typed fixed-point problem. |
| Double Dual | Compute the dual twice using the fixed-point procedure. |
| Smallest Dual | Search for a smallest dual relative to the supplied fixed point. |
| Double Dual (diagram route) | Compute a double dual using a fixed-point problem or its diagram. |
| Make Label Different | Apply a selected-label log-star upper-bound transformation. |
| Get Label of Other Side | Reveal or encode the label at the opposite side. |
| MIS | Apply the MIS-based upper-bound transformation. |
| Automatic Logstar Upper Bound | Search for a log-star upper bound with label, depth, and size limits. |
| Just yes/no | Run that search as a Boolean decision instead of returning a sequence. |

## Information displayed for a computed problem

| Feature | Short description |
| --- | --- |
| Constraint tables | Show active and passive allowed configurations, grouped labels, powers, and stars. |
| Label count | Show the number of labels in the current problem. |
| Zero-round solvability | Report solvable or unsolvable and show witnessing label sets when available. |
| Coloring solvability | Report whether a suitable given coloring gives a zero-round solution and show color sets. |
| Edge-coloring solvability | Report maximal/unbounded feasible palettes and output sets for input colors. |
| Orientation zero-round solvability | Report solvability under a given orientation and show witness pairs. |
| Orientation coloring solvability | Report coloring solvability under a given orientation and show witness pairs. |
| Mergeable labels | List groups of labels that can be merged. |
| Logstar reversible relaxations | Display reversible label groups and labels that must be removed. |
| Fixed-point procedure status | Report whether a nontrivial fixed-point relaxation was obtained. |
| Marks technique status | Report whether Marks' technique gives a lower bound. |
| Zero-round solvability with input | Report the result and show a witnessing input-to-output mapping. |
| Tracked expressions | Display expressions recorded during fixed-point construction. |
| Renaming table | Show each current label and its source-label set. |
| Label mapping table | Show each original label's result labels or removal. |
| Strength diagram | Draw directed label relations and allow node and edge selection. |
| Physics / Hierarchical | Toggle force layout and hierarchical diagram layout. |
| Export to Clipboard (diagram) | Copy diagram nodes and arrows as text. |
| Reversible-edge report | Show search statistics, verified alternatives, recipes, and mappings. |
| Action and sequence history | Display each transformation and nested automatic-search sequence. |
| Warnings and errors | Display computation diagnostics. |
| Progress and stop | Show computation status or a progress bar and offer cancellation. |
