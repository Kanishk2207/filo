#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

# Comprehensive end-to-end verification for filo.
#
# What this script does:
#  1) Uninstalls existing filo binary
#  2) Installs latest local repo changes
#  3) Creates isolated test folders and seeds 200-300 files per watched folder
#  4) Replaces config.toml with a comprehensive test config
#  5) Exercises manual commands
#  6) Exercises watcher daemon mode
#  7) Adds many files while daemon is running
#  8) Stops daemon
#  9) Verifies deterministic/predictable outcomes
#
# Environment overrides:
#   BULK_COUNT=300   # files per watched folder before scan
#   LIVE_COUNT=300   # files per watched folder while daemon is running
#   KEEP_ARTIFACTS=1 # keep temp test folder on failure for inspection

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

BULK_COUNT="${BULK_COUNT:-300}"
LIVE_COUNT="${LIVE_COUNT:-300}"
KEEP_ARTIFACTS="${KEEP_ARTIFACTS:-0}"

TEST_ROOT=""
BACKUP_CONFIG=""
CONFIG_DIR=""
DATA_DIR=""
CONFIG_PATH=""
LOG_PATH=""

WATCH_A=""
WATCH_B=""
WATCH_C=""
ARRANGE_SRC=""
ARRANGE_DEST=""

log() {
  printf '\n[%s] %s\n' "$(date '+%H:%M:%S')" "$*"
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"
}

run_and_capture() {
  local __var="$1"
  shift
  local tmp_base tmp_file status output

  tmp_base="${TMPDIR:-/tmp}"
  tmp_base="${tmp_base%/}"
  tmp_file="$(mktemp "${tmp_base}/filo-e2e-cmd.XXXXXX")"

  set +e
  "$@" 2>&1 | tee "${tmp_file}"
  status=${PIPESTATUS[0]}
  set -e

  output="$(cat "${tmp_file}")"
  rm -f "${tmp_file}"
  printf -v "${__var}" '%s' "${output}"
  return "${status}"
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  printf '%s' "$haystack" | grep -Fq "$needle" || {
    printf 'Assertion failed: expected output to contain: %s\n' "$needle" >&2
    printf -- '--- Output Start ---\n%s\n--- Output End ---\n' "$haystack" >&2
    exit 1
  }
}

assert_file() {
  local path="$1"
  [[ -f "$path" ]] || die "Expected file not found: $path"
}

assert_dir() {
  local path="$1"
  [[ -d "$path" ]] || die "Expected directory not found: $path"
}

toml_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

detect_platform_paths() {
  case "$(uname -s)" in
    Darwin)
      CONFIG_DIR="${HOME}/Library/Application Support/filo"
      DATA_DIR="${HOME}/Library/Application Support/filo"
      ;;
    Linux)
      CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/filo"
      DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/filo"
      ;;
    *)
      die "Unsupported OS for this script: $(uname -s)"
      ;;
  esac

  CONFIG_PATH="${CONFIG_DIR}/config.toml"
  LOG_PATH="${DATA_DIR}/filo.log"
}

cleanup() {
  local status=$?
  set +e
  if command -v filo >/dev/null 2>&1; then
    filo stop >/dev/null 2>&1 || true
  fi

  if [[ -n "${BACKUP_CONFIG}" && -f "${BACKUP_CONFIG}" ]]; then
    mkdir -p "${CONFIG_DIR}"
    cp "${BACKUP_CONFIG}" "${CONFIG_PATH}"
    rm -f "${BACKUP_CONFIG}"
  else
    rm -f "${CONFIG_PATH}"
  fi

  if [[ -n "${TEST_ROOT}" && -d "${TEST_ROOT}" ]]; then
    if [[ "${status}" -ne 0 && "${KEEP_ARTIFACTS}" == "1" ]]; then
      log "KEEP_ARTIFACTS=1 set; preserving test root: ${TEST_ROOT}"
    else
      rm -rf "${TEST_ROOT}"
    fi
  fi

  return "${status}"
}
trap cleanup EXIT

