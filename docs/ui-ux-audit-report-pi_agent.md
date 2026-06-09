# SysVibe UI/UX Denetim Raporu

**Proje:** SysVibe v0.4.0 — Linux Sistem Monitörü TUI  
**Tarih:** 2026-06-09  
**Kapsam:** Tüm UI modülleri (`src/ui/`), olay yönetimi, durum yönetimi, widget motoru

---

## 1. Yönetici Özeti

SysVibe, Catppuccin Macchiato temeli, Braille grafik motoru ve çok-sekmeli yapısıyla görsel olarak etkileyici bir TUI. Bununla birlikte, derinlemesine incelemede **kritik kullanılabilirlik sorunları**, **tutarlılık eksiklikleri**, **erişilebilirlik problemleri** ve **UX iyileştirme fırsatları** tespit edildi. Bu rapor, tespit edilen her konuyu açıklar, etkisini değerlendirir ve önerilen çözümü sunar.

### Tespit Özeti

| Şiddet | Sayı | Kategori |
|---------|------|----------|
| 🔴 Kritik | 5 | Kullanılabilirlik, Tutarlılık |
| 🟡 Orta | 8 | UX, Bilgi Mimarisi, Etkileşim |
| 🟢 Düşük | 7 | Görsel İyileştirme, Erişilebilirlik |
| **Toplam** | **20** | |

---

## 2. Kritik Sorunlar (🔴)

### 2.1 Fare ile Sekme Geçişi Kırık

**Dosya:** `src/app/events.rs:224-248`

Mouse click handler, sabit 120 genişlik ve 90 sekme genişliği varsayarak çalışıyor. Gerçek terminal genişliği ve sekme pozisyonları hesaba katılmıyor.

```rust
// Mevcut (bozuk) kod:
let total_tab_width = 90;
let terminal_width: usize = 120; // sabit!
let start = terminal_width.saturating_sub(total_tab_width) / 2;
```

**Etki:** Dar veya geniş terminallerde fare ile sekme tıklama tamamen yanlış konumlara denk geliyor. Kullanıcı farkında olmadan yanlış sekmeye geçebilir.

**Öneri:** Header render sırasında her sekmenin x-koordinat aralığını kaydet, tıklama olayında gerçek koordinatları kullan.

---

### 2.2 Ağ Panelinde Public IP Her Tick'te Kontrol Ediliyor (UX Dondurma Riski)

**Dosya:** `src/ui/tabs/hardware.rs:280-290`

Network panelinde `app.public_ip()` her render frame'inde çağrılıyor. Bu değer bir DNS/HTTP sorgusu gerektiriyorsa, TUI donma veya lag hissedilebilir.

**Etki:** Public IP çözümlenmesi 100ms-2s sürebilir. Render döngüsü içinde bloklama I/O yapılması, düşük refresh rate hissi yaratır.

**Öneri:** Public IP'yi arka planda tek seferde çözümle, sonucu cache'le. UI'da cache'lenmiş değeri göster.

---

### 2.3 Sekmeler Arası Panel Odak Tutarsızlığı

**Dosya:** `src/app/state.rs:PanelFocus`

`PanelFocus` enum'u sekmeler arası geçişte sıfırlanmıyor. Her sekme farklı sayıda panel kullanıyor (Dashboard: 6, Logs: 1, Processes: 1), ama PanelFocus global olarak kalıyor.

**Etki:** Logs sekmesinde Panel1 odaklıyken Hardware sekmesine geçince Panel1 odaklı kalıyor — ancak Hardware'deki Panel1 CPU paneli. Kullanıcı odak döngüsünün `[` `]` tuşlarıyla tutarsız davrandığını hisseder.

**Öneri:** Her sekme için ayrı panel odak durumu tut, veya sekme geçişinde Panel1'e sıfırla.

---

### 2.4 Gauge Overlay Pozisyonlama Kırılgan Desen

**Dosyalar:** `hardware.rs:render_memory_panel`, `gpu.rs:render_gpu_card`, `system.rs:render_battery`

Gauge widget'ları, satır indeksleri elle hesaplanarak overlay ediliyor. Bu yaklaşım kırılgandır:

