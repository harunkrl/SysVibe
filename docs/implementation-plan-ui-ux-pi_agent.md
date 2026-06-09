# SysVibe UI/UX İyileştirme — Implementation Plan

**Hazırlayan:** Pi Agent (Coder)  
**Tarih:** 2026-06-09  
**Onay Durumu:** Antigravity (Architect) onayı bekleniyor  
**Temel:** `docs/ui-ux-audit-report-pi_agent.md` + `docs/ui-ux-audit-report-antigravity.md`

---

## Genel Yaklaşım

Her görev kendi commit'inde, test edilebilir şekilde teslim edilecek. Her commit öncesi `cargo build` + `cargo test` + görsel doğrulama (manuel) yapılacak. Fazlar sıralı bağımlılıklar içerir — Faz 1 tamamlanmadan Faz 2'ye geçilmeyecek.

---

## Faz 1: Kritik Teknik Borçlar (P0)

### Görev 1.1 — Public IP Cache Doğrulaması

**Sorun:** `app.public_ip()` her render frame'inde çağrılıyor (hardware.rs:372).  
**Gerçek:** Kod incelemesinde `spawn_public_ip_resolve()` fonksiyonu zaten arka plan thread'inde çalışıyor ve `Arc<Mutex<Option<String>>>` ile cache'liyor. Bu görevde sadece UI tarafında gereksiz çağrı olup olmadığını doğrulayıp, varsa optimize edeceğiz.

**Değişecek Dosyalar:**
- `src/ui/tabs/hardware.rs` — `render_network_panel` fonksiyonu

**Adımlar:**
1. `spawn_public_ip_resolve()` mekanizmasını doğrula — zaten async ve cached mi?
2. Eğer her tick'te yeniden spawn ediliyorsa, `already resolved` kontrolünü güçlendir
3. UI tarafında `app.public_ip()` çağrısı zaten cached değeri okuyor mu teyit et
4. Gerekirse rate-limiting ekle (her 60 saniyede bir yeniden çözümle)

**Doğrulama:**
- `cargo build` başarılı
- TUI açılıp public IP gösterildiğinde lag yok
- İkinci render'da tekrar network sorgusu yapılmadığı log ile doğrulanabilir

**Tahmini süre:** 30 dk — mevcut implementasyon zaten çoğu sorununu çözüyor olabilir

---

### Görev 1.2 — Fare ile Sekme Tıklama Düzeltmesi

**Sorun:** `events.rs:224-248` — sabit `terminal_width: 120` ve `total_tab_width: 90` ile çalışıyor. Gerçek terminal boyutunu ve sekme pozisyonlarını hesaba katmıyor.

**Değişecek Dosyalar:**
- `src/app/state.rs` — Yeni `TabRects` yapısı ekle
- `src/app/mod.rs` — `tab_rects` field ve accessor ekle
- `src/ui/header.rs` — Render sırasında sekme Rect koordinatlarını kaydet
- `src/app/events.rs` — Mouse handler'ı gerçek koordinatlarla güncelle

**Adımlar:**

1. **State'e Rect saklama alanı ekle:**
```rust
// state.rs
pub struct TabRectEntry {
    pub tab: AppTab,
    pub x_start: u16,
    pub x_end: u16,
}
```

2. **App struct'ına ekle:**
```rust
// mod.rs — App struct'ına
tab_hit_regions: Vec<TabRectEntry>,
pub fn set_tab_hit_regions(&mut self, regions: Vec<TabRectEntry>) { ... }
pub fn tab_hit_regions(&self) -> &[TabRectEntry] { ... }
```

3. **Tab pozisyon hesaplamasını `draw()` seviyesine taşı:**
   - `draw()` fonksiyonu zaten `&mut App` alıyor — `RefCell`'e gerek yok
   - `calculate_tab_hit_regions(area, app) -> Vec<TabRectEntry>` ayrı bir hesaplama fonksiyonu oluştur
   - Bu fonksiyonu `draw()` içinde `render_header()` çağrısından sonra çalıştır
   - `app.set_tab_hit_regions(regions)` ile kaydet
   - `render_header` fonksiyonunun imzası değişmez (`&App` olarak kalır)

