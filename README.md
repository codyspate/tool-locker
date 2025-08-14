# tlk – Tool Locker

> Reproducible, per‑repo installs of infra / DevOps CLI tools (terraform, kubectl, helm, gh, buf, jq, node, pnpm, etc.) without polluting your global PATH.

`tlk` lets a repository declare the *exact* external command‑line tools it depends on in a simple `tlk.toml`. A lock file (`tlk.lock`) captures resolved versions + source URLs. Teammates (and CI) run `tlk install` to materialize a private `.tlk/bin` folder that is auto‑activated by a lightweight shell hook. No more “which version of terraform do I need?” messages.

It’s like a `package.json` + lock for the miscellaneous non-language CLIs you need to build/deploy/test… and nothing else. Bring your own language toolchain manager; `tlk` focuses on the rest.

---

## Quick Start

### 1. Build or obtain `tlk`

Install latest release: https://github.com/codyspate/tool-locker/releases

**Or Build:**

```bash
git clone https://github.com/codyspate/tool-locker.git
cd tool_locker
cargo build --release
./target/release/tlk --help
```

Place the binary somewhere on your PATH or just run it from the repo.

### 2. Add a `tlk.toml` to your project

```toml
terraform = "^1.8.5"
kubectl = "1.30.2"         # exact version
helm = "^3.15.2"           # range (caret)
just = "latest"            # will resolve to current latest at install time

[tools.protoc]              # custom tool form
version = "24.4"
source = "https://github.com/protocolbuffers/protobuf/releases/download/v{version}/protoc-{version}-linux-x86_64.zip"
kind = "archive"
binary = "bin/protoc"
```

### 3. Install

```bash
tlk install
```

Creates `.tlk/bin` and downloads each tool (parallelized when >1). Writes / updates `tlk.lock` with exact versions and fully rendered URLs.

### 4. Auto‑activate PATH (optional but nice)

```bash
eval "$(tlk hook)"   # bash / zsh
```

Now when you `cd` into a directory containing `tlk.toml`, that repo’s `.tlk/bin` is transparently prepended to PATH for the shell session.

Commit both `tlk.toml` and `tlk.lock`.

---


## Why (Problems This Solves)

| Pain | tlk Answer |
|------|------------|
| Onboarding friction (“install these 7 tools, specific versions”) | One command after clone: `tlk install` |
| Global version drift (Homebrew / chocolatey upgrades behind your back) | Explicit versions/ranges, concretized & locked |
| Cross‑platform URL differences | Normalized `{os}` / `{arch}` placeholders; multi‑platform sources in lock |
| “Works on my machine” infra tooling | Per‑repo sandbox `.tlk/bin`, not global PATH |
| CI reproducibility | `tlk install --locked` guarantees lock fidelity |
| Ad‑hoc curl | Manual URLs become declarative entries with optional checksums |

Core ideas:
1. Declarative desired tool specs (`tlk.toml`).
2. Deterministic resolution -> concrete download URL(s).
3. Idempotent installer (skips unchanged versions).
4. Lock file capturing *exact* resolved version + rendered URL and platform matrix.
5. Zero global side effects: everything lives under project root (unless you purposely use `setup`).

---


## Configuration Reference (`tlk.toml`)

Two syntaxes coexist:

1. **Shorthand (for known tools)** – single line: `terraform = "1.8.5"` or ranges like `^1.8.0` or `latest`.
2. **Full table (for custom / advanced)** under `[tools.<name>]` with fields:
	- `version` (string; can be range for known tools, but custom entries should be concrete)
	- `source` (URL template; supports `{version}`, `{os}`, `{arch}`)
	- `kind` = `archive` | `direct` (defaults to archive)
	- `binary` (path inside archive; omitted for direct downloads or auto‑detected for some known tools)
	- `sha256` (optional explicit checksum of the archive / binary)
	- `per_os` and `per_os_arch` override maps for differing naming conventions (see code for full shape)

Placeholders:
| Token | Values |
|-------|--------|
| `{os}` | linux, darwin, windows |
| `{arch}` | amd64, arm64 |
| `{version}` | The resolved *exact* version |