```rust
// Bellek panelinde gauge overlay:
for (row_idx, ratio, color, label) in gauge_slots {
    let y = inner.y + row_idx as u16;
    // ...
}
```

**Etki:** Herhangi bir satır eklendiğinde veya layout değiştiğinde gauge'lar yanlış pozisyona kayar. Bakımı zor, hata yapmaya açık.

**Öneri:** Gauge render için ayrı bir layout constraint sistemi kullan. Overlay yerine doğrudan constraint bazlı layout ile Gauge'ı render et.

---

### 2.5 Tree View'da Seçim Tutarsızlığı

**Dosya:** `processes.rs:render_tree_view`

Tree view, `proc_table_state.selected()` indeksini düz liste indeksi olarak kullanıyor ama tree view farklı bir sıralama ve sayıda öğe içeriyor. Seçim indeksleri flat view ve tree view arasında paylaşılmıyor.

**Etki:** Tree view'a geçerken seçili öğe tamamen farklı bir sürece karşılık gelebilir. Tree'den flat'e dönerken de aynı sorun yaşanır.

**Öneri:** Tree view için ayrı bir seçim durumu tut, veya geçişte seçimi sıfırla.

---

## 3. Orta Şiddetli Sorunlar (🟡)

### 3.1 Header'da 6 Sekme Sıkışması

**Dosya:** `src/ui/header.rs`

6 sekme (Dashboard, System, Hardware, Processes, Logs, GPU) ikon + metin ile gösteriliyor. Minimum terminal genişliği 60 olarak ayarlanmış, ama 6 sekmeli header 60 karakterde sığmıyor.

**Etki:** Dar terminallerde sekmeler taşabilir, metin kırpılabilir veya okunamaz hale gelebilir.

**Öneri:** Terminal genişliğine göre adaptif header: dar terminallerde sadece ikonlar, geniş terminellerde ikon + metin. Scrollable veya iki satırlı header alternatifi değerlendir.

---

### 3.2 Footer Bilgi Yoğunluğu

**Dosya:** `src/ui/footer.rs`

Processes sekmesinde footer şu bilgileri sıkıştırıyor: `[h] Help · [/] Filter · [s] Sort · [p] Tree · [g] Norm · [x] Kill · [q] Quit · 🔍 filter_text · 342 procs · SysVibe v0.4.0`

**Etki:** Tek satırda çok fazla bilgi. Önemli shortcut'lar kayboluyor. Filtre aktifken metin satır dışına taşabilir.

**Öneri:** Sadece en önemli 3-4 kısayolu göster, geri kalanı için help referansı. Process sayısı ve versiyon bilgisini header'a taşı.

---

### 3.3 Help Modalında Scroll Desteği Yok

**Dosya:** `src/ui/widgets/modal.rs`

Help modalı tüm kısayolları tek bir listede gösteriyor (24 kısayol). Modal küçük terminallerde sığmıyor ve scrolable değil.

**Etki:** Dar terminallerde bazı kısayollar görüntülenemiyor.

**Öneri:** Help modalına scroll desteği ekle (yukarı/aşağı tuşları ile). Alternatif olarak kısayolları kategorilere ayır.

---

### 3.4 Dashboard ile Diğer Sekmeler Arasında Bilgi Tekrarı

**Dosyalar:** `dashboard.rs`, `system.rs`, `hardware.rs`

Dashboard sekmesi CPU, RAM, Network, GPU, Disk I/O ve sistem bilgilerini gösteriyor — bunların tümü zaten ayrı sekmelerde mevcut.

**Etki:** Kullanıcıya değer katmayan bir tekrar. Dashboard'in amacı "hızlı genel bakış" olmalı, ama mevcut durumda diğer sekmelerin kısıtlı versiyonları.

**Öneri:** Dashboard'i gerçek bir özet paneli olarak yeniden tasarla: Sadece kritik uyarılar, tek satırlık CPU/RAM/Disk/Network özeti, ve alarm durumları. Detay için ilgili sekmeye yönlendir.

---

### 3.5 Süreç Tablosunda Kolon Genişlikleri Sabit

**Dosya:** `processes.rs:render_process_table`

