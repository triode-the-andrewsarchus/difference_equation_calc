# AGENTS.md — Development Guidelines for AI Agents

## Project Overview

`difference_equation_calc` is a native Linux desktop application built in Rust and `egui`/`eframe`, inspired by Bret Victor's 2013 talk *"Media for Thinking the Unthinkable"*.

### Core Design Philosophy
- **The central object is BEHAVIOR**: a finite history of the state of a discrete dynamical system.
- **Never turn this into a node editor or modal graph plotter.**
- **No "Run" buttons**: everything must remain live, reactive, and continuously linked.
- **Direct manipulation**: equations, numeric constants, parameters, selections, search queries, and parameter sweeps must be interactive and update downstream state immediately.
- **Target Platform**: Strictly **Linux desktop (Wayland native)**. Do **not** add Android, mobile, or web dependencies.

---

## Architecture Boundaries

The codebase strictly enforces clean separation between the mathematical core and the UI:

```
[Mathematical Core] (Independent of egui, 100% testable)
├── ast.rs          AST expressions, operators, state references, spans
├── lexer.rs        Logos-based compile-time lexer
├── parser.rs       Pratt parser for expressions and recurrence equations
├── deps.rs         Topological sorting and same-step cycle detection
├── eval.rs         Expression evaluation & builtin functions
├── sim.rs          Dynamical simulation engine & failure localization
├── query.rs        Behavioral search over time & region extraction
├── measure.rs      Measurements on selections
├── sweep.rs        Parameter sweep vs measurement
└── document.rs     Document state, undo/redo, serde serialization

[UI Layer] (egui / eframe)
└── ui/
    ├── mod.rs           Main application loop and panel layout
    ├── waveforms.rs     Custom Painter rendering of aligned waveforms & cursor
    ├── equation_view.rs Equation list editor with scrubbable numbers
    ├── search_bar.rs    Behavioral search query bar & presets
    ├── params_view.rs   Parameter sliders and range controls
    ├── measure_view.rs  Selection measurement cards
    └── sweep_view.rs    Interactive 2D measurement vs parameter plot
```

### Invariants to Maintain
1. **Mathematical Core Independence**: Never import `egui`, `eframe`, or rendering types into the core modules (`ast`, `lexer`, `parser`, `deps`, `eval`, `sim`, `query`, `measure`, `sweep`, `document`).
2. **Parameter Sweep Non-Mutation**: Parameter sweeps must clone parameters and must **never** mutate the document's active parameter state during the sweep execution.
3. **Recurrence Semantics**:
   - Same-step instantaneous dependencies (e.g. `b[n+1]` referencing `a[n+1]`) must be resolved via topological sort.
   - Instantaneous cycles (e.g. `a[n+1] = b[n+1]` and `b[n+1] = a[n+1]`) are errors.
   - Delayed cycles (e.g. `a[n+1] = b[n]` and `b[n+1] = a[n]`) are completely legal.
4. **Lexer & Parser**: Use `logos` (`src/lexer.rs`) for tokenization and `nom` combinators (`src/parser.rs`) for expression and equation parsing. Do not replace them with fragile ad-hoc string regexes.
5. **UI Rendering**: Waveforms, selection bands, search overlays, and cursors are drawn using custom egui `Painter` primitives for precise alignment and maximum performance.

---

## Verification Commands

Always run the following commands before completing any task:

```bash
# 1. Run unit test suite
cargo test

# 2. Run Clippy across all targets
cargo clippy --all-targets

# 3. Verify formatting
cargo fmt --check

# 4. Build release binary
cargo build --release
```