backup_existing_config() {
  local tmp_base
  mkdir -p "${CONFIG_DIR}"
  if [[ -f "${CONFIG_PATH}" ]]; then
    tmp_base="${TMPDIR:-/tmp}"
    tmp_base="${tmp_base%/}"
    BACKUP_CONFIG="$(mktemp "${tmp_base}/filo-config-backup.XXXXXX")"
    cp "${CONFIG_PATH}" "${BACKUP_CONFIG}"
    log "Backed up existing config to ${BACKUP_CONFIG}"
  else
    log "No existing config found at ${CONFIG_PATH}; a temporary test config will be created."
  fi
}

write_comprehensive_config() {
  local watch_a_esc
  local watch_b_esc
  watch_a_esc="$(toml_escape "${WATCH_A}")"
  watch_b_esc="$(toml_escape "${WATCH_B}")"

  # Keep this section as the single source of truth for evolving feature tests.
  cat > "${CONFIG_PATH}" <<EOF
other_category = "Other"

[watch]
folders = [
  "${watch_a_esc}",
  "${watch_b_esc}",
]
auto_start = false
debounce_ms = 700

[rename]
enabled = true
lowercase_with_hyphens = true
strip_redundant_words = true
prepend_date = false

[duplicates]
action = "move"
folder_name = "Duplicates"

[keyword_rules]
rules = [
  { to = "Amazon", keywords = ["amazon"] },
  { to = "Finance", keywords = ["invoice", "receipt", "statement"] },
  { to = "Travel", keywords = ["ticket", "boarding"] },
]

[rules]
Archives  = ["zip", "tar", "gz", "rar", "7z"]
Audio     = ["mp3", "wav", "m4a"]
Code      = ["rs", "py", "js", "ts", "json", "toml", "yaml", "yml"]
Documents = ["pdf", "doc", "docx", "txt", "md", "csv"]
Images    = ["jpg", "jpeg", "png", "gif", "webp"]
Videos    = ["mp4", "mkv", "mov"]
EOF

  log "Wrote test config to ${CONFIG_PATH}"
}

wait_for_file() {
  local path="$1"
  local timeout_s="$2"
  local ticks=$((timeout_s * 10))
  local i
  for ((i = 0; i < ticks; i++)); do
    if [[ -f "${path}" ]]; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

count_actionable_top_files() {
  local dir="$1"
  find "${dir}" -maxdepth 1 -type f \
    ! -name '.*' \
    ! -iname '*.part' \
    ! -iname '*.crdownload' \
    ! -iname '*.download' \
    ! -iname '*.tmp' \
    ! -iname '*.partial' \
    | wc -l | tr -d ' '
}

dump_top_level_state() {
  local dir="$1"
  local label="$2"
  log "Diagnostics for ${label} (${dir})"
  printf '  total top-level files: %s\n' "$(find "${dir}" -maxdepth 1 -type f | wc -l | tr -d ' ')"
  printf '  actionable top-level files: %s\n' "$(count_actionable_top_files "${dir}")"
  printf '  sample top-level entries (up to 40):\n'
  find "${dir}" -maxdepth 1 -type f -print | head -n 40 | sed 's/^/    /'
}

wait_for_actionable_drain() {
  local dir="$1"
  local max_remaining="$2"
  local timeout_s="$3"
  local ticks=$((timeout_s * 2))
  local i count
  for ((i = 0; i < ticks; i++)); do
    count="$(count_actionable_top_files "${dir}")"
    if [[ "${count}" -le "${max_remaining}" ]]; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}

wait_for_match_count() {
  local dir="$1"
  local name_glob="$2"
  local min_count="$3"
  local timeout_s="$4"
  local ticks=$((timeout_s * 10))
  local i count

  for ((i = 0; i < ticks; i++)); do
    if [[ -d "${dir}" ]]; then
      count="$(find "${dir}" -maxdepth 1 -type f -name "${name_glob}" | wc -l | tr -d ' ')"
    else
      count="0"
    fi
    if [[ "${count}" -ge "${min_count}" ]]; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

wait_for_daemon() {
  local i
  for i in $(seq 1 40); do
    local out
    out="$(filo refresh 2>&1 || true)"
    if printf '%s' "${out}" | grep -Fq "Reload signal sent."; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}

generate_bulk_files() {
  local dir="$1"
  local label="$2"
  local count="$3"
  local i idx mod stem ext file

  mkdir -p "${dir}"
  for i in $(seq 1 "${count}"); do
    printf -v idx "%03d" "${i}"
    mod=$((i % 12))
    case "${mod}" in
      0) stem="${label}_amazon_invoice_${idx}"; ext="pdf" ;;
      1) stem="${label}_receipt_${idx}"; ext="png" ;;
      2) stem="${label}_ticket_${idx}"; ext="pdf" ;;
      3) stem="${label}_photo_${idx}"; ext="jpg" ;;
      4) stem="${label}_archive_${idx}"; ext="zip" ;;
      5) stem="${label}_music_${idx}"; ext="mp3" ;;
      6) stem="${label}_script_${idx}"; ext="py" ;;
      7) stem="${label}_movie_${idx}"; ext="mp4" ;;
      8) stem="${label}_notes_${idx}"; ext="txt" ;;
      9) stem="${label}_data_${idx}"; ext="csv" ;;
      10) stem="${label}_unknown_${idx}"; ext="weird" ;;
      *) stem="${label} Report FINAL FINAL ${idx}"; ext="PDF" ;;
    esac
    file="${dir}/${stem}.${ext}"
    printf 'payload-%s-%s\n' "${label}" "${idx}" > "${file}"
  done

  # Files that should be ignored by organizer rules.
  printf 'hidden-metadata\n' > "${dir}/.DS_Store"
  printf 'still-downloading\n' > "${dir}/${label}_incomplete.crdownload"
}

