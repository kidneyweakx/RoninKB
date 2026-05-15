#!/usr/bin/env bash
# Parse `hhkb_daemon::backend::macos_native::latency` trace lines from the
# daemon's stderr, pair `rx` (observed input) with the next `tx` (re-injected
# output) on the M1 Caps→Esc/LCtrl HoldTap binding, and report
# min/median/p95/p99/max latency in milliseconds.
#
# See `docs/m1-killswitch-validation.md` Test 1 for the kill-switch criterion
# (p99 < 250ms, max < 300ms).
#
# Usage:
#   RUST_LOG=hhkb_daemon=trace cargo run -p hhkb-daemon 2> daemon.log
#   # ...tap Caps Lock 100 times at ~1Hz...
#   scripts/measure-tap-latency.sh < daemon.log
#
#   # Or pipe directly:
#   RUST_LOG=hhkb_daemon=trace cargo run -p hhkb-daemon 2>&1 \
#     | scripts/measure-tap-latency.sh
#
# Pairing rule: each `tx` line is paired with the most recent unconsumed
# `rx` line whose `phase=rx`. The reported latency is `tx.t_us - rx.t_us`,
# converted to milliseconds. This matches the validation plan's definition
# (initial input observation → first re-injected output) and works for both
# pure remap (1 rx → 1 tx) and HoldTap (Caps press → Esc press for tap, or
# Caps press → LCtrl press for hold).
#
# Bash + awk only, no python dependency. macOS / Linux compatible.

set -euo pipefail

awk '
  # Match latency-channel lines and extract phase + t_us.
  /hhkb_daemon::backend::macos_native::latency/ {
    phase = ""
    t_us  = ""
    for (i = 1; i <= NF; i++) {
      if ($i ~ /^phase=/) {
        phase = $i
        sub(/^phase=/, "", phase)
        gsub(/"/, "", phase)
      }
      if ($i ~ /^t_us=/) {
        t_us = $i
        sub(/^t_us=/, "", t_us)
      }
    }
    if (phase == "" || t_us == "") next

    if (phase == "rx") {
      # Push onto a tiny ring buffer; we only need the most recent unconsumed
      # rx for each tx, but keep up to 64 to handle bursts.
      rx_queue[++rx_tail] = t_us
    } else if (phase == "tx") {
      if (rx_tail < rx_head + 1) {
        # Stray tx with no preceding rx; rare under the test (idle tick
        # release of a stale modifier on hot-swap), skip it so the
        # latency stats stay honest.
        next
      }
      rx_head++
      lat_us = t_us - rx_queue[rx_head]
      if (lat_us < 0) next   # clock skew between threads (should not happen with shared Instant origin)
      samples[++n] = lat_us
    }
  }

  END {
    if (n == 0) {
      print "no (rx, tx) pairs found in input — did you set RUST_LOG=hhkb_daemon=trace and tap an owned key?" > "/dev/stderr"
      exit 2
    }

    # Sort samples ascending using a simple in-place insertion sort (n is
    # bounded by the human typing rate; insertion sort is fine here and
    # avoids depending on `sort -n`).
    for (i = 2; i <= n; i++) {
      key = samples[i]
      j = i - 1
      while (j >= 1 && samples[j] > key) {
        samples[j+1] = samples[j]
        j--
      }
      samples[j+1] = key
    }

    sum = 0
    for (i = 1; i <= n; i++) sum += samples[i]
    mean_us = sum / n

    min_us    = samples[1]
    max_us    = samples[n]
    median_us = pct(samples, n, 50)
    p95_us    = pct(samples, n, 95)
    p99_us    = pct(samples, n, 99)

    printf "samples: %d\n", n
    printf "min:     %.1f ms\n", min_us / 1000
    printf "median:  %.1f ms\n", median_us / 1000
    printf "mean:    %.1f ms\n", mean_us / 1000
    printf "p95:     %.1f ms\n", p95_us / 1000
    printf "p99:     %.1f ms\n", p99_us / 1000
    printf "max:     %.1f ms\n", max_us / 1000

    # Kill-switch verdict per docs/m1-killswitch-validation.md Test 1.
    fail = 0
    if (p99_us / 1000 >= 250) { printf "FAIL: p99 >= 250ms\n"; fail = 1 }
    if (max_us / 1000 >= 300) { printf "FAIL: max >= 300ms\n"; fail = 1 }
    if (fail == 0) printf "PASS: p99 < 250ms and max < 300ms\n"
    exit fail
  }

  function pct(arr, count, p,    rank) {
    # Nearest-rank percentile: ceil(p/100 * n).
    rank = int((p / 100) * count + 0.999999)
    if (rank < 1) rank = 1
    if (rank > count) rank = count
    return arr[rank]
  }
'
