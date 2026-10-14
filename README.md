# ⬡ AltunHub

Altun uygulamalarını (AltunDP, AltunCleaner, AltunCore), oyunları ve programları tek yerden açan,
karanlık temalı Windows launcher'ı. Tauri 2 (Rust + web arayüz) ile yapıldı.

## Setup (kurulum dosyası) nasıl çıkar?
1. Bu klasörü GitHub'a yükle (`main` dalına).
2. **Actions** sekmesinde "Build AltunHub Setup" otomatik çalışır (~5-10 dk).
3. Bittiğinde **Artifacts → AltunHub-Setup** içinden `AltunHub_1.0.0_x64-setup.exe` dosyasını indir.
4. Sürüm yayınlamak için: `git tag v1.0.0 && git push --tags` → Releases sayfasına otomatik eklenir.

## Kendi bilgisayarında derleme
Gerekenler: Node.js 20+, Rust (rustup), Windows'ta WebView2 (Win10/11'de hazır).
```
npm install
npx tauri icon app-icon.png   # kendi logonla değiştir (1024x1024 PNG)
npm run dev                   # geliştirme
npm run build                 # setup .exe -> src-tauri/target/release/bundle/nsis/
```

## Özelleştirme
- Uygulama listesi, ikonlar ve sosyal linkler: `src/index.html` (`DEF` listesi).
- Logo: `app-icon.png` dosyasını kendi logonla değiştir.

## Lisans
MIT
