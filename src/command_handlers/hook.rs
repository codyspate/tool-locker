use anyhow::Result;

// Public entry: print the appropriate hook script. For now we just ignore shell arg difference
// and output a POSIX-friendly function that should work in bash and zsh.
pub fn print_hook(shell: Option<&str>) -> Result<()> {
    match shell.map(|s| s.to_lowercase()) {
        Some(ref s) if s == "fish" => println!("{}", FISH_HOOK),
        Some(ref s) if s == "powershell" || s == "pwsh" => println!("{}", POWERSHELL_HOOK),
        _ => println!("{}", POSIX_HOOK),
    }
    Ok(())
}

// The hook strategy:
// - Define a function _tlk_sync_path invoked on every prompt (PROMPT_COMMAND / precmd)
// - Walk up from $PWD to filesystem root looking for tlk.toml.
// - If found, ensure $PROJECT/.tlk/bin exists; prepend (once) via an exported PATH containing TLK_ACTIVE_BIN.
// - If not found and TLK_ACTIVE_BIN was previously set, remove it from PATH.
// - Use an exported TLK_ACTIVE_BIN var to track currently active bin path.
// - Avoid repeated expensive scans by caching last $PWD in TLK_LAST_PWD.
// - Keep modifications idempotent and reversible.
// User usage: eval "$(tlk hook)"  OR tlk hook --shell bash | source /dev/stdin
const POSIX_HOOK: &str = r##"
# tlk dynamic PATH activation hook
# Add by running: eval "$(tlk hook)"
# Supports bash (PROMPT_COMMAND) and zsh (precmd). Safe to re-eval.

_tlk_find_project_root() {
  local dir="$PWD"
  while [ "$dir" != "/" ]; do
    if [ -f "$dir/tlk.toml" ]; then
      printf '%s' "$dir"
      return 0
    fi
    dir="${dir%/*}"
    [ -z "$dir" ] && break
  done
  return 1
}

_tlk_path_remove() {
  # remove an entry ($1) from PATH (exact match on path element)
  local target="$1" newpath="" IFS=':' part first=1
  for part in $PATH; do
    if [ "$part" = "$target" ]; then
      continue
    fi
    if [ $first -eq 1 ]; then
      newpath="$part"; first=0
    else
      newpath="$newpath:$part"
    fi
  done
  PATH="$newpath"
  export PATH
}

_tlk_sync_path() {
  # Fast path: if directory unchanged, exit
  if [ "$PWD" = "$TLK_LAST_PWD" ]; then
    return 0
  fi
  TLK_LAST_PWD="$PWD"
  export TLK_LAST_PWD

  local root
  if root=$(_tlk_find_project_root); then
    local bindir="$root/.tlk/bin"
    if [ -d "$bindir" ]; then
      if [ "$TLK_ACTIVE_BIN" != "$bindir" ]; then
        # switching context
        if [ -n "$TLK_ACTIVE_BIN" ] && [ -n "$TLK_ACTIVE_BIN" ] && [ -d "$TLK_ACTIVE_BIN" ]; then
          _tlk_path_remove "$TLK_ACTIVE_BIN"
        fi
  case ":$PATH:" in
          *":$bindir:"*) ;; # already there
          *) PATH="$bindir:$PATH"; export PATH;;
        esac
        TLK_ACTIVE_BIN="$bindir"; export TLK_ACTIVE_BIN
  echo "[tlk] activated $bindir"
      fi
    fi
  else
    # leaving a project
    if [ -n "$TLK_ACTIVE_BIN" ]; then
      _tlk_path_remove "$TLK_ACTIVE_BIN"
      unset TLK_ACTIVE_BIN
    fi
  fi
}

# Install prompt hooks (bash)
if [ -n "${BASH_VERSION:-}" ]; then
  case "$PROMPT_COMMAND" in
    *"_tlk_sync_path"*) ;;
    "") PROMPT_COMMAND="_tlk_sync_path" ;;
    *) PROMPT_COMMAND="_tlk_sync_path;${PROMPT_COMMAND}" ;;
  esac
  export PROMPT_COMMAND
fi

