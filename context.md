# Code Context — Performance Hot Path Analysis

## Files Retrieved
1. `src/main.rs` (full) — Main loop, async collector tasks, state update channel
2. `src/app/mod.rs` (full) — App struct, accessors, setters, refresh logic
3. `src/app/state.rs` (full) — All data types, constants
4. `src/app/events.rs` (full) — Event dispatch and key handling
5. `src/app/processes.rs` (full) — Process list building
6. `src/app/helpers.rs` (full) — History buffer helpers
7. `src/app/collectors/cpu.rs` (full) — CPU refresh
8. `src/app/collectors/network.rs` (full) — Network refresh
9. `src/app/collectors/disk.rs` (full) — Disk I/O + partition enumeration
10. `src/app/collectors/sensors.rs` (full) — Temperature + battery
11. `src/app/collectors/gpu.rs` (full) — GPU stats (nvidia-smi + AMD sysfs)
12. `src/app/collectors/logs.rs` (full) — Journalctl/dmesg log collector
13. `src/app/collectors/hardware.rs` (full) — Static hardware data (one-shot)
14. `src/ui/mod.rs` (full) — UI draw dispatch
15. `src/ui/tabs/dashboard.rs` (full) — Dashboard tab render
16. `src/ui/tabs/system.rs` (full) — System tab render
17. `src/ui/tabs/processes.rs` (full) — Processes tab + tree view
18. `src/ui/tabs/hardware.rs` (full) — Hardware tab render
19. `src/ui/tabs/logs.rs` (full) — Logs tab render
20. `src/ui/widgets/sparkline.rs` (full) — Braille graph engine
21. `src/ui/helpers.rs` (full) — UI utility functions

---

## Finding 1 — CRITICAL: `system_info()` allocates 14+ heap Strings every frame, called multiple times per render

**Files:** `src/app/mod.rs:349–374`, called from `dashboard.rs:131`, `dashboard.rs:378`, `system.rs:131`, `hardware.rs:363`

`system_info()` is a method on `App` that constructs a brand-new `SystemInfo` struct each time it is called. The struct owns 14 `String` fields. It also makes multiple system calls (`System::long_os_version()`, `System::kernel_version()`, `System::host_name()`, `System::uptime()`, `System::load_average()`, four `std::env::var()` calls, and `cpu_brand().to_string()`).

On the Dashboard tab alone, it is called **twice per frame** (once in `render_memory_panel`, once in `render_system_disk_panel`). On the System tab, once per frame. On the Hardware tab, once per frame. Each frame = ~30-60 heap allocations just for this.

```rust
// src/app/mod.rs:349–374
pub fn system_info(&self) -> SystemInfo {
    let secs = System::uptime();        // syscall
    let load = System::load_average();   // syscall
    let desktop_env = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "Unknown".to_string());       // String alloc
    let display_server = std::env::var("WAYLAND_DISPLAY")
        .map(|_| "Wayland".to_string())                    // String alloc
        .or_else(|_| std::env::var("DISPLAY").map(|_| "X11".to_string()))
        .unwrap_or_else(|_| "Unknown/TTY".to_string());   // String alloc
    // ...
    SystemInfo {
        os_name: System::long_os_version().unwrap_or_else(|| "Unknown".into()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".into()),
        hostname: System::host_name().unwrap_or_else(|| "Unknown".into()),
        uptime: format!("{}d {}h {}m", days, hours, mins),   // format! → String
        cpu_brand: self.sys.cpus().first().map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown".into()),
        sys_vendor: hw.sys_vendor.clone(),     // clone
        product_name: hw.product_name.clone(), // clone
        bios_version: hw.bios_version.clone(), // clone
        // ...14 total String fields
    }
}
```

**Fix:** Cache `SystemInfo` in the `App` struct and refresh it on a slow interval (e.g., every 10s). Most fields never change during runtime.

---

## Finding 2 — CRITICAL: Deep clone of entire CPU/per-core history every ~1s in the hot collector

**File:** `src/main.rs:227–228`

```rust
// src/main.rs:226–228 (inside Tier 1+2 tokio task, runs every ~1s)
let _ = tx_fast.send(StateUpdate::CpuMemoryNetDisk {
    cpu_history: cpu_history.clone(),        // VecDeque<u64> × 60 elements
    per_core_history: per_core_history.clone(), // Vec<VecDeque<u64>> × N_cores × 60
```

`cpu_history` is a `VecDeque<u64>` with up to 60 entries — cheap but unnecessary. `per_core_history` is `Vec<VecDeque<u64>>` where each inner deque has 60 entries. On an 8-core machine, this clones 480 `u64` values + 8 VecDeque headers + 1 Vec header every second.

