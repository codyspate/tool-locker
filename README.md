# Tool Locker (tlk)

A lightweight CLI to manage non-language specific tool binaries across a code workspace using a single `tlk.toml`.

## Goals

- Keep *infrastructure / ancillary* tool versions (terraform, kubectl, helm, gh, buf, protoc, etc.) pinned alongside code.
- Provide reproducible, idempotent installs into a local `.tlk/bin` directory (ignored by VCS typically).
- Simple, explicit config file.

## Non-Goals

- Replacing language-native package managers like cargo, npm, pip.
- Complex resolution / dependency graphs. This is a flat list of necessary external tools.

## Quick Start

1. Create `tlk.toml` at the repo root using shorthand or explicit entries:

```toml
# Shorthand for common tools (terraform, kubectl, helm, gh, buf)
terraform = "1.8.5"
kubectl = "1.30.2"
helm = "3.15.2"

# Explicit form (table style) for unknown tool with overrides
[tools.terraform]
version = "1.8.5"
source = "https://releases.hashicorp.com/terraform/{version}/terraform_{version}_{os}_{arch}.zip" # {os},{arch} placeholders
kind = "archive"
```

2. Build and run:

```bash
cargo run -- plan
cargo run -- install
cargo run -- list
```

The binaries end up in `.tlk/bin`. Add that to your PATH in your shell profile:

```bash
export PATH="$(pwd)/.tlk/bin:$PATH"
```

## Lock File

Running `tlk install` writes `tlk.lock` capturing the exact resolved versions & fully-expanded source URLs (placeholders resolved for your platform).

Subsequent plain `tlk install` will verify:

- Every configured tool exists in the lock
- Versions match
- Expanded source URLs match
- Checksums match (when specified)
- Installed binary digest (sha256 of the installed file) matches what was recorded when the lock was generated (if present)

If any mismatch occurs the install aborts. Update the config or re-run `tlk install` to refresh the lock (unless you used `--no-lock`).

Use `tlk install --no-verify` to bypass verification temporarily (not recommended for CI). Run `tlk verify` to perform verification without installing.

## CLI Commands

`tlk plan`
	Print each configured tool with its version and resolved (template) source URL. No network or filesystem changes.

`tlk install [--no-verify] [--no-lock] [--locked]`
	Install all tools defined in `tlk.toml` into `.tlk/bin` (or custom `install_dir`).
	Behavior details:
	- Writes / updates `tlk.lock` by default.
	- Use `--no-lock` to suppress writing the lock file.
	- Use `--locked` to install only the versions already recorded in `tlk.lock` (no network version lookup, no lock modification). Fails if the lock file is missing.
	- Performs lock verification first unless `--no-verify`.
	- Skips downloading a tool if an installed binary reports the same version via `--version` output.

`tlk install <spec> [<spec> ...] [--latest] [--exact] [--no-lock]`
	Install one or more known tools directly (lock updated by default unless `--no-lock`). Each `spec` is `name` or `name@version`.
	Examples:
		`tlk install terraform`
		`tlk install terraform@1.7.5`
		`tlk install terraform helm --latest`
		`tlk install terraform@1.7.5 --exact`
	Flags:
	- `--latest` forces resolving the newest available version for every provided spec (ignores any @version segment).
	- `--exact` marks intention to keep the specified version exact (currently informational).
	- Lock file merges/updates by default; use `--no-lock` to avoid touching `tlk.lock`.

`tlk list`
	Show desired vs currently installed versions for each declared tool. Does not contact the network.

`tlk verify`
	Perform lock + integrity verification (same checks as a default `install` run) without performing downloads or writing files.

Note: Single or multi known-tool installs using specs do not update `tlk.toml` unless a lock write occurs (default). For reproducible setups commit both `tlk.toml` and `tlk.lock`.

Exit codes:
	- 0 on success for all commands.
	- Non-zero if any verification or installation step fails.

Idempotency:
	Running `tlk install` repeatedly with no config changes and an up-to-date lock is a no-op (apart from version checks and digest verification).

## Config Reference

