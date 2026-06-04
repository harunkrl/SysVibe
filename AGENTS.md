# SysVibe — Proje Kuralları

## Proje Hakkında
SysVibe, Rust ile yazılmış terminal tabanlı sistem izleyicidir (TUI).
ratatui + crossterm + sysinfo kullanır.

## Mimari
- `src/app/` — Uygulama durumu, event handling, collectors
- `src/ui/` — TUI rendering (tab'lar, bileşenler)
- `src/config.rs` — Konfigürasyon yönetimi
- `src/theme.rs` — Tema sistemi (built-in + custom)
- `src/main.rs` — Entry point

## Kod Kuralları
- `cargo fmt` her zaman çalıştır
- `cargo clippy -- -D warnings` hatasız olmalı
- Yeni özellikler için test yaz
- Commit mesajları İngilizce, conventional commits formatında
- Theme dosyaları `src/themes/` altında `.ron` formatında

## Mevcut Sekmeler
1. **System** — CPU, RAM, Swap, Load, Uptime
2. **Hardware** — CPU detayları, Disk, Network arayüzleri
3. **Processes** — Süreç listesi, sıralama, filtreleme
4. **Logs** — systemd/journal log okuyucu

## Build & Test
```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
```