**Fix:** Use `std::mem::take` / swap pattern: swap out the history from the collector, send the owned data through the channel, and rebuild an empty history in the collector. Or wrap in `Arc<Mutex<...>>` or use `bytes::Bytes`-style copy-on-write.

---

## Finding 3 — CRITICAL: Blocking `/proc` and `/sys` file I/O inside plain `tokio::spawn` tasks (not `spawn_blocking`)

**Files:** `src/main.rs:157–238` (Tier 1+2 task), `src/app/collectors/disk.rs:42–98`

The background collector tasks use plain `tokio::spawn`, but perform blocking operations:

```rust
// src/main.rs:159 — plain tokio::spawn
tokio::spawn(async move {
    loop {
        tokio::time::sleep(interval).await;
        sys.refresh_cpu_all();      // CPU-bound sysinfo refresh
        sys.refresh_memory();       // reads /proc/meminfo internally
        networks.refresh(false);     // reads /proc/net/dev internally
        sys.refresh_processes(...);  // reads /proc/*/stat
        // ...
    }
});
```

Inside the loop, `collectors::disk::refresh_disk()` calls `read_diskstats()`:

```rust
// src/app/collectors/disk.rs:42
fn read_diskstats() -> (u64, u64, Option<u64>, Option<u64>) {
    let content = match fs::read_to_string("/proc/diskstats") { // BLOCKING
        Ok(c) => c,
        Err(_) => return (0, 0, None, None),
    };
```

`sysinfo::System::refresh_*` methods also perform blocking reads from `/proc`. Similarly, the Tier 3 sensor task calls `fs::read_to_string` on `/sys/class/power_supply/BAT*/*` files, and the GPU task spawns `nvidia-smi` via `Command::new`.

All of these block a tokio worker thread, reducing concurrency. On a single-threaded runtime (common for TUI apps), this blocks the entire event loop.

**Fix:** Wrap each heavy blocking I/O section in `tokio::task::spawn_blocking(|| { ... })`. Or restructure so the main loop uses a dedicated blocking thread.

---

## Finding 4 — HIGH: Sparkline renderer allocates a `String` per braille cell

**File:** `src/ui/widgets/sparkline.rs:177, 267, 281, 295`

```rust
// src/ui/widgets/sparkline.rs:177 (braille_line_graph, inside per-column loop)
let ch = char::from_u32(BRAILLE_OFFSET + bits as u32).unwrap_or(' ');
spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
//                         ^^^^^^^^^^^^^^ heap allocation per cell
```

The same pattern appears in `braille_mirrored_graph()` at lines 267, 281, and 295:

```rust
// src/ui/widgets/sparkline.rs:267
spans.push(Span::styled(ch.to_string(), Style::default().fg(up_color)));
```

For a graph area of 80×10 = 800 cells, this creates 800 heap-allocated `String` objects per graph per frame. When the dashboard renders CPU graph + network mirrored graph + disk I/O mirrored graph, that's 2400+ String allocations.