Kolon genişlikleri `Constraint::Length(8)`, `Constraint::Percentage(30)`, `Constraint::Length(10)`, `Constraint::Length(10)` olarak sabit.

**Etki:** Dar terminallerde isim kolonu çok dar kalıyor. Geniş terminallerde isim kolonu gereksiz boşluk kaplıyor. Kullanıcı kolonları yeniden boyutlandıramaz.

**Öneri:** Terminal genişliğine göre orantılı kolon genişlikleri. İsteğe bağlı kolon göster/gizle desteği.

---

### 3.6 Log Seviye Filtre Barı Dar Terminallerde Taşıyor

**Dosya:** `src/ui/tabs/logs.rs:render_level_filter_bar`

Seviye filtreleme barı `[ERR] [WRN] [INF] [NTC] [DBG] Toggle: e=ERR w=WRN i=INF` gösteriyor. Bu dar terminallerde taşıyor.

**Etki:** Dar terminallerde toggle ipucu metni kırılıyor.

**Öneri:** Toggle ipucunu tooltip olarak göster, veya yalnızca ilk kullanımda gösterip sonra gizle.

---

### 3.7 Export ve Gizli Kısayollar Keşfedilemez

**Dosya:** `src/app/events.rs`

`E` (export) ve `7` (GPU sekmesi) gibi kısayollar footer'da gösterilmiyor ve help modalında da eksik.

**Etki:** Kullanıcı bu özelliklerin varlığını bilmiyor. Export özelliği "gizli" kalıyor.

**Öneri:** Help modalını tüm kısayolları içerecek şekilde güncelle. Footer'da en azından export kısayolunu göster.

---

### 3.8 `draw(&mut App)` İmzası Gereksiz Mutable

**Dosya:** `src/ui/mod.rs`

`draw` fonksiyonu `&mut App` alıyor ama çoğu render fonksiyonu `&App` ile çalışabiliyor. Sadece Processes tab'ı mutable state gerektiriyor (table state).

**Etki:** Gereksiz mutable borrow, paralel render veya test yazımını zorlaştırıyor.

**Öneri:** Sadece Processes tab için scoped mutable borrow kullan. Ana `draw` imzasını `&App` yap.

---

## 4. Düşük Şiddetli Sorunlar ve İyileştirmeler (🟢)

### 4.1 Minimum Terminal Boyutu Mesajı Yetersiz

**Dosya:** `src/ui/mod.rs:28-36`

Terminal çok küçükse sadece hata mesajı gösteriliyor. Kullanıcıya ne yapması gerektiği söylenmiyor.

**Öneri:** "Terminal en az 80x24 boyutunda olmalıdır. Lütfen pencereyi büyütün." gibi yönlendirici mesaj göster.

---

### 4.2 Braille Grafiklerde Eksik Y-Ekseni Etiketleri

**Dosyalar:** `sparkline.rs`

`braille_mirrored_graph` ve `braille_mini` fonksiyonlarında Y-ekseni etiketleri yok. `braille_line_graph` ve `halfblock_graph`'ta var ama tutarsız.

**Etki:** Kullanıcı grafikteki değerlerin ölçeğini anlayamıyor.

**Öneri:** Tüm grafik türlerinde minimum/maximum değer etiketi göster.

---

### 4.3 Nerd Font Fallback İkonları Tutarsız

**Dosya:** `src/ui/icons.rs:fallback`

ASCII fallback ikonlar tutarsız: bazıları `[Sys]`, bazıları `⬡`, bazıları `◈` karışık. Tutarlı bir alternatif set değil.

**Öneri:** Fallback seti tutarlı ASCII/Unicode ikonlarla yeniden düzenle. Ya hep bracket (`[Sys]`, `[Hw]`) ya hep Unicode sembol.

---

### 4.4 Renk Paletinde Tekrar Eden Renk Atamaları

**Dosya:** `src/ui/theme.rs`

Dracula temasında: `rosewater = pink = flamingo = #FF79C6`. Nord temasında: `teal = sky = sapphire`. Bu, bazı UI elemanlarının ayırt edilememesine neden oluyor.

**Etki:** Tema değiştiğinde bazı metinler/ikonlar birbirine karışıyor.

