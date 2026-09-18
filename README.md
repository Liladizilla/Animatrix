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

The repository is in a working implementation phase. The core foundation is stable and validated:

- project and channel persistence works through SQLite-backed local state
- workflow selection and app reload behavior are stable
- asset metadata and provenance hashing are in place
- lifecycle event logging is present for traceability
- job transitions exist for queued, running, completed, and failed states
- the workspace test suite passes across all implemented crates

## Local-first architecture

Animatrix is designed as a durable creative workstation for AI-assisted video production:

- Channels represent brand or studio lanes with identity and visual style
- Projects represent production workstreams attached to a channel
- Assets capture generated or source media with metadata and hash provenance
- Jobs track execution context for scene generation and render/export work
- Events provide a project timeline for debugging, auditing, and workflow traceability
- Storage persists the local state under the user's home directory in `.animatrix/data/animatrix.db`

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