**Fix:** Use `Span::raw(ch)` which takes a `char` or `&str` directly (ratatui's `Span` constructor accepts `char` in some variants), or pre-allocate a static lookup table for braille characters as `&'static str`.

---

## Finding 5 — HIGH: `filtered_processes()` allocates a `Vec` on every call; called repeatedly

**File:** `src/app/mod.rs:207–218`

```rust
// src/app/mod.rs:207
pub fn filtered_processes(&self) -> Vec<&ProcessEntry> {
    if !self.filter_active || self.filter_input.is_empty() {
        self.top_processes.iter().collect()  // allocates Vec<&ProcessEntry>
    } else {
        let query = self.filter_input.to_lowercase();  // allocates String
        self.top_processes
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&query)) // to_lowercase() per process
            .collect()
    }
}
```

This is called from:
- `navigate_down()` / `navigate_up()` / `navigate_page_down()` / `navigate_page_up()` / `navigate_home()` / `navigate_end()` — navigation methods
- `clamp_selection()` — after filter changes
- `request_kill()` — on kill request
- `render_top_processes()` in dashboard.rs:178
- `render_process_table()` in processes.rs:113
- `render_tree_view()` in processes.rs:285

During a single keypress event, the flow is: `handle_event → navigate_down → filtered_processes()` (for length check), then `draw → render_process_table → filtered_processes()` again. **That's 2 allocations of the full Vec per navigation keypress.** And when filtering is active, every process name gets `.to_lowercase()` creating a temporary String.

**Fix:** Cache filtered results and invalidate on filter/process list change. Use a lazy/update-on-mutate pattern.

---

## Finding 6 — HIGH: `per_core_usage()` allocates `Vec<f32>` on every render call

**File:** `src/app/mod.rs:240–244`

```rust
// src/app/mod.rs:240
pub fn per_core_usage(&self) -> Vec<f32> {
    self.per_core_history.iter()
        .map(|h| h.back().copied().unwrap_or(0) as f32)
        .collect()  // allocates Vec<f32> with N_cores elements
}
```

Called from:
- `render_cpu_panel()` in `system.rs:173`
- `render_cpu_panel()` in `hardware.rs:140`

This runs every frame for the System and Hardware tabs. Minor allocation but easily avoidable.

**Fix:** Store as a cached `[SmallVec<f32>; N]` or return an iterator.

---

## Finding 7 — MEDIUM: Log entries cloned via channel every 5s

**File:** `src/main.rs:273–274`

```rust
// src/main.rs:273-274 (Tier 4 log task, every 5s)
let _ = tx_logs.send(StateUpdate::Logs {
    entries: log_collector.entries().clone(),  // clones entire VecDeque<LogEntry>
});
```

Each `LogEntry` contains 3 Strings (`timestamp`, `message` + `level` enum). With up to 500 entries (MAX_LOG_LINES), this clones 1000+ Strings every 5 seconds.

**Fix:** Use `std::mem::take` to swap out the entries instead of cloning.

---

## Finding 8 — MEDIUM: Battery collector reads 8+ individual sysfs files per refresh

**File:** `src/app/collectors/sensors.rs:81–118`

```rust
// src/app/collectors/sensors.rs:81–118 (read_battery, called every 5s)
let cap = fs::read_to_string(path.join("capacity")).ok()?;           // 1
let status = fs::read_to_string(path.join("status"))...;           // 2
fs::read_to_string(path.join("power_now"))...;                     // 3
fs::read_to_string(path.join("current_now"))...;                   // 4
fs::read_to_string(path.join("voltage_now"))...;                    // 5
fs::read_to_string(path.join("manufacturer"))...;                  // 6
fs::read_to_string(path.join("model_name"))...;                    // 7
fs::read_to_string(path.join("technology"))...;                     // 8
fs::read_to_string(path.join("cycle_count"))...;                    // 9
fs::read_to_string(path.join("charge_full"))...;                    // 10
fs::read_to_string(path.join("energy_full"))...;                    // 11
fs::read_to_string(path.join("charge_full_design"))...;             // 12
fs::read_to_string(path.join("energy_full_design"))...;             // 13
```

Up to 13 separate `fs::read_to_string` calls, each opening and closing a file descriptor. While individual files are small, this is sysfs overhead multiplied by 13 per refresh cycle.

**Fix:** Batch-read into a single `std::fs::read_dir` + buffered approach, or at minimum cache values that rarely change (manufacturer, model, technology, cycle_count).

---

## Finding 9 — MEDIUM: Disk partition enumeration reads many individual sysfs files (Tier 5)

**File:** `src/app/collectors/disk.rs:133–192` (`enumerate_partitions` + `disk_hardware_info` + `sys_attr` + `is_ssd`)

For each disk, `disk_hardware_info()` calls:
- `parent_block_dev()` — string parsing, no I/O
- `is_ssd()` — reads `/sys/block/<dev>/queue/rotational` (1 file)
- `sys_attr()` — tries up to 2 paths each for `model`, `vendor`, `serial`, `device/model`, `device/serial` = up to 10 file reads per disk

On a system with 3 partitions backed by 2 physical disks, that's ~23 `fs::read_to_string` calls every 10 seconds.

**Fix:** Cache disk hardware info per block device name (it never changes at runtime).

---

## Finding 10 — MEDIUM: Process tree rebuild re-clones all entries on every render

**File:** `src/ui/tabs/processes.rs:227–228, 276, 354, 361`

```rust
// src/ui/tabs/processes.rs:227
fn build_node(...) -> Option<TreeNode> {
    let entry = (*pid_map.get(&pid)?).clone();  // clones ProcessEntry (2 Strings)
    // ...
}

// src/ui/tabs/processes.rs:276
rows.push(TreeRow {
    name: node.entry.name.clone(),  // another clone
    // ...
});