**Öneri:** Her tema için benzersiz tonlar kullan, veya en azından semantic token'ları (focus, accent, warning) ayırt edilebilir yap.

---

### 4.5 Tooltip / Onay Sistemi Yok

Kullanıcı bir tuşa bastığında ne olduğunu anlayan bir inline bildirim sistemi var (`StatusMessage`) ama bu yalnızca bazı eylemlerden sonra tetikleniyor.

**Öneri:** İlk kullanımda keşfedici tooltip'ler göster. "İpucu: Tab tuşu ile sekmeler arası geçiş yapabilirsiniz" gibi.

---

### 4.6 Theme Seçimi Config'de Yok

**Dosya:** `src/ui/palette.rs`

Temalar `thread_local!` ile tutuluyor ve `apply_theme` ile değiştirilebiliyor. Ama config.rs'de theme seçeneği yok ve runtime'da tema değiştirme kısayolu da yok.

**Etki:** Tema sistemi var ama kullanıcı erişimi yok.

**Öneri:** Config'e `theme = "catppuccin-macchiato"` seçeneği ekle. Runtime'da tema değiştirme desteği (Ctrl+T gibi) değerlendir.

---

### 4.7 Kill Onay Modalı Tek Tuşla Onay

**Dosya:** `modal.rs:render_kill_confirm_modal`

Kill onayı `[Y]` tek tuşla yapılıyor. Kullanıcı yanlışlıkla `y` tuşuna basabilir.

**Öneri:** Daha güvenli onay mekanizması: "Kill için 'yes' yazın" veya en azından çift basış (double-press) gereksinimi.

---

## 5. Mimari ve Kod Kalitesi Gözlemleri

### 5.1 UI Modül Yapısı İyi Organize

`ui/tabs/`, `ui/widgets/`, `ui/helpers.rs` ayrımı temiz. Her tab kendi modülünde. Widget'lar (sparkline, modal) ayrı tutulmuş.

### 5.2 helpers.rs Çok Yüklü

`helpers.rs` hem layout helper, hem renk fonksiyonları, hem text formatting, hem block constructor içeriyor. Bu 4 sorumluluğu ayırmak okunabilirliği artırır:
- `blocks.rs` — Block constructor'lar
- `colors.rs` — Renk fonksiyonları (`usage_color`, `temp_color`, vb.)
- `format.rs` — Text formatting (`format_speed`, `format_bytes`, `truncate_str`, vb.)
- `layout.rs` — Layout helper'lar (`centered_rect`)

### 5.3 sparkline.rs İyi Optimize

`BRAILLE_CHARS` lazy lookup table, `resample` fonksiyonu ve üç farklı grafik türü (line, mirrored, halfblock) iyi implemente edilmiş. Performans bilincinin yüksek olduğu bariz.

### 5.4 Thread-Local Tema Yaklaşımı Uygun ama Sınırlı

Runtime tema değişimi için thread_local iyi çalışıyor ama multi-thread render senaryolarında sorun yaratabilir. Bununla birlikte TUI uygulamalarında genelde tek thread olduğu için pratikte sorun değil.

---

## 6. Önceliklendirilmiş İyileştirme Yol Haritası