seed_scan_fixtures() {
  # Deterministic keyword + extension assertions.
  printf 'fixture-a\n' > "${WATCH_A}/Amazon_Invoice_May.PDF"
  printf 'fixture-b\n' > "${WATCH_A}/Monthly_receipt.png"
  printf 'fixture-c\n' > "${WATCH_A}/plain_photo.JPG"

  # Duplicate handling assertions (action = move).
  printf 'fixture-dup\n' > "${WATCH_A}/dup_source_one.pdf"
  printf 'fixture-dup\n' > "${WATCH_A}/dup_source_two.pdf"

  # Collision safety assertion.
  mkdir -p "${WATCH_A}/Documents"
  printf 'old-collision\n' > "${WATCH_A}/Documents/collision.pdf"
  printf 'new-collision\n' > "${WATCH_A}/collision.pdf"

  # Should be skipped.
  printf 'stay-hidden\n' > "${WATCH_A}/.hidden_should_stay"
  printf 'still-downloading\n' > "${WATCH_A}/pending.download"
}

seed_arrange_fixtures() {
  printf 'arrange-a\n' > "${ARRANGE_SRC}/2026_invoice_alpha.pdf"
  printf 'arrange-b\n' > "${ARRANGE_SRC}/invoice_beta.txt"
  printf 'arrange-c\n' > "${ARRANGE_SRC}/receipt_gamma.pdf"
  printf 'arrange-d\n' > "${ARRANGE_SRC}/random_photo.jpg"
}