4. **Mouse handler'ı güncelle:**
```rust
// draw() içinde, header render'dan sonra:
let hit_regions = calculate_tab_hit_regions(chunks[0], app);
app.set_tab_hit_regions(hit_regions);

// events.rs mouse handler:
fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.row <= 2 {
                let col = mouse.column;
                for region in app.tab_hit_regions() {
                    if col >= region.x_start && col <= region.x_end {
                        app.set_tab(region.tab);
                        break;
                    }
                }
            }
        }
        // ... scroll handler'lar aynı kalır
    }
}
```

**Doğrulama:**
- `cargo build` başarılı
- Farklı terminal genişliklerinde (80, 120, 200 kolon) fare tıklama testi
- Her sekme tıklaması doğru sekmeye geçmeli
- Sekmeler arası boşluklara tıklama hiçbir şey yapmamalı

**Tahmini süre:** 2-3 saat

---

### Görev 1.3 — Sekme Geçişinde Panel Odak Sıfırlama

**Sorun:** `PanelFocus` global olarak tutuluyor, sekme değişince odak doğru panele denk gelmeyebiliyor. `set_tab()` fonksiyonu panel focus'u sıfırlamıyor.

**Değişecek Dosyalar:**
- `src/app/mod.rs` — `set_tab()`, `next_tab()`, `prev_tab()` fonksiyonları

**Adımlar:**

1. **`set_tab` fonksiyonuna odak sıfırlama ekle:**
```rust
pub fn set_tab(&mut self, tab: AppTab) {
    if self.tab != tab {
        self.tab = tab;
        self.panel_focus = PanelFocus::Panel1; // Yeni sekmeye geçince Panel1'e sıfırla
    }
}
```

2. **`next_tab` ve `prev_tab`'ı güncelle** — bunlar zaten `set_tab` kullanmıyor, doğrudan `self.tab` atıyor. Onları da `set_tab` üzerinden geçir veya aynı sıfırlamayı ekle.

**Doğrulama:**
- `cargo build` başarılı
- Hardware sekmesinde Panel3'e odaklan → Dashboard'a geç → `[` tuşuna bas → Panel1'den başlamalı
- Logs sekmesinde herhangi bir panele odak → Processes'e geç → `[` ile Panel1'den başlamalı

**Tahmini süre:** 30 dk

---

### Görev 1.4 — Gauge Layout Refactor: Overlay → Layout::split()

**Sorun:** `hardware.rs`, `gpu.rs`, `system.rs`'de gauge'lar tek bir büyük Paragraph üzerine `row_idx` bazlı overlay ediliyor. Bir text satırı eklendiğinde gauge'lar yanlış pozisyona kayar.

**Antigravity Review Notu:** "Overlay + line_index saymak ratatui'de anti-pattern'dir. `Layout::split()` ile paneli satırlara bölme yaklaşımı kullanılmalıdır."

**Değişecek Dosyalar:**
- `src/ui/tabs/hardware.rs` — `render_memory_panel`
- `src/ui/tabs/gpu.rs` — `render_gpu_card`
- `src/ui/tabs/system.rs` — `render_battery`

**Yeni Yaklaşım:** Panel içeriğini mantıksal bölümlere ayır, her bölüm için `Layout::split()` ile ayrı Rect hesapla, overlay yerine doğrudan bölüme render et.

**Örnek: Memory Panel Refactor (hardware.rs)**

Mevcut yapı:
```
Paragraph (tüm text satırları + boş placeholder'lar)
  → overlay: Gauge'lar placeholder satırların üzerine bindirilir
```

Yeni yapı:
```
Layout::split() ile inner rect'i bölümlere ayır:
  [0] RAM text satırları  → Paragraph
  [1] RAM gauge          → Gauge widget (direkt render)
  [2] SWAP text satırları → Paragraph
  [3] SWAP gauge          → Gauge widget (direkt render)
```

**Adımlar:**

