# Difference Equation Calculator — Interacting with Behavior

A native Linux (Wayland-first) desktop application inspired by Bret Victor's "Interacting with Behavior" difference-equation prototype from his 2013 talk *"Media for Thinking the Unthinkable"*.

In this tool, the central object of manipulation is **BEHAVIOR**: the finite history of the state of a discrete dynamical system. Rather than treating this as a static equation editor with detached plots, equations immediately generate live behavior that can be inspected, selected, searched, measured, and explored across parameter values.

```
    equations + parameters
             ↓
         simulation
             ↓
          behavior
        ↙    ↓    ↘
    inspect select search
              ↓
           measure
              ↓
      vary a parameter
              ↓
    measurement vs parameter
```

---

## First Milestone: Victor Oscillator Demo

The application comes preconfigured to run and explore Victor's oscillator system:

```text
a[n+1] = a[n] - k * b[n]
b[n+1] = b[n] + k * a[n+1]
c[n+1] = cond(a[n] > 0, 1, -1)
d[n+1] = d[n] + 0.08 * c[n]
```

with initial conditions:

```text
a[0] = 1.0
b[0] = 0.0
c[0] = 0.0
d[0] = 0.0
```

and default parameter $k = 0.094$.

All equations, search queries, and parameter sweeps remain live and continuously linked with no "Run" button.

---

## Features

1. **Live Recurrence Evaluation with Topological Ordering**
   - Forward discrete difference equations evaluated step-by-step.
   - Same-step instantaneous dependencies (such as `b[n+1]` depending on `a[n+1]`) are topologically sorted and evaluated in order.
   - Delayed cycles (e.g. `a[n+1] = b[n]`, `b[n+1] = a[n]`) are fully legal.
   - Instantaneous cycles (e.g. `a[n+1] = b[n+1]`, `b[n+1] = a[n+1]`) are identified and localized with clear cycle path error reporting.

2. **Synchronized Multi-Row Waveform Canvas**
   - Custom egui `Painter` rendering for crisp, antialiased, responsive waveforms.
   - Shared horizontal time axis across all variables.
   - Hovering anywhere produces a cross-row vertical cursor displaying $n$ and the state of each variable (`a[n]`, `b[n]`, `c[n]`, `d[n]`).
   - Horizontal pan (middle/right-click drag) and zoom (scroll wheel).

3. **Behavioral Search Engine**
   - Real-time search across the entire behavior using Boolean expressions:
     - `a[n+1] > a[n]`
     - `a[n] > 0`
     - `abs(a[n]) < 0.1`
     - `b[n] > 0 and a[n] < 0`
     - `c[n] == 1`
   - Highlights matching samples and contiguous matching regions across **all** waveform rows.
   - Live match statistics (sample count, percentage, region count) with quick buttons to select and measure matching regions.

4. **Horizontal Behavior Selection & Measurements**
   - Dragging horizontally across the waveform area selects a time range spanning the entire dynamical system.
   - Displays $n_{\text{start}}$, $n_{\text{end}}$, and duration $\Delta n$.
   - Live measurement cards for each variable:
     - Mean
     - Min
     - Max
     - Range ($\text{Max} - \text{Min}$)
     - Sum
     - RMS (Root Mean Square)
     - First
     - Last
     - Delta ($\text{Last} - \text{First}$)
     - Duration ($\Delta n$)

5. **Scrubbable Numeric Literals & Parameter Promotion**
   - Numeric literals inside equations can be directly dragged horizontally to scrub their values in real time.
   - Modifiers: Shift for coarse adjustment, Ctrl/Cmd for fine adjustment.
   - Single-click "→param" button promotes any constant into a named parameter with slider and range controls.

6. **Measurement vs Parameter Exploration Plot**
   - Sweeps a parameter (e.g. $k$) across a range of values and plots any selected measurement (e.g. $\text{Mean}(a)$ or $\Delta n$) vs the parameter.
   - The current parameter value appears as a glowing interactive point on the curve.
   - Dragging this point sweeps the parameter live and immediately updates the system's waveforms in real time.

7. **Error Localization**
   - Arithmetic failures (division by zero, negative square root, NaN, divergence) are captured with the exact timestep $n$ where failure occurred and displayed both on the timeline and inline.
   - Equation parse and dependency errors are shown directly beneath the corresponding equation.

8. **Undo/Redo & Save/Load**
   - Full history checkpointing with Ctrl+Z and Ctrl+Y.
   - Session serialization to/from human-readable JSON files via `rfd` dialogs.

---

## Architecture

The mathematical core has zero GUI dependencies and is independently unit-tested:

```
src/
├── ast.rs          # AST nodes: Expr, BinaryOp, UnaryOp, StateRef, Span
├── lexer.rs        # Compile-time Logos DFA tokenizer with byte span tracking
├── parser.rs       # Pratt parser for recurrence equations & boolean expressions
├── deps.rs         # Dependency graph, cycle detection, topological sorting
├── eval.rs         # Expression evaluator & builtin math/cond functions
├── sim.rs          # Dynamical system simulator & failure localization
├── query.rs        # Behavioral search & contiguous region extraction
├── measure.rs      # Measurements engine (mean, rms, delta, duration, etc.)
├── sweep.rs        # Parameter vs measurement sweep
├── document.rs     # Document model, undo/redo history, serde serialization
├── ui/
│   ├── mod.rs           # eframe app orchestration & top-level view
│   ├── waveforms.rs     # Aligned custom Painter waveforms, cursor, selection
│   ├── equation_view.rs # Equation editor with scrubbable numbers & promote-to-param
│   ├── search_bar.rs    # Behavioral search bar, presets, region navigation
│   ├── params_view.rs   # Parameter sliders and scrubber controls
│   ├── measure_view.rs  # Selection measurement cards
│   └── sweep_view.rs    # Interactive measurement vs parameter 2D plot
└── main.rs         # Wayland desktop native entry point
```

---

## Building and Running

Prerequisites: Rust 1.80+ and standard Linux desktop development libraries (e.g., Wayland / X11 / libglvnd).

### Run Application
```bash
cargo run --release
```

### Run Unit Tests
```bash
cargo test
```

### Run Clippy & Format
```bash
cargo clippy --all-targets
cargo fmt --check
```

---

## Keyboard & Mouse Shortcuts

- **Left Mouse Drag (on waveform)**: Create / adjust time selection range across all rows.
- **Middle / Right Mouse Drag (on waveform)**: Pan timeline horizontally.
- **Scroll Wheel (on waveform)**: Zoom in / out horizontally around cursor.
- **Left Mouse Drag (on parameter sweep plot)**: Scrub parameter live by dragging along the measurement curve.
- **Left Mouse Drag (on scrub chip)**: Drag numeric constant in equation (hold Shift for coarse, Ctrl for fine).
- **Escape**: Clear current time selection.
- **Ctrl+Z**: Undo last state change.
- **Ctrl+Y / Ctrl+Shift+Z**: Redo last undone state change.