main() {
  require_cmd cargo
  require_cmd find
  require_cmd sed
  require_cmd grep

  detect_platform_paths
  backup_existing_config

  local tmp_base
  tmp_base="${TMPDIR:-/tmp}"
  tmp_base="${tmp_base%/}"
  TEST_ROOT="$(mktemp -d "${tmp_base}/filo-e2e.XXXXXX")"
  WATCH_A="${TEST_ROOT}/watch_a"
  WATCH_B="${TEST_ROOT}/watch_b"
  WATCH_C="${TEST_ROOT}/watch_c"
  ARRANGE_SRC="${TEST_ROOT}/arrange_src"
  ARRANGE_DEST="${TEST_ROOT}/arrange_dest"
  mkdir -p "${WATCH_A}" "${WATCH_B}" "${WATCH_C}" "${ARRANGE_SRC}" "${ARRANGE_DEST}"
  WATCH_A="$(cd "${WATCH_A}" && pwd -P)"
  WATCH_B="$(cd "${WATCH_B}" && pwd -P)"
  WATCH_C="$(cd "${WATCH_C}" && pwd -P)"
  ARRANGE_SRC="$(cd "${ARRANGE_SRC}" && pwd -P)"
  ARRANGE_DEST="$(cd "${ARRANGE_DEST}" && pwd -P)"

  log "1) Uninstall existing filo binary (if present)"
  cargo uninstall filo >/dev/null 2>&1 || true

  log "2) Install latest local changes"
  cargo install --path . --force
  require_cmd filo

  log "3) Create bulk fixture files (${BULK_COUNT} per watched folder)"
  generate_bulk_files "${WATCH_A}" "initial_a" "${BULK_COUNT}"
  generate_bulk_files "${WATCH_B}" "initial_b" "${BULK_COUNT}"
  seed_scan_fixtures
  seed_arrange_fixtures

  log "4) Replace config with comprehensive test config"
  write_comprehensive_config

  log "5) Manual command checks"
  local out

  run_and_capture out filo --version
  assert_contains "${out}" "filo"

  run_and_capture out filo preview
  assert_contains "${out}" "Preview summary"
  assert_contains "${out}" "would move"

  run_and_capture out filo scan
  assert_contains "${out}" "Scan complete:"

  # Assert deterministic scan outcomes.
  assert_file "${WATCH_A}/Amazon/amazon-invoice-may.pdf"
  assert_file "${WATCH_A}/Finance/monthly-receipt.png"
  assert_file "${WATCH_A}/Images/plain-photo.jpg"
  local dup_primary_count dup_duplicate_count
  dup_primary_count="$(find "${WATCH_A}/Documents" -maxdepth 1 -type f \( -name 'dup-source-one*.pdf' -o -name 'dup-source-two*.pdf' \) | wc -l | tr -d ' ')"
  dup_duplicate_count="$(find "${WATCH_A}/Documents/Duplicates" -maxdepth 1 -type f \( -name 'dup-source-one*.pdf' -o -name 'dup-source-two*.pdf' \) | wc -l | tr -d ' ')"
  [[ "${dup_primary_count}" -ge 1 ]] || {
    die "Expected at least one dup-source-{one,two}*.pdf in Documents"
  }
  [[ "${dup_duplicate_count}" -ge 1 ]] || {
    die "Expected at least one dup-source-{one,two}*.pdf in Documents/Duplicates"
  }
  assert_file "${WATCH_A}/.hidden_should_stay"
  assert_file "${WATCH_A}/pending.download"

  # Collision should not overwrite pre-existing destination.
  if ! grep -Fxq 'old-collision' "${WATCH_A}/Documents/collision.pdf"; then
    die "Expected pre-existing collision file content to remain unchanged"
  fi
  local collision_count
  collision_count="$(find "${WATCH_A}/Documents" -maxdepth 1 -type f -name 'collision*.pdf' | wc -l | tr -d ' ')"
  [[ "${collision_count}" -ge 2 ]] || die "Expected at least 2 collision*.pdf files in Documents"

  run_and_capture out filo arrange -s "${ARRANGE_SRC}" -d "${ARRANGE_DEST}" -k invoice -k receipt -e pdf -y
  assert_contains "${out}" "Arrange complete:"
  assert_file "${ARRANGE_DEST}/2026_invoice_alpha.pdf"
  assert_file "${ARRANGE_DEST}/receipt_gamma.pdf"

  run_and_capture out filo watch list
  assert_contains "${out}" "${WATCH_A}"
  assert_contains "${out}" "${WATCH_B}"

  run_and_capture out filo watch add "${WATCH_C}"
  assert_contains "${out}" "added:"
  run_and_capture out filo watch list
  assert_contains "${out}" "${WATCH_C}"

  run_and_capture out filo watch remove "${WATCH_C}"
  assert_contains "${out}" "removed:"
  run_and_capture out filo watch list
  if printf '%s' "${out}" | grep -Fq "${WATCH_C}"; then
    die "watch remove failed: ${WATCH_C} still listed"
  fi

  # Safe status-only check for autostart.
  filo autostart status >/dev/null

  run_and_capture out filo refresh
  assert_contains "${out}" "not running"

  log "6) Watcher daemon checks"
  filo stop >/dev/null 2>&1 || true
  run_and_capture out filo start --no-scan
  assert_contains "${out}" "daemon started"
  wait_for_daemon || die "Daemon did not become ready in time"

  log "7) Add many files while daemon is running (${LIVE_COUNT} per watched folder)"
  printf 'live-amazon\n' > "${WATCH_B}/amazon_invoice_live.pdf"
  printf 'live-dup\n' > "${WATCH_B}/live_dup_one.pdf"
  printf 'live-dup\n' > "${WATCH_B}/live_dup_two.pdf"

  generate_bulk_files "${WATCH_A}" "live_a" "${LIVE_COUNT}"
  generate_bulk_files "${WATCH_B}" "live_b" "${LIVE_COUNT}"

  wait_for_file "${WATCH_B}/Amazon/amazon-invoice-live.pdf" 120 || {
    die "Watcher did not route amazon_invoice_live.pdf as expected"
  }
  wait_for_match_count "${WATCH_B}/Documents" 'live-dup-*.pdf' 1 120 || {
    die "Watcher did not place any live duplicate in Documents"
  }
  wait_for_match_count "${WATCH_B}/Documents/Duplicates" 'live-dup-*.pdf' 1 120 || {
    die "Watcher did not place any live duplicate in Documents/Duplicates"
  }
  wait_for_actionable_drain "${WATCH_A}" 12 240 || {
    dump_top_level_state "${WATCH_A}" "watch_a (post-daemon)"
    die "Watcher backlog too high in watch_a after timeout"
  }
  wait_for_actionable_drain "${WATCH_B}" 12 240 || {
    dump_top_level_state "${WATCH_B}" "watch_b (post-daemon)"
    die "Watcher backlog too high in watch_b after timeout"
  }

  log "8) Stop daemon"
  run_and_capture out filo stop
  assert_contains "${out}" "stopped"
  run_and_capture out filo refresh
  assert_contains "${out}" "not running"

  log "9) Predictability checks"
  local top_files_a top_files_b actionable_top_a actionable_top_b moved_a moved_b
  top_files_a="$(find "${WATCH_A}" -maxdepth 1 -type f | wc -l | tr -d ' ')"
  top_files_b="$(find "${WATCH_B}" -maxdepth 1 -type f | wc -l | tr -d ' ')"
  actionable_top_a="$(count_actionable_top_files "${WATCH_A}")"
  actionable_top_b="$(count_actionable_top_files "${WATCH_B}")"
  moved_a="$(find "${WATCH_A}" -mindepth 2 -type f | wc -l | tr -d ' ')"
  moved_b="$(find "${WATCH_B}" -mindepth 2 -type f | wc -l | tr -d ' ')"

  # A small number of top-level leftovers is expected (.DS_Store, partial files).
  [[ "${top_files_a}" -le 20 ]] || {
    dump_top_level_state "${WATCH_A}" "watch_a"
    die "Too many top-level leftovers in watch_a (${top_files_a})"
  }
  [[ "${top_files_b}" -le 20 ]] || {
    dump_top_level_state "${WATCH_B}" "watch_b"
    die "Too many top-level leftovers in watch_b (${top_files_b})"
  }
  [[ "${actionable_top_a}" -le 12 ]] || {
    dump_top_level_state "${WATCH_A}" "watch_a actionable"
    die "Too many actionable top-level leftovers in watch_a (${actionable_top_a})"
  }
  [[ "${actionable_top_b}" -le 12 ]] || {
    dump_top_level_state "${WATCH_B}" "watch_b actionable"
    die "Too many actionable top-level leftovers in watch_b (${actionable_top_b})"
  }

  local min_expected
  min_expected=$((BULK_COUNT + LIVE_COUNT - 10))
  [[ "${moved_a}" -ge "${min_expected}" ]] || die "Unexpectedly low moved file count in watch_a (${moved_a})"
  [[ "${moved_b}" -ge "${min_expected}" ]] || die "Unexpectedly low moved file count in watch_b (${moved_b})"

  assert_dir "${WATCH_A}/Amazon"
  assert_dir "${WATCH_A}/Finance"
  assert_dir "${WATCH_A}/Documents"
  assert_dir "${WATCH_A}/Images"
  assert_dir "${WATCH_A}/Other"
  assert_dir "${WATCH_B}/Amazon"
  assert_dir "${WATCH_B}/Finance"
  assert_dir "${WATCH_B}/Documents"

  log "All comprehensive checks passed."
  printf '\nSummary:\n'
  printf '  Config path used: %s\n' "${CONFIG_PATH}"
  printf '  Test root: %s\n' "${TEST_ROOT}"
  printf '  watch_a moved files: %s\n' "${moved_a}"
  printf '  watch_b moved files: %s\n' "${moved_b}"
  if [[ -f "${LOG_PATH}" ]]; then
    printf '  Log file: %s\n' "${LOG_PATH}"
  fi
}

main "$@"
