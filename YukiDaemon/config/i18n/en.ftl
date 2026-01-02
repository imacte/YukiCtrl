# --- Main & Monitor ---
yuki-daemon-starting = Yuki Daemon Unified Starting...
scheduler-module-started = Scheduler module started.
scheduler-module-start-failed = Failed to start scheduler module: { $error }
monitor-module-crashed = Monitor module crashed: { $error }
monitor-module-started = Monitor module started.
monitor-starting = Starting yuki-monitor module...

# Boot
boot-scripts-running = [Boot] Running boot scripts...
boot-script-applying = [Boot] Applying script: { $path }
boot-script-success = [Boot] Script { $name } applied successfully.
boot-script-failed = [Boot] Script { $name } failed: { $error }
boot-script-exec-failed = [Boot] Failed to execute script { $name }: { $error }
boot-scripts-finished = [Boot] Boot scripts execution finished.

# Power
power-cpu-temp-found = [Power] Found CPU temp sensor: { $path }
power-cpu-temp-not-found = [Power] CPU temp path not found: { $error }. Using 0.0.
power-loop-started = [Power] Power monitoring loop started.
power-screen-off-skip = [Power] Screen is off, skipping power poll.
power-charging-stopped = [Power] Charging stopped. Checking session limits...
power-trim-failed = [Power] Failed to trim old sessions: { $error }
power-new-session = [Power] Starting new session: { $id }
power-db-write-failed = [Power] Failed to write power log to DB: { $error }
power-read-failed = [Power] Failed to read voltage or current: { $error }
power-status-read-failed = [Power] Failed to read charging status: { $error }

# DB
db-initialized = [DB] Database initialized at { $path }
db-logged-raw = [DB] Logged raw data: { $vol }uV, { $cur }uA for { $pkg }
db-session-limit-exceeded = [DB] Session count ({ $count }) exceeds limit ({ $limit }). Trimming { $trim } old session(s)...
db-trimmed-entries = [DB] Trimmed { $rows } log entries from { $sessions } old session(s).
db-session-limit-ok = [DB] Session count ({ $count }) is within limit ({ $limit }). No trim needed.

# AppDetect
app-detect-config-watch = [AppDetect] Started watching config file: { $path }
app-detect-change-detected = [AppDetect] Change detected, debouncing for 100ms...
app-detect-reloading = [AppDetect] Debounce finished. Reloading config...
app-detect-load-failed = [AppDetect] Failed: { $error }. Using default.
app-detect-reload-success = [AppDetect] Config reloaded successfully.
app-detect-loop-started = [AppDetect] App detection loop started (3000ms poll).
app-detect-screen-changed = [AppDetect] Screen changed: { $old } -> { $new }
app-detect-mode-change = [AppDetect] Mode change: { $old } -> { $new }
app-detect-mode-change-pkg = [AppDetect] Mode change: { $old } -> { $new } ({ $pkg })

# ScreenDetect
screen-state-change-detected = [Screen] State change detected via '{ $source }'.
screen-state-changed-value = [Screen] Screen state changed: { $state }
screen-netlink-started = [Screen] Started netlink-sys socket listener.

# --- Scheduler (Additions) ---
scheduler-ipc-started = [Scheduler] IPC Channel listener started.
scheduler-mode-change-request = [Scheduler] Mode change request: { $old } -> { $new } (Pkg: { $pkg }, Temp: { $temp })
scheduler-boost-active-ignore = [Scheduler] Boost active, ignoring mode apply.
scheduler-apply-failed = [Scheduler] Failed to apply settings: { $error }
scheduler-channel-closed = [Scheduler] Channel closed! Thread exiting.
config-apply-mode-failed = Failed to apply reloaded mode settings: { $error }
config-apply-tweaks-failed = Failed to apply reloaded system tweaks: { $error }
app-launch-watch-failed = Failed to watch for app launch: { $error }
boost-apply-failed = Failed to apply boost frequencies: { $error }
boost-restore-freq-failed = Failed to restore frequencies: { $error }
boost-mode-changed = Mode changed during boost ({ $old } -> { $new }), applying all settings.
boost-mode-apply-failed = Failed to apply new mode settings after boost: { $error }
boost-get-mode-failed = Could not get current mode in boost loop: { $error }
pidof-failed = Failed to execute pidof for '{ $name }': { $error }
process-not-found = Process '{ $name }' not found, skipping.
cpuset-write-failed = Failed to write to cpuset for { $name }: { $error }
cpuctl-write-failed = Failed to write to cpuctl for { $name }: { $error }
thread-core-allocation-log = Thread core allocation completed.
main-config-watch-thread-create = Main config watcher thread created.
applaunch-detected-boosting-frequencies = App launch detected, boosting frequencies...
boost-finished-restoring-settings = Boost finished, restoring settings.
appLaunchboost-thread-created = AppLaunchBoost thread created.
apply-settings-for-mode = Applying settings for mode: { $mode }
settings-applied-success = Settings for mode '{ $mode }' applied successfully.

# --- Logger ---
log-level-updated = Log level updated to: { $level }