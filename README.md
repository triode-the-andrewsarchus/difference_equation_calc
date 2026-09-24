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

8. **Canonical Dynamical Systems Gallery (Presets)**
   - Instant switcher menu with 6 classic discrete systems:
     - **Harmonic Oscillator (Bret Victor Demo)**: Coupled oscillator with same-step dependency, comparator, and integrator.
     - **Van der Pol Oscillator**: Non-linear damping producing isolated limit cycle attractors.
     - **Logistic Map**: Classic quadratic map exhibiting period-doubling cascade into chaos.
     - **Hénon Strange Attractor**: 2D discrete chaotic dynamical system with fractal attractor.
     - **Lotka-Volterra (Predator-Prey)**: Coupled non-linear ecological population cycles.
     - **PID Feedback Controller**: Closed-loop setpoint tracking, latency, and overshoot.
     - **Chamberlin State Variable Filter (SVF)**: Simultaneous lowpass, bandpass, and highpass outputs with instantaneous same-step topological resolution.
     - **Resonant Biquad Lowpass (Direct Form II)**: 2nd-order audio biquad demonstrating step response overshoot, ringing, and cutoffs.
     - **Notch Filter (Interference Rejection)**: Unit circle zero pair eliminating unwanted harmonic frequencies.

9. **Ghost Behavior Comparison & Diffing**
   - Click "Pin Ghost" to snapshot the current system behavior as a reference trace.
   - Pinned ghost curves are drawn faintly underneath live waveforms and in the phase portrait.
   - Measurement cards automatically display quantitative deltas ($\Delta = \text{live} - \text{ghost}$ and percentage changes).
   - Synchronized cursor readout shows live and ghost values side-by-side.

10. **Phase Portrait / State Space View ($y[n]$ vs $x[n]$)**
    - Interactive 2D parametric state space trajectory plotting any two chosen variables.
    - Synchronized with waveform timeline: the active cursor $n$ appears as a glowing orbit marker with coordinates HUD.
    - Active time selection highlights the corresponding orbit arc while dimming unselected portions.
    - Bidirectional interaction: clicking or dragging on the phase canvas scrubs the waveform cursor $n$ live.

11. **Inline Term Evaluation at Cursor $n$**
    - Moving the timeline cursor $n$ displays concrete numeric evaluations of all referenced variables and parameters directly beneath each equation.
    - Bridges the gap between symbolic definition and temporal consequence.

12. **Bifurcation / Attractor Sweep Mode**
    - Switch parameter sweep between scalar measurements and attractor point clouds.
    - Discards transient steps and plots asymptotic attractor states across parameter values (e.g. generating the Feigenbaum period-doubling bifurcation tree for the logistic map).
    - Dragging along the bifurcation plot updates the parameter live and dynamically shifts the system between fixed points, cycles, and chaos.

13. **Interactive Digital Filter Designer (Z-Domain & Audio DSP)**
    - Direct manipulation Z-domain pole-zero unit circle canvas: draggable conjugate pole pair (`X` in coral) and zeros (`O` in cyan).
    - Direct physical mapping: moving poles inside the unit disc interactively recalculates cutoff frequency $f_c$ and resonance $Q$.
    - Live theoretical frequency response curve $|H(e^{j\omega})|$ in decibels ($-60\text{ dB} \dots +18\text{ dB}$) with $-3\text{ dB}$ cutoff line and hover frequency/gain crosshair.
    - Dual filter architectures:
      - Direct Form II 2nd-order Biquad (RBJ Audio Cookbook coefficients $b_0, b_1, b_2, a_1, a_2$).
      - Chamberlin State Variable Filter (SVF) with live damping and resonance parameters.
    - Stimulus signal selector: Impulse $\delta[n]$, Unit Step $u[n]$, Sine + High-Frequency Noise, Chirp Frequency Sweep, and Square Wave Train.
    - Continuous live document link: dynamically injects synthesized difference equations and parameters into the live simulation workspace with zero-latency preview.

14. **Error Localization**
    - Arithmetic failures (division by zero, negative square root, NaN, divergence) are captured with the exact timestep $n$ where failure occurred and displayed both on the timeline and inline.
    - Equation parse and dependency errors are shown directly beneath the corresponding equation.

15. **Undo/Redo & Save/Load**
    - Full history checkpointing with Ctrl+Z and Ctrl+Y.
    - Session serialization to/from human-readable JSON files via `rfd` dialogs.

---

## Architecture

The mathematical core has zero GUI dependencies and is independently unit-tested:

```
src/
├── ast.rs          # AST nodes: Expr, BinaryOp, UnaryOp, StateRef, Span
├── lexer.rs        # Compile-time Logos DFA tokenizer with byte span tracking
├── parser.rs       # Nom-based parser combinators for recurrence equations & queries
├── deps.rs         # Dependency graph, cycle detection, topological sorting
├── eval.rs         # Expression evaluator, builtin math functions & term inspection
├── sim.rs          # Dynamical system simulator & failure localization
├── query.rs        # Behavioral search & contiguous region extraction
├── measure.rs      # Measurements engine (mean, rms, delta, duration, etc.)
├── sweep.rs        # Parameter vs measurement & attractor bifurcation sweeps
├── presets.rs      # Canonical discrete dynamical systems catalog
├── filter.rs       # Complex arithmetic, biquad synthesis, SVF model, Z-plane roots & Bode eval
├── document.rs     # Document model, undo/redo history, serde serialization
├── ui/
│   ├── mod.rs             # eframe app orchestration & top-level view
│   ├── icons.rs           # Embedded vector SVG icons (no emojis)
│   ├── filter_designer.rs # Interactive Z-plane pole-zero & Bode plot filter designer window
│   ├── waveforms.rs       # Aligned custom Painter waveforms, cursor, ghost traces
│   ├── equation_view.rs # Equation editor with live term evaluation at step n
│   ├── search_bar.rs    # Behavioral search bar, presets, region navigation
│   ├── params_view.rs   # Parameter sliders and scrubber controls
│   ├── measure_view.rs  # Selection measurement cards with ghost diffs
│   ├── phase_view.rs    # 2D parametric state space trajectory canvas
│   └── sweep_view.rs    # 2D plot supporting measurement & bifurcation sweeps
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
