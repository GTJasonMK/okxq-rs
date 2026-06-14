#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_BIN="${OKXQ_APP_BIN:-$ROOT_DIR/src-tauri/target/release/okxq-rs}"
OUT_DIR="${OKXQ_GUI_SMOKE_OUT_DIR:-/tmp/okxq-gui-smoke}"
WAIT_SECONDS="${OKXQ_GUI_SMOKE_WAIT_SECONDS:-20}"
MIN_WINDOW_AREA="${OKXQ_GUI_SMOKE_MIN_WINDOW_AREA:-100000}"
SCREENSHOT="${OUT_DIR}/okxq-gui-smoke.png"
LOG_FILE="${OUT_DIR}/okxq-gui-smoke.log"
IMPORT_ERR="${OUT_DIR}/import.err"

mkdir -p "$OUT_DIR"
rm -f "$SCREENSHOT" "$LOG_FILE" "$IMPORT_ERR"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 127
  fi
}

require_command xdotool
require_command import
require_command identify

if [ ! -x "$APP_BIN" ]; then
  echo "release binary not found or not executable: $APP_BIN" >&2
  echo "run npm run build first, or set OKXQ_APP_BIN=/path/to/okxq-rs" >&2
  exit 2
fi

if [ -z "${DISPLAY:-}" ]; then
  echo "DISPLAY is empty; GUI smoke requires an active X11/XWayland display" >&2
  exit 2
fi

declare APP_PID=""

cleanup() {
  if [ -n "$APP_PID" ] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

GDK_BACKEND="${GDK_BACKEND:-x11}" RUST_LOG="${RUST_LOG:-okxq_rs=info}" "$APP_BIN" >"$LOG_FILE" 2>&1 &
APP_PID=$!

best_window=""
best_area=0
deadline=$((SECONDS + WAIT_SECONDS))

while [ "$SECONDS" -lt "$deadline" ]; do
  while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    geometry="$(xdotool getwindowgeometry --shell "$candidate" 2>/dev/null || true)"
    width="$(printf '%s\n' "$geometry" | sed -n 's/^WIDTH=//p')"
    height="$(printf '%s\n' "$geometry" | sed -n 's/^HEIGHT=//p')"
    case "$width:$height" in
      *[!0-9:]*|:|*:|'') continue ;;
    esac
    area=$((width * height))
    if [ "$area" -gt "$best_area" ]; then
      best_area="$area"
      best_window="$candidate"
    fi
  done < <(xdotool search --pid "$APP_PID" 2>/dev/null || true)

  if [ "$best_area" -ge "$MIN_WINDOW_AREA" ]; then
    break
  fi
  sleep 0.5
done

if [ -z "$best_window" ] || [ "$best_area" -lt "$MIN_WINDOW_AREA" ]; then
  echo "GUI smoke failed: main window not found; best_area=${best_area}; pid=${APP_PID}" >&2
  echo "--- app log tail ---" >&2
  tail -n 120 "$LOG_FILE" >&2 || true
  exit 3
fi

window_name="$(xdotool getwindowname "$best_window" 2>/dev/null || true)"
geometry="$(xdotool getwindowgeometry --shell "$best_window")"
width="$(printf '%s\n' "$geometry" | sed -n 's/^WIDTH=//p')"
height="$(printf '%s\n' "$geometry" | sed -n 's/^HEIGHT=//p')"
x_pos="$(printf '%s\n' "$geometry" | sed -n 's/^X=//p')"
y_pos="$(printf '%s\n' "$geometry" | sed -n 's/^Y=//p')"

xdotool windowactivate "$best_window" 2>/dev/null || true
sleep 1

if ! import -silent -window "$best_window" "$SCREENSHOT" 2>"$IMPORT_ERR"; then
  echo "GUI smoke failed: screenshot capture failed" >&2
  cat "$IMPORT_ERR" >&2 || true
  exit 4
fi

if ! identify "$SCREENSHOT" >/dev/null; then
  echo "GUI smoke failed: screenshot is not a valid image: $SCREENSHOT" >&2
  exit 5
fi

missing_markers=0
for marker in \
  'external strategy scan completed' \
  'configuration loaded' \
  'sqlite database ready' \
  'application state bootstrapped'
do
  if ! grep -q "$marker" "$LOG_FILE"; then
    echo "missing log marker: $marker" >&2
    missing_markers=1
  fi
done

if [ "$missing_markers" -ne 0 ]; then
  echo "--- app log tail ---" >&2
  tail -n 120 "$LOG_FILE" >&2 || true
  exit 6
fi

identify_line="$(identify "$SCREENSHOT")"
echo "GUI_SMOKE_OK"
echo "scope=launch_window_screenshot_bootstrap_only"
echo "interaction_tested=no"
echo "pid=$APP_PID"
echo "window_id=$best_window"
echo "window_name=$window_name"
echo "window_geometry=${width}x${height}+${x_pos}+${y_pos}"
echo "screenshot=$SCREENSHOT"
echo "screenshot_info=$identify_line"
echo "log=$LOG_FILE"