1. **Memory panel'i refactor et (`render_memory_panel`):**
   - İçeriği mantıksal bölümlere ayır: RAM bölümü (text + gauge), SWAP bölümü (text + gauge)
   - Her bölümün yüksekliğini hesapla (text satır sayısı + gauge için 1 satır)
   - `Layout::default().direction(Vertical).constraints([...])` ile Rect'leri ayır
   - Her Rect'e bağımsız render yap (text → Paragraph, gauge → Gauge widget)
   - `gauge_slots` ve `row_idx` hesaplamasını tamamen kaldır

```rust
// Yaklaşım şemasası:
let ram_text_height = 4; // Used/Buff/Cached/Avail satırları
let swap_text_height = if swap_total > 0 { 2 } else { 1 }; // SWAP bilgisi

let mut constraints = vec![];
constraints.push(Constraint::Length(ram_text_height)); // RAM text
constraints.push(Constraint::Length(1));                // RAM gauge
if swap_total > 0 {
    constraints.push(Constraint::Length(swap_text_height)); // SWAP text
    constraints.push(Constraint::Length(1));                // SWAP gauge
}

let sections = Layout::default()
    .direction(Direction::Vertical)
    .constraints(&constraints)
    .split(inner);

let mut idx = 0;
// Render RAM text
f.render_widget(Paragraph::new(ram_lines), sections[idx]);
idx += 1;
// Render RAM gauge
f.render_widget(ram_gauge, sections[idx]);
idx += 1;
// Render SWAP text + gauge (if applicable)
// ...
```

2. **GPU card'ı refactor et (`render_gpu_card`):**
   - VRAM text ve VRAM gauge için aynı Layout::split() yaklaşımını uygula

3. **Battery panel'i refactor et (`render_battery`):**
   - Battery text ve battery gauge için aynı yaklaşımı uygula

**Doğrulama:**
- `cargo build` başarılı
- Hardware → Memory panel'de RAM ve Swap gauge'ları doğru pozisyonda
- GPU tab'da VRAM gauge'ı doğru pozisyonda
- System tab'da Battery gauge'ı doğru pozisyonda
- **Kritik test:** Memory panel'e yeni bir text satırı ekle → gauge'lar kaymamalı (önceki yapıda kayardı)
- `row_idx` bazlı hesaplama kodu tamamen kaldırılmış olmalı

**Tahmini süre:** 3-4 saat (3 dosya, her biri farklı gauge yapısına sahip)

---

### Görev 1.5 — Tree/Flat View Seçim Tutarlılığı

**Sorun:** `processes.rs`'te tree view ve flat view arasında `proc_table_state.selected()` indeksi paylaşılıyor. Farklı sıralama ve öğe sayıları olduğundan seçim tutarsız.

**Değişecek Dosyalar:**
- `src/app/mod.rs` — Ayrı tree selection state ekle
- `src/app/state.rs` — Opsiyonel: `TreeViewState` struct
- `src/ui/tabs/processes.rs` — Tree view'da ayrı state kullan

**Adımlar:**

1. **App'e tree view selection state ekle:**
```rust
// mod.rs — App struct'ına
tree_table_state: TableState,
```
   - Başlangıçta `tree_table_state = TableState::default()` ile `selected = Some(0)`

2. **Tree view toggle sırasında seçimi sıfırla:**
```rust
pub fn toggle_tree_view(&mut self) {
    self.tree_view = !self.tree_view;
    // Yeni moda geçince seçimi başa al
    if self.tree_view {
        self.tree_table_state.select(Some(0));
    } else {
        self.proc_table_state.select(Some(0));
    }
}
```

3. **Navigate fonksiyonlarını güncelle:**
   - `navigate_up`, `navigate_down` gibi fonksiyonlarda `if self.tree_view` kontrolüyle doğru state'i kullan

4. **Processes render'da doğru state'i kullan:**
   - `render_tree_view` → `app.tree_table_state`
   - `render_process_table` → `app.proc_table_state`

**Doğrulama:**
- `cargo build` başarılı
- Processes sekmesinde bir süreç seç → Tree view'a geç → seçim sıfırlanmış olmalı (index 0)
- Tree view'da farklı bir süreç seç → Flat view'a geç → seçim sıfırlanmış olmalı
- Her iki modda da scroll çalışmalı

**Tahmini süre:** 1.5 saat

---

## Faz 2: Kullanıcı Deneyimi İyileştirmeleri (P1)