# Install prompt hooks (zsh)
if [ -n "${ZSH_VERSION:-}" ]; then
  if ! typeset -f precmd >/dev/null 2>&1; then
    precmd() { :; }
  fi
  if ! typeset -f _tlk_prepend_precmd >/dev/null 2>&1; then
    _tlk_prepend_precmd() {
      _tlk_sync_path
      # Call original precmd if saved
      if [ -n "$__TLK_ORIG_PRECMD" ]; then
        eval "$__TLK_ORIG_PRECMD"
      fi
    }
    if typeset -f precmd >/dev/null 2>&1; then
      __TLK_ORIG_PRECMD="$(typeset -f precmd | tail -n +2)"
    fi
    precmd() { _tlk_prepend_precmd; }
  fi
fi

# Initial invocation for current directory
_tlk_sync_path
"##;

// fish shell variant using fish_prompt event
const FISH_HOOK: &str = r#"# tlk dynamic PATH activation (fish)
function __tlk_find_root
    set -l dir $PWD
    while test "$dir" != /
        if test -f "$dir/tlk.toml"
            echo $dir
            return 0
        end
        set dir (dirname $dir)
    end
    return 1
end

function __tlk_path_remove
    set -l target $argv[1]
    set -l new ''
    for p in $PATH
        if test $p != $target
            if test -z "$new"
                set new $p
            else
                set new $new $p
            end
        end
    end
    set -gx PATH $new
end

function __tlk_sync_path --on-event fish_prompt
    if test "$PWD" = "$TLK_LAST_PWD"
        return
    end
    set -gx TLK_LAST_PWD $PWD
    set -l root (__tlk_find_root)
    if test -n "$root"
        set -l bindir "$root/.tlk/bin"
        if test -d $bindir
            if test "$TLK_ACTIVE_BIN" != $bindir
                if test -n "$TLK_ACTIVE_BIN"
                    __tlk_path_remove $TLK_ACTIVE_BIN
                end
                if not contains $bindir $PATH
                    set -gx PATH $bindir $PATH
                end
                set -gx TLK_ACTIVE_BIN $bindir
                echo "[tlk] activated $bindir"
            end
        end
    else
        if test -n "$TLK_ACTIVE_BIN"
            __tlk_path_remove $TLK_ACTIVE_BIN
            set -e TLK_ACTIVE_BIN
        end
    end
end

# initial run
__tlk_sync_path
"#;

// PowerShell hook leveraging prompt function override
const POWERSHELL_HOOK: &str = r#"# tlk dynamic PATH activation (PowerShell)
function Get-TlkProjectRoot {
  $d = Get-Location
  while ($d -and $d -ne [IO.Path]::GetPathRoot($d)) {
    if (Test-Path (Join-Path $d 'tlk.toml')) { return $d }
    $parent = Split-Path $d -Parent
    if (-not $parent -or $parent -eq $d) { break }
    $d = $parent
  }
  return $null
}

function Remove-TlkPath([string]$target) {
  if (-not $target) { return }
  $parts = $Env:PATH -split ';' | Where-Object { $_ -and ($_ -ne $target) }
  $Env:PATH = ($parts -join ';')
}

function global:prompt {
  if ($PWD.Path -ne $Env:TLK_LAST_PWD) {
    $Env:TLK_LAST_PWD = $PWD.Path
    $root = Get-TlkProjectRoot
    if ($root) {
      $bindir = Join-Path $root '.tlk/bin'
      if (Test-Path $bindir) {
        if ($Env:TLK_ACTIVE_BIN -ne $bindir) {
          if ($Env:TLK_ACTIVE_BIN) { Remove-TlkPath $Env:TLK_ACTIVE_BIN }
          if (-not ($Env:PATH -split ';' | Where-Object { $_ -eq $bindir })) {
            $Env:PATH = "$bindir;" + $Env:PATH
          }
          $Env:TLK_ACTIVE_BIN = $bindir
          Write-Host "[tlk] activated $bindir" -ForegroundColor Cyan
        }
      }
    } else {
      if ($Env:TLK_ACTIVE_BIN) { Remove-TlkPath $Env:TLK_ACTIVE_BIN; $Env:TLK_ACTIVE_BIN = $null }
    }
  }
  if (Get-Command Write-Host -ErrorAction SilentlyContinue) { "PS " + $(Get-Location) + "> " } else { "PS> " }
}

# Initial sync
& global:prompt > $null
"#;
