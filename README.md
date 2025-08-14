# tlk – Workspace Tool Locker

`tlk` keeps reproducible versions of auxiliary CLI tools (terraform, kubectl, helm, gh, buf, jq, etc.) beside your source code, installing them locally into `.tlk/bin` and locking exact download sources.

## Why

Problems this solves:
- Onboarding: clone repo, run one command, all required infra tools appear locally.
- Reproducibility: versions are declared & locked, not guessed from your global PATH.
- Isolation: nothing installed globally; per‑repository sandbox in `.tlk/bin`.

## Key Features
- Declarative `tlk.toml` with shorthand for popular tools.
- Exact lock file (`tlk.lock`) capturing resolved versions + URLs.
- Parallel downloads & idempotent re-runs (skips unchanged tools).
- Cross‑platform (Linux, macOS, Windows) with normalized `{os}` / `{arch}` placeholders.
- Shell hook to auto prepend the correct `.tlk/bin` when you cd into a repo.

## Install tlk (the manager)

From source (Rust toolchain required):
```bash
git clone <this repo>
cd tool_locker
cargo build --release
./target/release/tlk --help
```
Optionally place the built binary somewhere on your PATH (or use the shell hook below inside the repo itself).

## Quick Start (Using tlk in Your Project)

1. Add a `tlk.toml` at the repository root:
```toml
terraform = "^1.8.5"
kubectl = "1.30.2"
helm = "^3.15.2"

# Example custom / explicit form
[[tools]]
name = "protoc"
version = "24.4"
source = "https://github.com/protocolbuffers/protobuf/releases/download/v{version}/protoc-{version}-linux-x86_64.zip"
kind = "archive"
```
2. Install the tools (creates / updates `tlk.lock`):
```bash
tlk install
```
3. Add the shell hook so `.tlk/bin` appears in PATH automatically when you enter the project:
```bash
eval "$(tlk hook)"   # bash / zsh
```
4. Commit both `tlk.toml` and `tlk.lock`.

Now every collaborator runs `tlk install` once (or just relies on the hook if already locked) and gets consistent binaries.

## The Lock File
`tlk install` writes `tlk.lock` with the exact version (resolving any ranges like `^1.2.3`) and the fully rendered source URL for the current platform. Later installs verify versions, source templates, optional checksums, and binary digests. Use `tlk install --locked` in CI to assert nothing drifts.

## Common Commands
| Command | Purpose |
|---------|---------|
| `tlk plan` | Show planned tool sources (dry run) |
| `tlk install` | Install/update all tools & update lock |
| `tlk install terraform helm` | Install specific known tools (latest or range) |
| `tlk list` | Show desired vs installed versions |
| `tlk verify` | Verify lock + integrity without installing |
| `tlk hook` | Print shell hook script (eval it) |

Useful flags:
- `--locked` (with install) : only use versions already in `tlk.lock`.
- `--no-lock` : do not write/update the lock.
- `--no-verify` : skip pre-install verification (faster, less safe).

## Configuration Cheatsheet
Shorthand line form (known tools only):
```toml
terraform = "1.8.5"
helm = "^3.15.0"
```

Full table form (custom or to override):
```toml
[[tools]]
name = "mytool"
version = "2.0.1"
source = "https://example.com/mytool-{version}-{os}-{arch}.tar.gz"
kind = "archive"   # or "direct" for a single binary download
binary = "bin/mytool"  # optional path inside archive
```

Platform placeholders:
- `{os}` -> `linux`, `darwin`, `windows`
- `{arch}` -> `amd64`, `arm64`
- `{version}` always replaced (ranges are normalized for the URL)

Range specs: You can use caret/tilde/wildcards (e.g. `^3.15.0`); they are resolved to a concrete version and recorded in the lock (original request kept for verification).

## Supported Shorthand Tools
`terraform`, `kubectl`, `helm`, `gh`, `buf`, `node`, `pnpm`, `yarn`, `just`, `jq`, `cosign`, `age`.

## Shell Hook
Add dynamically (bash / zsh):
```bash
eval "$(tlk hook)"
```
Fish:
```fish
tlk hook --shell fish | source
```
PowerShell:
```powershell
tlk hook --shell powershell | Invoke-Expression
```
The hook prepends the nearest ancestor `.tlk/bin` whenever you cd into a directory containing `tlk.toml`, and removes it when you leave.

## CI Example
```bash
tlk install --locked    # ensure tlk.lock is honored
terraform version
kubectl version --client
```

## Troubleshooting
| Symptom | Hint |
|---------|------|
| 404 download | Check `source` URL & that `{os}` / `{arch}` values match upstream naming. Run `tlk plan` to see rendered URLs. |
| Binary missing after archive install | Add/adjust `binary` (e.g. `binary = "linux-amd64/helm"` not usually needed; tlk auto-detects common nested layouts). |
| Wrong version installed | Remove stale binary in `.tlk/bin` then re-run `tlk install`; ensure range actually matches the intended version. |
| PATH not updated | Ensure hook evaluated; run `which terraform` to confirm it points into `.tlk/bin`. |

## Build & Contribute
Minimal local build:
```bash
git clone <repo>
cd tool_locker
cargo build
./target/debug/tlk --help
```
Run tests / lint (if added later):
```bash
cargo test
```
Contribution guidelines (short form):
1. Open an issue describing the change (feature or fix).
2. Keep PRs small & focused; include a brief rationale in the description.
3. Update this README if user-facing behavior changes.
4. Prefer adding or updating tests when touching install / lock logic.

License: MIT OR Apache-2.0

---
Happy locking! 🚀