Field | Type | Required | Description
----- | ---- | -------- | -----------
`tools` | array | yes | List of tool entries.
`name` | string | yes | Tool name; binary will be installed under this name.
`version` | string | yes | Desired version string; currently treated literally; future: semver range.
`kind` | enum | no | `archive` (default) or `direct`.
`source` | string | yes (implicit for shorthand) | Download URL; placeholders: `{version}`, `{os}`, `{arch}`.
`sha256` | string | no | If provided, checksum must match.
`binary` | string | no | Relative path inside archive to actual binary (defaults to `name`).
`install_dir` | string | no | Custom install directory (default `.tlk/bin`).

## Shorthand Supported Tools

Current built-in shorthand keys: `terraform`, `kubectl`, `helm`, `gh`, `buf`, `node`, `pnpm`, `yarn`, `just`, `jq`, `cosign`, `age`.

Platform mapping: `{os}` -> `linux`, `darwin`, `windows`; `{arch}` -> `amd64`, `arm64` (common remaps from Rust triples).

### Known vs Unknown Tools

There are two categories:

1. Known tools: built-in shorthand keys (`terraform = "1.2.3"`). tlk knows their URL patterns.
2. Unknown tools: any others you declare with a full `[[tools]]` table providing `name`, `version`, and at least one source template (`source`, optionally overrides).

For unknown tools you can use the same multi-platform override system plus an extra `{ext}` placeholder if you want to abstract archive extension differences (e.g. `.zip` on Windows vs `.tar.gz` elsewhere). tlk currently does not auto-determine `{ext}`, but you can model it via per_os/per_os_arch entries that hardcode the right extension.

### Multi-Platform Source Overrides

You can flexibly describe platform-specific download URLs using layered overrides in `tlk.toml`.

Precedence (highest wins): `per_os_arch` > `per_os` > `source`.

Supported placeholder variables inside templates:
- `{version}` always replaced
- `{os}` replaced with `linux|darwin|windows` (only meaningful in `source` or `per_os.*` entries)
- `{arch}` replaced with normalized `amd64|arm64`
- `{ext}` (user-defined in your patterns; not auto-resolved globally—use per_os/per_os_arch to swap extensions)

Arch synonyms accepted in config: `x86_64` == `amd64`, `aarch64` == `arm64`.

Basic single template example:
```toml
[tools.foo]
version = "1.2.3"
source = "https://example.com/foo/{version}/{os}/{arch}/foo.tar.gz"
```

Add per-OS overrides (still may contain `{arch}`):
```toml
[tools.bar]
version = "2.0.0"
source = "https://cdn.example.com/bar/{version}/neutral/bar.tgz" # fallback

[tools.bar.per_os]
linux = "https://cdn.example.com/bar/{version}/linux/{arch}/bar.tgz"
mac = "https://cdn.example.com/bar/{version}/macos/universal/bar.zip" # arch ignored
windows = "https://cdn.example.com/bar/{version}/windows/{arch}/bar.zip"
```

Fine-grained per OS+Arch overrides (only `{version}` required; you can hardcode filenames):
```toml
[tools.baz]
version = "3.1.0"
source = "https://fallback.example.com/baz-{version}.tgz"

[tools.baz.per_os_arch.linux]
amd64 = "https://downloads.example.com/baz/v{version}/baz-linux-amd64.tar.gz"
arm64 = "https://downloads.example.com/baz/v{version}/baz-linux-arm64.tar.gz"

[tools.baz.per_os_arch.mac]
arm64 = "https://downloads.example.com/baz/v{version}/baz-macos-arm64.zip"
x86_64 = "https://downloads.example.com/baz/v{version}/baz-macos-x86_64.zip"

[tools.baz.per_os_arch.windows]
amd64 = "https://downloads.example.com/baz/v{version}/baz-windows-amd64.zip"
```

For each tool entry, only include the override tables relevant to that tool. The resolver picks the most specific match for the running platform.

Tip: Keep `source` pointing to a portable default (or even a 404) for clarity; real platform-specific URLs can live in the override tables.

## Future Ideas

- Auto OS / ARCH template variables (e.g., `{os}`, `{arch}`).
- Support GitHub release pattern shorthand (e.g., `github:owner/repo@v{version}#asset`)
- Caching / skipping existing up-to-date installs.
- Lock file generation (`tlk.lock`).
- Support other artifact types (.tar.xz, checksums file auto-verify).
- Concurrent downloads.
- Windows support nuances.
- `self-update` command.
- `doctor` command to validate PATH setup.

## License

Dual-licensed under MIT or Apache-2.0.
