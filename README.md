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

## Otomatik güncelleme nasıl çalışır?
- Hub açılınca her uygulamanın GitHub'daki **son release**'ine bakar (AltunDP, AltunSpace, AltunCPU ve AltunHub'ın kendisi).
- Yeni sürüm varsa kartta **Güncelle** çıkar. Butona basınca release'teki `.exe` / `.msi` / `.zip` indirilir.
  Kurulum dosyası (adında setup/install geçen) çalıştırılır, taşınabilir exe ise `%LOCALAPPDATA%\AltunHub\apps\` altına konur.
- Sen sadece GitHub'da yeni tag'li bir release çıkar ve `.exe` dosyasını Assets'e yükle, hub'a dokunmana gerek yok.
- Release'e dosya yüklemezsen (sadece otomatik kaynak zip'leri olursa) buton sürüm sayfasını açar.

## Liste internetten nasıl güncellenir? (apps.json)
1. `apps.json` dosyası repoda ana klasörde durur. Yeni sürüm, yeni indirme linki veya yeni uygulama için bu dosyayı düzenle.
2. `src/index.html` içindeki `const REMOTE=""` satırına dosyanın raw adresini yaz:
   `https://raw.githubusercontent.com/Altun667/REPO_ADI/main/apps.json`
3. Bundan sonra kullanıcılar yeni setup indirmeden listeyi otomatik alır.

## Özelleştirme
- Uygulama listesi, ikonlar ve sosyal linkler: `src/index.html` (`BUILTIN` listesi).
- Logo: `app-icon.png` dosyasını kendi logonla değiştir.

## Lisans
MIT
