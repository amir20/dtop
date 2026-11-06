#!/usr/bin/env bash
# Records demo.gif from a real Ghostty window, because the app logos and Nerd icons
# need a terminal that draws them. macOS only: needs Ghostty, ffmpeg, Xcode's swift,
# Docker and Screen Recording permission for the terminal running this script.
# The window opens on screen and must stay uncovered while it records.
#
# Usage: scripts/record-demo.sh [output.gif]   (default: demo.gif)
set -euo pipefail

cd "$(dirname "$0")/.."
OUT=${1:-demo.gif}
MAX_WIDTH=2200 # only scaled down from a larger capture (a Retina display)
FPS=15
SECONDS_TO_RECORD=34

WORK=$(mktemp -d)
GO="$WORK/go"

cleanup() {
    [ -z "${MOUSE:-}" ] || swift "$WORK/mouse.swift" "$MOUSE" >/dev/null 2>&1 || true
    docker compose -f scripts/demo-compose.yaml down -t 1 >/dev/null 2>&1 || true
    docker ps -q --filter ancestor=dtop-demo | xargs docker rm -f >/dev/null 2>&1 || true
    docker rmi dtop-demo >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

docker build -q -t dtop-demo . >/dev/null
docker compose -f scripts/demo-compose.yaml up -d >/dev/null
sleep 45 # let the containers settle so the numbers look like a running stack
docker restart -t 1 shop-traffic-1 >/dev/null # one container with a shorter uptime

cat >"$WORK/window.swift" <<'EOF'
import CoreGraphics
let opts = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
for w in CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as! [[String: Any]] {
  guard (w["kCGWindowOwnerName"] as? String) == "Ghostty", (w["kCGWindowLayer"] as? Int) == 0,
        let b = w["kCGWindowBounds"] as? [String: Any] else { continue }
  print("\(b["X"]!),\(b["Y"]!),\(b["Width"]!),\(b["Height"]!)")
  break
}
EOF

# Prints where the mouse is, then moves it to the position given as "x,y"
cat >"$WORK/mouse.swift" <<'EOF'
import CoreGraphics
let at = CGEvent(source: nil)!.location
print("\(at.x),\(at.y)")
let p = CommandLine.arguments[1].split(separator: ",").map { Double($0)! }
CGWarpMouseCursorPosition(CGPoint(x: p[0], y: p[1]))
EOF

# Through a script: Ghostty starts its command via login, which breaks some python shims
cat >"$WORK/run.sh" <<EOF
#!/bin/bash
exec /usr/bin/python3 "$PWD/scripts/record-demo-driver.py" "$GO" 2>"$WORK/driver.log"
EOF
chmod +x "$WORK/run.sh"

open -na Ghostty.app --args \
    --config-default-files=false \
    --theme="Ayu Mirage" \
    --font-size=22 \
    --window-width=150 --window-height=24 \
    --window-position-x=40 --window-position-y=60 \
    --window-padding-x=24 --window-padding-y=20 \
    --window-decoration=none \
    --cursor-style-blink=false \
    --confirm-close-surface=false \
    --quit-after-last-window-closed=true \
    --command="$WORK/run.sh"

sleep 3
RECT=$(swift "$WORK/window.swift")
[ -n "$RECT" ] || { echo "No Ghostty window found" >&2; exit 1; }
echo "Recording window at $RECT"

# The capture includes the mouse pointer, so park it outside the window until we're done
IFS=, read -r X Y W H <<<"$RECT"
MOUSE=$(swift "$WORK/mouse.swift" "$((X + W + 40)),$((Y + H + 40))")

screencapture -x -v -V "$SECONDS_TO_RECORD" -R "$RECT" "$WORK/raw.mov" &
CAPTURE=$!
sleep 1.5
touch "$GO"
wait "$CAPTURE"
[ -f "$GO.done" ] || { echo "The demo script did not finish before capture stopped:" >&2; cat "$WORK/driver.log" >&2; exit 1; }

# Trim the wait before the keys start and stop before dtop quits; the crop drops the
# window's edge line. One palette with no dithering keeps text and solid bars crisp.
ffmpeg -v error -y -ss 1.5 -t 25.5 -i "$WORK/raw.mov" -filter_complex \
    "crop=iw-8:ih-8,fps=$FPS,scale='min($MAX_WIDTH,iw)':-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=256:stats_mode=full[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
    "$OUT"
ls -la "$OUT"
