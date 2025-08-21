# TRIAD — physics‑driven consensus

TRIAD is a project to build a high‑performance, energy‑efficient distributed network. The core idea is a physics‑inspired consensus: interference of event “waves” in a graph (DAG) field yields fast and robust decisions without heavy rounds of a classical protocol.

> TRIAD consensus isn’t a protocol — it’s physics.

## Key ideas
- **Interference instead of a protocol.** The decision is the maximum of transaction “wave” interference in the graph field.
- **Parallelism by default.** DAG + batched signature checks, vectorization, and parallel stages.
- **Minimal network rounds.** Compact messages ("waves") and aggregates instead of heavy voting.
- **Stability analytics.** `error_analysis/` modules and semantic heuristics for diagnostics and stabilization.

## Project status
- Rust codebase under active development; demo implementations for interference/consensus and visualization utilities are available.
- Local CPU metrics (single machine, indicative):
  - Ed25519 batch verify (~500): single to tens of milliseconds.
  - Interference + decision: tens of milliseconds per round.
  - Note: these are local estimates, not “mainnet” numbers; they depend on hardware, batch size, and parallelism.

## Quick start
Requirements: Rust (stable), Cargo.

Build and run the consensus demo:
```bash
cargo build --release
RAYON_NUM_THREADS=4 TRIAD_WAVE_SIG_BATCH=500 \
  cargo run --release --example quantum_consensus_demo
```
Useful environment variables:
- `RAYON_NUM_THREADS` — CPU worker count for parallel sections.
- `TRIAD_WAVE_SIG_BATCH` — batch size for wave signature checks.

Where to look for logs: the example `examples/quantum_consensus_demo.rs` prints steps, wave statistics, and final results.



## Repository structure
```
TRIAD/
├── Cargo.toml
├── src/
│   ├── dag/
│   ├── error_analysis/
│   ├── github/
│   ├── network/
│   └── ...
├── examples/
│   └── quantum_consensus_demo.rs
└── README.md
```
Key areas:
- `src/network/` — networking, errors, transmission analysis.
- `src/dag/` — DAG structures and operations.
- `src/error_analysis/` — semantics/diagnostics.
- `examples/quantum_consensus_demo.rs` — interference‑based consensus demo.

## Parameters and reproducibility
- Pin `RAYON_NUM_THREADS`, batch size, and number of “waves” for comparable results.
- Profiling: `--release`, `perf`/`dtrace`/`Instruments`; benchmark scripts with a fixed seed.

## Roadmap
- SIMD/AVX optimizations for the interference core; GPU backend exploration.
- Networking heuristics for wave propagation and gossip aggregation.
- Better diagnostics and stability (semantics/errors).
- More examples and visualizations.

## Contributing
PRs/issues are welcome. Please read `CONTRIBUTING.md`.

## License
MIT — see `LICENSE`.

## Contacts
- Author: fillay
- Email: keshashel@gmail.com
- GitHub: https://github.com/fillay12321