// src/ui/tabs/processes.rs:354, 361
let tree_prefix = if row.indent.is_empty() { String::new() } else { row.indent.clone() };
let name_display = if row.name.len() > 20 {
    format!("{}...", &row.name[..17])
} else {
    row.name.clone()  // yet another clone
};
```

The tree view is rebuilt from scratch on every render frame in the Processes tab. For 100+ processes, this creates ~100 `TreeNode` structs (each cloning a `ProcessEntry` with `name: String`), then ~100 `TreeRow` structs with more clones.

**Fix:** Build tree once when process list updates, cache it, and only rebuild on data change.

---

## Finding 11 — MEDIUM: `AppMode::clone()` on every keypress in event dispatch

**File:** `src/app/events.rs:14`

```rust
// src/app/events.rs:14
Event::Key(key) if key.kind == KeyEventKind::Press => {
    match app.mode().clone() {  // clones AppMode enum (Copy type is not derived)
```

`AppMode` is `#[derive(Clone)]` but NOT `Copy`. Every keypress clones a small enum. This is minor but trivially fixable.

**Fix:** Derive `Copy` for `AppMode` (it contains no heap data).

---

## Finding 12 — MEDIUM: Missing `Vec::with_capacity` in hot-path collectors

**File:** `src/app/processes.rs:51`

```rust
// src/app/processes.rs:51
let mut procs: Vec<_> = sys.processes().iter()
    .filter(|(_, p)| !p.name().is_empty())
    .collect();  // no capacity hint — could be 200-500+ processes
```

**File:** `src/app/collectors/sensors.rs:21`

```rust
// src/app/collectors/sensors.rs:21
let fresh: Vec<(String, f32)> = components.list().iter()
    .filter_map(...)
    .collect();  // no capacity hint
```

**File:** `src/ui/widgets/sparkline.rs:139, 243–244`

```rust
// sparkline.rs:139 (braille_line_graph)
let data_vec: Vec<u64> = data.iter().copied().collect();  // no capacity

// sparkline.rs:243-244 (braille_mirrored_graph)
let up_vec: Vec<u64> = up_data.iter().copied().collect();   // no capacity
let down_vec: Vec<u64> = down_data.iter().copied().collect(); // no capacity
```

**File:** `sparkline.rs:157`

```rust
let line_v: Vec<usize> = samples.iter().map(...).collect();  // no capacity
```

**Fix:** Add `with_capacity(data.len())` hints throughout.

---

## Finding 13 — LOW: `build_process_list` double-converts process names

**File:** `src/app/processes.rs:64`

```rust
// src/app/processes.rs:64
name: p.name().to_string_lossy().to_string(),
//      ^^^^^^^^^^^^^^^^^^^^^ creates Cow<str>
//                               ^^^^^^^^^^^ creates owned String from Cow
```

`sysinfo`'s `name()` returns `&OsStr`. The `to_string_lossy().to_string()` creates a `Cow<str>` then immediately converts to `String`. While not a huge cost, it can be simplified.

**Fix:** Use `p.name().to_string_lossy().into_owned()` or directly `String::from_utf8_lossy(p.name().as_bytes()).to_string()`. Or better: if the name is valid UTF-8 (common), use `p.name().to_str().unwrap_or_default().to_string()`.

---

## Finding 14 — LOW: `filtered_log_entries()` allocates Vec + per-entry `to_lowercase()` during filtering

**File:** `src/app/mod.rs:485–497`

```rust
pub fn filtered_log_entries(&self) -> Vec<&LogEntry> {
    self.log_entries().iter()
        .filter(|e| self.log_level_filter.allows(&e.level))
        .filter(|e| {
            if !self.log_filter_active || self.log_filter_input.is_empty() {
                true
            } else {
                let query = self.log_filter_input.to_lowercase(); // alloc per entry!
                e.message.to_lowercase().contains(&query)         // alloc per entry!
            }
        })
        .collect()
}
```

When log filter is active, `self.log_filter_input.to_lowercase()` is called inside the iterator closure — once **per log entry** (up to 500 times). The query should be hoisted out of the closure.

**Fix:** Compute `query` once before the `.filter()` call.

---

## Architecture

```
main.rs::main()
  ├── App::new(config)           — one-shot init, fetches hardware
  ├── spawn_collector_tasks(tx)  — 4 tokio::spawn tasks (blocking I/O!)
  │   ├── Tier 1+2 (~1s)        — CPU, RAM, Net, Disk, Processes
  │   ├── Tier 3 (~5s)           — Sensors, Battery, GPU
  │   ├── Tier 4 (~5s)           — Logs (journalctl/dmesg)
  │   └── Tier 5 (~10s)          — Disk partitions
  └── run_async_app(terminal, app, rx)
      └── tokio::select! loop
          ├── crossterm event poll (spawn_blocking, 50ms)
          └── rx.recv() → apply_state_update → redraw
```

Data flows: Background tokio tasks → mpsc::channel → main async loop → App state mutation → `terminal.draw(|f| ui::draw(f, app))`.

The render path: `ui::draw → tab router → tab render functions → App accessors (system_info, filtered_processes, per_core_usage, etc.) → ratatui widget construction`.

---

## Start Here

**`src/app/mod.rs:349`** — The `system_info()` method is the single highest-impact optimization target. Caching this struct eliminates ~14 String heap allocations × 2+ calls per frame. After that, **`src/ui/widgets/sparkline.rs:177`** (per-cell String allocations in braille renderer) and **`src/main.rs:227`** (deep history clone) are the next highest-impact items.

---

## Supervisor coordination

No coordination needed — all findings are complete and reported above.