If you provide both generic `source` and more specific `per_os` / `per_os_arch`, specificity wins (per‑OS+arch > per‑OS > generic).

Legacy `[[tools]]` array form is still accepted; run `tlk migrate-config` to upgrade to the `[tools.<name>]` style.

---

## Supported Shorthand Tools (built‑in recipes)

`terraform`, `kubectl`, `helm`, `gh`, `buf`, `node`, `pnpm`, `yarn`, `just`, `jq`, `cosign`, `age`.

Each has logic for platform naming quirks (e.g. node’s x64 vs amd64) and implicit `binary` paths when they aren’t at archive root.

---

## Commands Cheat Sheet

| Command | What it does |
|---------|--------------|
| `tlk install` | Install or update all declared tools (writes/updates lock unless `--no-lock`) |
| `tlk install terraform@1.7.5 helm@latest` | Ad‑hoc install of specific known tool specs (bypasses `tlk.toml` entries for those) |
| `tlk install --locked` | Reinstall exactly what’s in `tlk.lock` (no writes) |
| `tlk plan` | Dry run: show planned names, versions, base URLs/templates |
| `tlk list` | Show desired vs installed versions (parse `--version` output) |
| `tlk verify` | Validate `tlk.lock` vs config + binaries (digest / checksum) |
| `tlk uninstall <name>` | Remove tool + config + lock entry |
| `tlk hook` | Emit shell hook (eval it) |
| `tlk setup` | One‑time create a global `~/.tlk/bin` (future use) |
| `tlk migrate-lock` | Regenerate lock at latest schema & platform matrix |
| `tlk migrate-config` | Rewrite legacy `[[tools]]` syntax to new table style |
| `tlk diagnose --kind missing-platforms` | Spot tools lacking multi‑platform entries in lock |

Useful flags:
| Flag | Meaning |
|------|---------|
| `--no-lock` | Skip creating/updating `tlk.lock` on install |
| `--locked` | Disallow resolution; only use already locked entries |
| `--no-verify` | Skip pre‑install verification (speed vs safety) |
| `--exact` | When installing specs, store exact instead of caret range |

---

## Version Specs & Resolution

For shorthand known tools you may supply:
* Exact semver: `1.2.3`
* Caret / tilde: `^1.2.3`, `~1.2.0`
* Partial / wildcard: `1.2.x`, `1.x`, `^1`
* `latest`
* Complex OR / hyphen ranges (limited support): `1.2.x || 1.3.x`, `1.2.0 - 1.4.5`

Canonicalization logic rewrites what’s stored back into config (when adding via specs) so teammates see the intended constraint (e.g. `1.2.3` becomes `^1.2.3` unless `--exact` used). The lock file always records the concrete chosen version.

Custom `[tools.<name>]` entries should generally provide exact versions (range satisfaction for arbitrary URLs is not yet implemented).

---

## The Lock File (`tlk.lock`)

Schema (v3) stores for each tool:
* `version` – resolved exact version
* `requested_version` – original range / spec (if different)
* `source` – concrete URL used for the current platform
* `source_template` – the template (with placeholders)
* `sources` – matrix of rendered platform URLs when placeholders are present (linux/darwin/windows × amd64/arm64)
* `sha256` – optional checksum copied from config
* `digest` – SHA256 of the installed binary (post‑extraction)

`tlk verify` re-renders expected URLs and compares digests & checksums so CI can catch drift or tampering. Use `tlk install --locked` to fail fast if config references versions not present in the lock.

---

## Shell Integration

Two patterns:
1. Ephemeral PATH adjustment after install (`tlk` attempts to prepend `.tlk/bin` to its own process PATH for immediate use).
2. Persistent dynamic hook (`eval "$(tlk hook)"`) that tracks `cd` events and toggles PATH accordingly. Remove it => no global pollution.

Fish / PowerShell variants available via `--shell`.

---

## Typical Workflows

