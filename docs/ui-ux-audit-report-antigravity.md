# SysVibe: UI/UX Analizi ve İyileştirme Raporu

Bu rapor, SysVibe (Ratatui tabanlı Linux sistem monitörü TUI uygulaması) projesinin kullanıcı arayüzü (UI) ve kullanıcı deneyimi (UX) açısından incelenmesi sonucunda tespit edilen temel problemleri ve potansiyel iyileştirmeleri içermektedir.

## 🎨 1. Görsel Tasarım ve Arayüz (UI)

### ✅ Mevcut Güçlü Yönler
- **Renk Temaları:** Catppuccin, Nord, Dracula, Tokyo Night gibi popüler ve modern temaların sisteme dahil edilmesi görsel bütünlüğü çok güçlendirmiş.
- **Grafikler (Sparkline/Braille):** İşlemci ve Ağ panellerinde `Braille` karakterleri kullanılarak çizilen grafikler yoğun veri setini temiz bir şekilde sunuyor.
- **İkon Desteği:** Nerd Font destekli ikonlar (örn. sıcaklık, ağ yönleri, RAM ikonları) metin tabanlı bir arayüz için çok modern bir his yaratıyor.

### ❌ Problemler ve İyileştirme Fırsatları
- **Sabit Karakter Kesmeleri (Hardcoded Truncation):** 
  `dashboard.rs` içerisindeki işlemci isimleri her zaman `14` karakterle sınırlandırılmış (`proc_entry.name[..11] + "..."`). Ancak terminal tam ekranda olduğunda bu sütunda çok daha fazla alan olabilir.
  - **💡 İyileştirme:** Karakter sınırlandırması `inner.width` değerine göre dinamik hesaplanmalıdır.
- **Panel Görünmezliği Sorunu:** 
  Küçük terminallerde paneller sığmadığında `if inner.width < 10 { return; }` denilerek panel içeriği aniden tamamen kayboluyor. Bu durum kullanıcıyı şaşırtabilir.
  - **💡 İyileştirme:** Minimum alan uyarıları ("Terminal çok dar") veya panelin başlığını gösterip sadece içeriğini gizleme gibi zarif düşme (graceful degradation) yöntemleri tercih edilmelidir.
- **Dar Alanda Yan Yana Sütunlar:** 
  Dashboard ortasında Memory, Processes ve Network aynı anda `%33` genişliklerle bulunuyor. Terminal eni ortalama altındaysa (örn. 80 kolon), her panel ~26 karaktere sıkışıyor ve okunaklılık kayboluyor.

---

## 🧭 2. Etkileşim ve Navigasyon (UX)

### ✅ Mevcut Güçlü Yönler
- **Vim Stili Navigasyon:** `j/k`, `Esc`, `/` (filtreleme) gibi klavye kısayollarının yerleşik olması Power User'lar için çok avantajlı.
- **Kolay İşlem Seçimi:** `Space` ile çoklu işlem (process) seçilip toplu öldürme vb. eylemler yapılabiliyor.

### ❌ Problemler ve İyileştirme Fırsatları
- **Kırılgan Fare Tıklama Mantığı:**
  `events.rs` içerisindeki sekme tıklama tespiti çok tehlikeli bir şekilde sabit hesaplamalara dayanıyor:
  ```rust
  let total_tab_width = 90;
  let terminal_width: usize = 120; // Varsayım!
  ```
  Eğer kullanıcının terminali 120 karakter değilse (ki genellikle değişir), sekmelere tıklamak yanlış sekmeyi açacak veya hiç çalışmayacaktır.
  - **💡 İyileştirme:** `ratatui`'nin layout sisteminde sekmelerin oluşturulduğu an koordinatları (Rect) saklanmalı ve `MouseEvent` koordinatları doğrudan bu saklanan bölgelerle eşleştirilmelidir.
- **Tek Yönlü Sıralama (Sorting) Döngüsü:**
  `s` tuşuna basıldığında sıralama `Cpu -> Mem -> Pid -> Name -> Cpu` şeklinde tek yönlü ilerliyor. Kullanıcı CPU'dan Pid'e geçmek için fazladan tuşlamak zorunda kalıyor veya tersine dönemiyor.
  - **💡 İyileştirme:** `Shift + S` ile geriye doğru döngü sağlanabilir veya `Sort By` menüsü için bir açılır pencere (modal) oluşturularak tek tuşla seçim (örn. `c` for CPU, `m` for Mem) sunulabilir.
- **Kısayol Keşfedilebilirliği (Discoverability):**
  Normal modda `[`, `]`, `t` (Sıcaklık birimi), `g` (Normalize CPU), `7` (GPU tab'ı) gibi çok fazla kısayol var ancak arayüzde (özellikle Footer kısmında) bunları hatırlatacak yeterli ipucu yer almıyor olabilir.
  - **💡 İyileştirme:** Footer/Header alanında en kritik kısayollar dönemsel olarak gösterilebilir veya ekrana minik bir "Help [?]" butonu eklenebilir.

---

## ⚙️ 3. Modlar ve Akış (Flow)

### ❌ Problemler ve İyileştirme Fırsatları
- **Kill Confirmation Modu:** 
  İşlemi sonlandırırken `y` (confirm) ve `k` (confirm) ikisi de aynı işlevi yapıyor (muhtemelen kill kelimesinden dolayı `k` eklendi), ancak yanlışlıkla basılmaya çok müsait olabilir.
  - **💡 İyileştirme:** Tehlikeli işlemler (Kill) için onaylarken `Enter` veya spesifik olarak yazarak ("y" yerine "yes") onay almak yanlışlıkları önler.
- **Filtreleme Modu Silme Deneyimi:**
  Kullanıcı `/` ile filtreye girip bir şeyler yazıyor. Ancak tüm filtreyi hızlıca temizlemek için (örneğin Ctrl+U) bir kısayol mevcut değil. Sadece `Backspace` tek tek siliyor.
  - **💡 İyileştirme:** Filtre modunda `Esc` filtreyi iptal ederken, `Ctrl+W` (kelime sil) ve `Ctrl+U` (satır sil) gibi standart terminal metin kısayolları eklenmelidir.

## 🎯 Sonuç
**SysVibe**, özellikle görsellik (UI) açısından oldukça iyi tasarlanmış ve renk paletleriyle Premium bir his veriyor. Ancak arka planda fare (mouse) entegrasyonu, ekran yeniden boyutlandırma davranışları (responsive design) ve bazı klavye kısayolu iş akışları (UX) açısından kod seviyesinde esnek olmayan pratiklere (hardcoded) sahip. Bu durumların dinamik hesaplamalara dönüştürülmesi projenin kalitesini çok artıracaktır.