### Görev 2.1 — Filtre Modunda Ctrl+W / Ctrl+U Desteği

**Sorun:** Filter mode'da sadece Backspace ve karakter girişi var. Kelime/satır silme kısayolları yok.

**Değişecek Dosyalar:**
- `src/app/events.rs` — `handle_filter_key` fonksiyonu
- `src/app/mod.rs` — `filter_backspace`'i genişlet veya yeni fonksiyonlar

**Adımlar:**

1. **`handle_filter_key`'de modifier kontrolü ekle:**
```rust
fn handle_filter_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match (code, mods) {
        // Mevcut Escape/Enter
        (KeyCode::Esc, _) | (KeyCode::Enter, _) => { ... }
        
        // Ctrl+W: son kelimeyi sil
        (KeyCode::Char('w'), KeyModifiers::CONTROL) |
        (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            app.filter_delete_word(); // Processes için
            // veya Logs tab için: app.log_filter_delete_word()
        }
        
        // Ctrl+U: tüm satırı sil
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            app.filter_clear_line();
        }
        
        // Normal Backspace (modifier yok)
        (KeyCode::Backspace, _) => { ... }
        
        // Normal karakter (modifier yok)
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => { ... }
        
        _ => {}
    }
}
```

2. **App'e yeni fonksiyonlar ekle:**
```rust
pub fn filter_delete_word(&mut self) {
    // Input'un sonuna kadar sondaki boşlukları sil, sonra son kelimeyi sil
    let input = &mut self.filter_input;
    while input.ends_with(' ') { input.pop(); }
    if let Some(pos) = input.rfind(' ') {
        input.truncate(pos);
    } else {
        input.clear();
    }
}

pub fn filter_clear_line(&mut self) {
    self.filter_input.clear();
}
```

3. **Aynı fonksiyonlar log filtresi için de ekle** (`log_filter_delete_word`, `log_filter_clear_line`)

**Doğrulama:**
- `cargo build` başarılı
- Filtre modunda "firefox developer" yaz → Ctrl+W → "firefox" kalmalı
- Ctrl+W tekrar → "" boş kalmalı
- "test" yaz → Ctrl+U → "" boş kalmalı
- Normal Backspace hâlâ çalışmalı
- Log sekmesinde de aynı kısayollar çalışmalı

**Tahmini süre:** 1 saat

---

### Görev 2.2 — Graceful Degradation (Panel Küçülme)

**Sorun:** Dar terminallerde paneller tamamen kullanılamaz hale geliyor. `inner.width < 10` kontrolü yok.

**Değişecek Dosyalar:**
- `src/ui/helpers.rs` — `panel_block_focused` ve/veya yeni `collapsed_panel` fonksiyonu
- `src/ui/tabs/dashboard.rs` — Her panel'e dar ekran kontrolü ekle
- `src/ui/tabs/hardware.rs` — Aynı

**Adımlar:**

1. **Yardımcı fonksiyon oluştur:**
```rust
/// Minimum panel genişliği. Altındaki paneller daraltılmış (collapsed) gösterilir.
pub const MIN_PANEL_WIDTH: u16 = 16;
pub const MIN_PANEL_HEIGHT: u16 = 4;

/// Render a collapsed panel (title only, no content).
pub fn render_collapsed_panel(f: &mut Frame, area: Rect, title: &str, icon: &str) {
    if area.width < MIN_PANEL_WIDTH || area.height < MIN_PANEL_HEIGHT {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(surface1()))
            .title(format!(" {} ", title));
        f.render_widget(block, area);
        return;
    }
    // Normal render devam eder...
}
```

2. **Dashboard'daki her panel render fonksiyonuna dar kontrol ekle:**
   - Panel render fonksiyonunun başında: eğer area çok küçükse, sadece başlık göster ve early return

3. **Hardware tab'daki panellere aynı kontrolü ekle**

**Doğrulama:**
- `cargo build` başarılı
- Terminali yavaş yavaş daralt → paneller sırayla collapsed moduna girmeli
- Minimum terminal boyutunda en azından panel başlıkları görünmeli
- Normal genişlikte hiçbir değişiklik olmamalı

