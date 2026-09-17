# Animatrix

Animatrix is a Rust-first local studio for AI-assisted video production. It is designed as a local-first creative workflow where channels, projects, assets, jobs, and event history are all kept in a durable local state while the user works through scenes, story direction, and final render/export steps.

## Product direction

The platform is organized around a small set of core concepts:

- Channels: brand or studio lanes with identity and visual style
- Projects: production workstreams mapped to a channel
- Assets: source and output media plus metadata
- Jobs: queued, running, completed, or failed production tasks
- Events: workflow history for traceability and debugging

The system is intentionally split into Rust crates so the app stays structured and local-first:

- `animatrix-core`: shared IDs, app errors, and base enums
- `animatrix-domain`: primary domain models
- `animatrix-project`: project and channel creation helpers
- `animatrix-storage`: SQLite persistence for durable local state
- `animatrix-assets`: asset metadata and provenance handling
- `animatrix-events`: event log and workflow traceability
- `animatrix-jobs`: job lifecycle management
- `animatrix-ai`: model/provider abstraction
- `animatrix-ui`: terminal dashboard and workspace shell

## Current status

The repository is in an active implementation phase. The core foundation is stable and validated:

- project/channel persistence works
- workflow selection is preserved across reloads
- asset metadata support is in place
- event logging is present for lifecycle tracking
- job lifecycle transitions exist for queued/running/completed/failed states
- the project test suite passes for the implemented crates

## Running locally

### Build

```bash
cargo build
```

### Run the CLI dashboard

```bash
cargo run -p animatrix-ui -- --cli
```

### Run tests

```bash
git push
```

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
├── README.md
├── target/
└── .gitignore
```

## Notes

This project is intentionally local-first. It stores state under the user's home directory in `.animatrix`, with SQLite-backed persistence and a Rust-based workflow shell.

## Repository workflow

From here forward, meaningful changes will be committed and pushed to GitHub as they are added.