| Öncelik | Konu | Tahmini Efor |
|---------|------|-------------|
| P0 | Fare ile sekme tıklama düzeltme (#2.1) | 2-3 saat |
| P0 | Gauge overlay pozisyonlama refactor (#2.4) | 4-6 saat |
| P0 | Sekmeler arası odak sıfırlama (#2.3) | 1 saat |
| P1 | Tree/Flat view seçim tutarlılığı (#2.5) | 2 saat |
| P1 | Public IP cache (#2.2) | 1 saat |
| P1 | Help modal scroll desteği (#3.3) | 2 saat |
| P1 | Eksik kısayolları help'e ekle (#3.7) | 30 dk |
| P2 | Header adaptif layout (#3.1) | 3 saat |
| P2 | Footer basitleştirme (#3.2) | 2 saat |
| P2 | Dashboard yeniden tasarım (#3.4) | 6-8 saat |
| P2 | Log filter bar responsive (#3.6) | 1 saat |
| P3 | Grafik Y-ekseni etiketleri (#4.2) | 2 saat |
| P3 | Fallback ikon tutarlılığı (#4.3) | 1 saat |
| P3 | Config'e tema desteği (#4.6) | 2 saat |
| P3 | Tooltip/onboarding sistemi (#4.5) | 4 saat |

---

## 7. Detaylı Sekme Bazlı Değerlendirme

### 7.1 Dashboard Tab

| Kriter | Puan | Not |
|--------|------|-----|
| Bilgi Yoğunluğu | ⭐⭐⭐ | İyi ama tekrarcı |
| Görsel Hiyerarşi | ⭐⭐⭐ | CPU grafiği dominant |
| Navigasyon | ⭐⭐ | Panel odak kırılgan |
| Adaptiflik | ⭐⭐ | Dar ekranlarda sorunlu |

### 7.2 System Tab

| Kriter | Puan | Not |
|--------|------|-----|
| Bilgi Yoğunluğu | ⭐⭐⭐⭐ | Çok kapsamlı |
| Görsel Hiyerarşi | ⭐⭐⭐ | KV satırları monoton |
| Navigasyon | ⭐⭐ | Scroll yok, sığmayan bilgiler kaybolur |
| Adaptiflik | ⭐⭐ | 58/42 split dar ekranlarda sorunlu |

### 7.3 Hardware Tab

| Kriter | Puan | Not |
|--------|------|-----|
| Bilgi Yoğunluğu | ⭐⭐⭐⭐ | Her panel bilgi dolu |
| Görsel Hiyerarşi | ⭐⭐⭐⭐ | Gauge + grafik iyi |
| Navigasyon | ⭐⭐⭐ | Panel odak iyi çalışıyor |
| Adaptiflik | ⭐⭐⭐ | Constraint bazlı, makul |

### 7.4 Processes Tab

| Kriter | Puan | Not |
|--------|------|-----|
| Bilgi Yoğunluğu | ⭐⭐⭐⭐ | Tablo + filtre + sort |
| Görsel Hiyerarşi | ⭐⭐⭐ | Zebra striping iyi |
| Navigasyon | ⭐⭐⭐⭐ | Virtual scroll + scrollbar |
| Adaptiflik | ⭐⭐ | Kolon genişlikleri sabit |

### 7.5 Logs Tab

| Kriter | Puan | Not |
|--------|------|-----|
| Bilgi Yoğunluğu | ⭐⭐⭐ | Filtre barları dikey alan kaplıyor |
| Görsel Hiyerarşi | ⭐⭐⭐⭐ | Log seviye badge'leri etkili |
| Navigasyon | ⭐⭐⭐ | Follow mode + scroll var |
| Adaptiflik | ⭐⭐ | İki filtre barı dar ekranlarda sorunlu |

### 7.6 GPU Tab

| Kriter | Puan | Not |
|--------|------|-----|
| Bilgi Yoğunluğu | ⭐⭐⭐ | İyi ama boş alan çok |
| Görsel Hiyerarşi | ⭐⭐⭐ | Gauge'ler etkili |
| Navigasyon | ⭐⭐⭐ | Multi-GPU scroll var |
| Adaptiflik | ⭐⭐⭐ | Tek panel, sorun yok |

---

## 8. Sonuç ve Öneriler

SysVibe'nin UI temeli sağlam: Catppuccin tema sistemi, Braille grafik motoru, ve modüler tab yapısı kaliteli. Ana sorunlar **etkileşim tutarlılığı** (fare, odak, seçim) ve **adaptiflik** (dar terminaller) alanlarında. Yukarıdaki yol haritası izlendiğinde, kullanıcı deneyimi önemli ölçüde iyileşecektir.

### En Yüksek Etkili 3 İyileştirme

1. **Fare ile sekme geçişi düzeltmesi** — Anında kullanılabilirlik artışı, en çok şikayet edilecek ilk sorun
2. **Sekme geçişinde odak sıfırlama** — Tutarlı deneyim, `[` `]` tuşlarına olan güveni artırır
3. **Help modal güncellemesi** — Feature discoverability, kullanıcıların tüm özellikleri öğrenmesini sağlar
