# GPU Tab Implementation — Completed

## Changes Made

### 1. `src/app/state.rs`
- Added `Gpu` variant to `AppTab` enum

### 2. `src/app/mod.rs`
- Updated `next_tab()` and `prev_tab()` to include `Gpu` tab in the cycle
- Added `gpu_scroll: usize` field for multi-GPU navigation
- Added `gpu_scroll()`, `gpu_scroll_down()`, `gpu_scroll_up()` accessor methods
- Added GPU-specific navigation in `navigate_down()` and `navigate_up()` — on GPU tab, up/down scrolls through GPUs
- Added `"gpu"` match arm in `default_tab` parsing

### 3. `src/app/events.rs`
- Added `KeyCode::Char('7')` binding to switch to GPU tab
- Updated `KeyCode::Char('g')` to no-op on GPU tab (avoids accidental CPU mode toggle)
- Updated mouse click tab detection to support 6 tabs (was 5)

### 4. `src/ui/tabs/gpu.rs` (NEW)
- `render_gpu_tab()` — main entry point, supports multi-GPU with side-by-side layout
- `render_gpu_card()` — renders single GPU with:
  - GPU name as border title (with Nerd Font icon)
  - Usage gauge (color-coded)
  - VRAM gauge (with MiB label)
  - Temperature bar (with °C/°F toggle support)
  - Power draw (W)
  - Fan speed (%)
  - Clock speed (MHz)
  - Focus panel support (PanelFocus)
- `render_no_gpu()` — placeholder when no GPU detected
- Multi-GPU scroll: shows 2 GPUs side-by-side, scroll with up/down for more

### 5. `src/ui/tabs/mod.rs`
- Registered `pub mod gpu;`

### 6. `src/ui/mod.rs`
- Added `AppTab::Gpu` match arm routing to `tabs::gpu::render_gpu_tab()`

### 7. `src/ui/header.rs`
- Added GPU tab to the tab bar with Nerd Font GPU icon

### 8. `src/ui/footer.rs`
- Added GPU tab context keybindings: Help, Temp unit, Scroll GPU, Quit

### 9. `src/config.rs`
- Added `"gpu"` to `valid_tabs` array for default_tab validation

## Design
- Follows existing tab patterns (panel_block_focused, palette colors, icons module)
- Uses Gauge widgets for usage and VRAM (consistent with Hardware tab)
- Temperature uses block bar characters (consistent with Hardware tab)
- Focus panel highlighting via PanelFocus enum
- Multi-GPU: up/down arrows scroll, side-by-side layout for up to 2 GPUs