**Tahmini süre:** 1.5 saat

---

### Görev 2.3 — Dinamik Truncation

**Sorun:** Süreç isimleri gibi metinler sabit karakter sayısında kesiliyor (`proc_entry.name[..11]`, Dashboard'da 14 karakter).

**Değişecek Dosyalar:**
- `src/ui/helpers.rs` — `truncate_str` fonksiyonunu genişlet
- `src/ui/tabs/processes.rs` — Süreç ismi kesimini dinamik yap
- `src/ui/tabs/dashboard.rs` — Aynı

**Adımlar:**

1. **`truncate_str` fonksiyonunu güncelle:**
```rust
/// Truncate a string to fit within `max_width` characters, appending "…" if truncated.
pub fn truncate_str_dynamic(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        s.to_string()
    } else if max_width > 1 {
        let truncated: String = s.chars().take(max_width - 1).collect();
        format!("{}…", truncated)
    } else {
        "…".to_string()
    }
}
```

2. **Processes tab'da süreç ismi kolonunu dinamik yap:**
   - Mevcut: `Constraint::Percentage(30)` sabit
   - Hücre içeriğinde: `truncate_str_dynamic(&proc.name, name_col_width)` kullan
   - `name_col_width`'i `inner.width` ve diğer kolon genişliklerinden hesapla

3. **Dashboard'da sabit 14 karakter sınırını dinamik yap:**
   - `&proc.name[..14]` → `truncate_str_dynamic(&proc.name, available_width)`

**Doğrulama:**
- `cargo build` başarılı
- Geniş terminalde uzun süreç isimleri tam görünmeli
- Dar terminalde isimler dinamik olarak kesilmeli ve "…" ile bitmeli
- ASCII olmayan karakterler (UTF-8) düzgün kesilmeli

**Tahmini süre:** 1 saat

---

## Görev Özet Tablosu

| Görev | Faz | Dosyalar | Tahmini Süre | Bağımlılık |
|-------|-----|----------|-------------|------------|
| 1.1 Public IP Cache Doğrulama | P0 | hardware.rs | 30 dk | Yok |
| 1.2 Fare ile Sekme Tıklama | P0 | state.rs, mod.rs, header.rs, events.rs | 2-3 saat | Yok |
| 1.3 Panel Odak Sıfırlama | P0 | mod.rs | 30 dk | Yok |
| 1.4 Gauge Layout Refactor | P0 | hardware.rs, gpu.rs, system.rs | 3-4 saat | Yok |
| 1.5 Tree/Flat Seçim Tutarlılığı | P0 | mod.rs, state.rs, processes.rs | 1.5 saat | Yok |
| 2.1 Ctrl+W / Ctrl+U Desteği | P1 | events.rs, mod.rs | 1 saat | Yok |
| 2.2 Graceful Degradation | P1 | helpers.rs, dashboard.rs, hardware.rs | 1.5 saat | Yok |
| 2.3 Dinamik Truncation | P1 | helpers.rs, processes.rs, dashboard.rs | 1 saat | Yok |
| **Toplam** | | | **~12-14 saat** | |

## Çalışma Sırası

Paralel bağımlılık olmadığı için optimum sıralama:

```
1.3 → 1.1 → 1.5 → 1.4 → 1.2 → 2.1 → 2.3 → 2.2
```

**Gerekçe:**
- 1.3 (odak sıfırlama) en basit ve en yüksek anlık etki — ısınma görevi
- 1.1 (public IP) hızlı doğrulama — belki hiç kod yazılmayacak
- 1.5 (tree/flat) kendi başına test edilebilir
- 1.4 (gauge refactor) 3 dosyayı etkiliyor, 1.5'ten sonra temiz state ile yapılabilir
- 1.2 (mouse fix) en karmaşık, state/header/events koordinasyonu gerekiyor — son P0
- 2.x P1 görevleri P0'dan sonra sırayla

## Test Stratejisi

Her görev için:
1. `cargo build` — derleme hatası yok
2. `cargo test` — mevcut testler geçiyor
3. `cargo clippy` — uyarı yok
4. Manuel görsel test — TUI açılıp ilgili özellik doğrulanıyor
