I would like to refactor the library code with the following principles:
- The crate's focus will be on the `api` module and the `ev_cost_recovery` GUI binary.
- Most sub-modules of the top-level modules will be pure, i.e., will only contain types and pure functions, i.e., no state, no I/O.
- Non-pure functions will be in sub-modules that focus on I/O. Such modules may contain helper pure functions that don't support the pure modules.
- A feature "historic" will be defined to gate code that will survive but does not support the `api` module or the `ev_cost_recovery` binary.
- Any library code that does not support the API will be identified and:
  - If the code does not support any binary, it should be marked for deletion.
  - If the code supports a binary but not the API or `ev_cost_recovery` GUI app, it should be gated with the new feature "historic".
- It would be nice to have a way to segregate the "historic" binaries, i.e., those that depend on feature "historic".

