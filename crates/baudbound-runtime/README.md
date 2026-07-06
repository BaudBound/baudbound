# baudbound-runtime

Script runtime and execution engine.

Current implementation:

- Parses exported `program.json` graph data into runtime nodes and edges
- Starts execution from the manual trigger, or from a selected trigger node with trigger payload output data
- Executes `action.log`
- Executes `action.calculate` with numeric expressions, exponentiation, grouping, and `round`, `floor`, `ceil`, `min`, `max`, `random`
- Executes `runtime.set_variable` operations:
  - set
  - increment
  - append list
  - set object field
  - clear
- Executes `action.delay`
- Dispatches external action nodes through `RuntimeActionHandler`
- Executes graph control flow:
  - `control.if`
  - `control.switch`
  - `control.loop`
  - `control.while`
  - `control.for_each`
- Expands `{{variable}}` references in supported node configs
- Exposes trigger payload object fields as `{{trigger-node-id.field}}` references
- Maintains derived variable metadata such as `name.$length` and `name.$count`

Planned responsibilities:

- Execution plans
- Action dispatch

