# configure-app (Tauri) — egui'nin yerine geçen ayarlar uygulaması

Eski `src/bin/configure.rs` (egui) kaldırıldı, referans olarak
`configure-app/reference/old-egui-configure.rs.txt` içinde duruyor.
Yerine bu Tauri uygulaması geldi.

## Neden değişti

egui immediate-mode bir framework olduğu için layout'u siz elle
hesaplıyordunuz (`app_panel` içindeki `inner_width = width - MARGIN_X*2.0`
gibi). Bir yerde unutulan/yanlış margin -> taşma/çakışma. Ayrıca tema
editöründe iki ayrı state vardı (ham YAML metni + "visual editor"
struct'ı) ve bunları elle senkron tutmanız gerekiyordu.

Bu yeni uygulamada:
- **Layout**: gerçek CSS grid/flexbox (`configure-app/src/css/style.css`).
  Genişlik/margin hesabı yok, tarayıcı motoru hallediyor.
- **Tema editörü**: TEK state (`yamlText` — bkz. `js/main.js`). Görsel
  form da, YAML metin editörü de bu tek string'i okuyup yazıyor; ikisi
  asla birbirinden bağımsız kalamıyor.
- **Rust tarafı**: `main.rs`'teki (monitor) ve eski `configure.rs`'teki
  preview/YAML mantığı artık kök projenin `src/preview.rs`'inde —
  paylaşılan `lib.rs` üzerinden. Böylece iki ayrı yerde aynı kodun iki
  kopyası olmuyor.

## Mimari

```
mini/                          (kök crate, mini_system_monitor)
├── src/
│   ├── lib.rs                 pub mod config, preview, renderer
│   ├── preview.rs             YAML->görüntü render mantığı (paylaşılan)
│   └── main.rs                monitor daemon (değişmedi)
└── configure-app/             YENİ — Tauri uygulaması
    ├── src-tauri/              Rust backend (Tauri command'ları)
    │   └── src/
    │       ├── main.rs
    │       ├── commands.rs     load_config, save_theme_yaml, render_preview, ...
    │       └── paths.rs        config.yaml / theme.yaml yolu çözümleme
    └── src/                    Frontend — DÜZ HTML/CSS/JS, bundler YOK
        ├── index.html
        ├── css/style.css
        └── js/main.js
```

Frontend'de bilerek Vite/webpack gibi bir build aracı kullanılmadı —
CodeMirror (YAML syntax highlight) ve js-yaml, `index.html` içinde CDN
üzerinden yükleniyor. `tauri.conf.json`'daki `frontendDist` doğrudan
`../src`'i gösteriyor, yani derleme adımı yok, sadece dosyaları
sunuyor. Bu, "minimalistik ayar menüsü" isteğinize uyacak şekilde
tutulan bilinçli bir tercih — daha az hareketli parça, daha az
kırılma noktası.

## Geliştirilenler (bu aşamada)

- **Genel Ayarlar** sekmesi artık solda tema önizlemesi, sağda ayarlar
  şeklinde (eski app'teki düzenle aynı fikir). COM port, tema, ağ
  arayüzleri, ping, hava durumu (şehir aramalı — aşağıda), parlaklık,
  ekran ters çevirme, başlangıçta sıfırlama. Kaydet + Monitörü Başlat.
- **Hava durumu şehir arama** geri geldi: Open-Meteo geocoding API
  üzerinden (`search_cities` komutu), sonuca tıklayınca enlem/boylam
  otomatik doluyor.
- **Tema Editörü** sekmesi:
  - Sol: X/Y içeren her öğeyi otomatik bulan görsel form.
  - Orta: syntax-highlighted YAML editörü (CodeMirror).
  - Sağ: canlı önizleme — ve önizlemenin ÜZERİNE tıklayıp
    sürükleyerek öğeleri doğrudan ekrandan seçip taşıyabiliyorsunuz
    (eski app'teki canvas seçimine benzer). Bir öğeye tıklamak sol
    taraftaki form panelini de otomatik açıp oraya kaydırıyor;
    sürüklemek X/Y'yi hem forma hem YAML'a anında yazıyor.
  - Üç panel de aynı `yamlText` state'ini paylaşıyor — hangisinden
    değiştirirseniz değiştirin diğer ikisi anında güncelleniyor.

## Sonraki adım (kapsam dışı bırakıldı)

- Yazı tipi seçici (backend'de `list_fonts` komutu hazır, frontend'e
  henüz bağlanmadı).
- Öğe ekleme/silme (yeni bir STATS öğesi eklemek şu an sadece YAML
  editöründen elle yapılabiliyor).

İsterseniz bir sonraki adımda bunlardan istediğinizi ekleyelim.

## Çalıştırma

Bu sandbox'ta Rust toolchain'i yok, yani derleme burada test
edilemedi — kod elle gözden geçirildi ama kendi makinenizde
derlerken küçük hatalar çıkabilir, olağan.

Windows'ta (proje zaten Windows'a özel — `wmi`, `serialport`,
`windows-sys` kullanıyor):

```powershell
# Tauri CLI'yı kurun (ikisinden biri yeter)
cargo install tauri-cli --version "^2"
# veya: npm install -g @tauri-apps/cli

cd configure-app
cargo tauri dev
```

`cargo tauri dev` hem backend'i derler hem pencereyi açar; frontend
için ayrı bir build adımı gerekmiyor (yukarıda açıklandığı gibi).

Kurulum paketi (.msi/.exe) üretmeden önce bir ikon eklemeniz gerekir:

```powershell
cargo tauri icon path/to/icon.png
```

## Kök projede değişenler

- `Cargo.toml`: `configure` bin hedefi ve `eframe` bağımlılığı
  kaldırıldı (artık kullanılmıyor).
- `src/lib.rs`: `pub mod preview;` eklendi.
- `src/preview.rs`: yeni — eski `configure.rs`'teki önizleme mantığı.
- `src/main.rs` (monitor daemon) hiç değişmedi.