Add a new known tool at latest:
```bash
tlk install just@latest --no-lock   # fetch latest
tlk install --no-verify             # install others if needed
tlk install --locked || tlk install # then lock (or simply rerun without --no-lock)
```

Update a range (`^1.8.5`) to absorb new patch/minor:
```bash
tlk install                         # will fetch newer if within range
```

Pin an exact version for reproducibility audit:
```bash
tlk install terraform@1.8.7 --exact
```

Uninstall a tool:
```bash
tlk uninstall jq
```

Verify before commit / in CI:
```bash
tlk verify && tlk install --locked
```

---

## Comparison & Tradeoffs

| Tool / Approach | Where tlk fits |
|-----------------|----------------|
| asdf / mise | Those manage language runtimes (and via plugins, other tools). `tlk` is zero‑plugin, fast, file‑driven; simpler surface, fewer moving parts. |
| Homebrew / apt / choco | System package managers; mutate global state; slower upgrades; no per‑repo isolation. Pair nicely: use them only for `tlk` binary itself. |
| Nix / Devbox | Extremely reproducible; steeper learning curve and larger conceptual surface. `tlk` is intentionally narrow / lightweight. |
| Docker images | Great for hermetic builds but slower for local iterative CLIs; `tlk` keeps native performance. |

Tradeoffs / current limitations:
* Limited “latest version listing” support (only for subset of known tools: terraform, helm, gh, buf, kubectl).
* No automatic checksum lookups (you must provide `sha256` manually if you want strict verification beyond digest of downloaded artifact).
* Range semantics best‑effort; extremely complex compound ranges may resolve unexpectedly.
* Not a general artifact cache / mirror (no offline mode yet).
* Windows support is present but less battle‑tested than Linux/macOS.

---

## Security & Integrity

`sha256` (config) vs `digest` (lock) – The former is a *known good* provided by you (or upstream release notes). The latter is the hash of what was actually installed. Add `sha256` for critical tools to catch supply chain tampering at download time; `digest` then confirms the stored binary hasn’t changed since locking.

Recommendations:
1. For security‑sensitive binaries (e.g. `cosign`), copy upstream published SHA256 and add to your entry.
2. Run `tlk verify` in CI.
3. Consider commit signing of lock file changes in high‑assurance environments.

Future ideas: automated checksum retrieval, optional signature verification (e.g., cosign attestations), offline cache.

---

## Architecture (High Level)

Rust workspace with a single `cli` crate. Core modules:
* `config.rs` – Parse `tlk.toml`, merging shorthand and custom entries; supports legacy repair.
* `known_tools.rs` – Catalog of built‑in tool recipes (templated or custom URL generators) + platform detection.
* `installer.rs` – Parallel download & extraction, verification, digesting, path refresh.
* `lock.rs` – v3 lock file schema + legacy upgrade.
* `versioning.rs` – Fetch & cache version lists (GitHub / HashiCorp scraping) for “latest” & range resolution.
* `command_handlers/*` – Thin orchestration for each subcommand (install, migrate, diagnose, etc.).
* `platform/*` – OS abstractions (permissions, naming, windows vs unix differences).

Install flow summary:
```
tlk.toml -> parse -> Tool structs -> (verify lock) -> parallel fetch -> extract -> compute digest -> write/update lock
```

---

## Roadmap / Ideas

* Offline / local cache (avoid re-downloading unchanged archives across repos).
* Checksum auto‑discovery & signature verification.
* Optional global registry of “recipes” discoverable from config.
* Richer `plan` diff (what’s changing & why).
* JSON output for machine integration (`--format json`).
* Built‑in update helper (bump locked versions satisfying ranges).
* More known tools (PRs welcome – keep curated, low maintenance).

---

## Contributing

1. Open an issue describing the change / addition.
2. Keep PRs focused; include rationale in the description.
3. Update this README for user‑visible changes.
4. Add or adjust tests (when they’re introduced) for lock / install logic.

Feeling adventurous? Prototype a feature behind a hidden flag and open a discussion.

---

## License

Dual licensed under either of

* MIT license
* Apache License, Version 2.0

at your option.

---

Happy locking! 🔐

