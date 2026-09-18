# Animatrix

Animatrix is a Rust-first local studio for AI-assisted video production. It is designed as a durable creative workstation where channels, projects, assets, jobs, and event history stay locally persisted while a creator moves from concept to script to render.

## Product vision

Animatrix is built for a simple local-first production flow:

- create a brand or channel
- launch a project within that channel
- assemble scripts, story beats, and media assets
- queue generation and render tasks
- review output history and project timeline
- keep everything stored locally for fast iteration

This makes the tool useful for creators who want to iterate privately without depending on a fragile cloud pipeline for every step.

## Core architecture

The system is intentionally split into Rust crates to keep the domain model, workflow logic, and persistence cleanly separated:

- `animatrix-core`: shared IDs, app errors, and base enums
- `animatrix-domain`: primary studio domain models such as channels, projects, scripts, and brand profiles
- `animatrix-project`: orchestration for project and channel creation, render completion, and workflow mutation
- `animatrix-storage`: SQLite-backed persistence with durable local state
- `animatrix-assets`: asset metadata, provenance, and media tracking
- `animatrix-events`: event log and workflow timeline traceability
- `animatrix-jobs`: queued job lifecycle management and worker execution flow
- `animatrix-ai`: provider abstraction and model-task routing
- `animatrix-ui`: dashboard shell, workflow view, and project interaction layer

## Current implementation status

The repo is in a working product foundation stage with real structure behind it:

- channel and project persistence works through SQLite-backed local state
- workflow selection and reload behavior are stable
- asset metadata and provenance hashing are in place
- lifecycle event logging is present for auditability
- job transitions exist for queued, running, completed, and failed states
- provider resolution is no longer a single local-only stub
- the workspace test suite passes across implemented crates

## Workflow model

The product is designed around a layered creative pipeline:

1. Brand and visual identity
2. Project creation and planning
3. Script/storyboard generation
4. Asset generation and review
5. Render/export job queue
6. Final output timeline and event history

This gives the studio a realistic creative workflow rather than a flat single-screen prototype.

## Local-first persistence

Animatrix stores its primary state under the local user profile in `.animatrix`, keeping the project durable and offline-friendly:

- SQLite database path: `.animatrix/data/animatrix.db`
- project-specific outputs in local data folders
- timeline and event history preserved locally
- provider selection and queue state retained across runs

## Demo / screen capture

The project includes a running product walkthrough that demonstrates the dashboard, workflow flow, and project interactions.

Embedded demo video:

```html
<video controls muted playsinline width="100%">
  <source src="docs/media/animatrix-demo.mp4" type="video/mp4" />
  Your browser does not support the video tag.
</video>
```

The actual recorded asset is committed under:

```text
docs/media/animatrix-demo.mp4
```

## Quick start

### Install dependencies

```bash
cargo build
```

### Run the CLI dashboard

```bash
cargo run -p animatrix-ui -- --cli
```

### Run the test suite

```bash
cargo test --quiet
```

## Workspace layout

```text
Animatrix/
├── Cargo.toml
├── crates/
│   ├── animatrix-ai/
│   ├── animatrix-assets/
│   ├── animatrix-core/
│   ├── animatrix-domain/
│   ├── animatrix-events/
│   ├── animatrix-jobs/
│   ├── animatrix-project/
│   ├── animatrix-storage/
│   └── animatrix-ui/
├── docs/
│   └── media/
│       └── animatrix-demo.mp4
├── README.md
├── target/
├── Cargo.lock
└── .gitignore
```

## Repository workflow

Meaningful product changes are committed and pushed to GitHub as the project matures. The repository is intended to reflect the actual working state of the local-first creative studio.
