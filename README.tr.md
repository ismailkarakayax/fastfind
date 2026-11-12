# fastfind

[![CI](https://github.com/ismailkarakayax/fastfind/actions/workflows/ci.yml/badge.svg)](https://github.com/ismailkarakayax/fastfind/actions/workflows/ci.yml)

[English](README.md) | **Türkçe**

Rust ile yazılmış, komut satırı için çok hızlı ve paralel çalışan bir **dosya adı** arama aracı.
Bir ismin bir kısmını yazarsınız, eşleşen tüm dosya ve klasörlerin tam yolunu alırsınız.

```console
> fastfind rapor C:\Users
C:\Users\ben\Belgeler\rapor_2025.docx
C:\Users\ben\Masaüstü\eski\Rapor.pdf
2 results · 184,302 entries scanned · 0.41s
```

## Neden?

Windows Gezgini'nin araması yalnızca indekslenmiş yerlerde hızlı. İndeks dışındaki
yerlerde (diğer sürücüler, proje klasörleri, harici diskler) diski yavaşça tarıyor.
`dir /s` ve `Get-ChildItem -Recurse` ise klasör ağacını tek iş parçacığıyla geziyor.

fastfind klasör ağacını **tüm CPU çekirdekleriyle aynı anda** gezer ve dosya
içeriklerine hiç bakmaz, yalnızca isimleri kontrol eder.

## Benchmark

**710.835 girdilik** bir kullanıcı klasöründe `report` araması yapıldı. Tüm araçlarda
gizli dosyalar dahildi, önbellek ısınmıştı ve her araç 5 kez çalıştırıldı. Ölçümde
[hyperfine](https://github.com/sharkdp/hyperfine) kullanıldı:

| Komut | Ortalama süre | |
|---|---:|---|
| **`fastfind`** | **1,43 sn** ± 0,18 | |
| `dir /s /b /a` | 13,24 sn ± 0,15 | 9,3 kat yavaş |
| `Get-ChildItem -Recurse -Force` | 25,95 sn ± 0,32 | 18,2 kat yavaş |

<sub>AMD Ryzen 5 9600X (6 çekirdek / 12 iş parçacığı), Kingston 1 TB SSD, Windows 10.</sub>

Kendi bilgisayarınızda [`scripts/bench.ps1`](scripts/bench.ps1) ile tekrarlayabilirsiniz.

## Kurulum

[Rust](https://rustup.rs) gerekli.

```bash
git clone https://github.com/ismailkarakayax/fastfind
cd fastfind
cargo install --path .
```

## Kullanım

```text
fastfind <KALIP> [YOL...]
```

| Örnek | Ne yapar |
|---|---|
| `fastfind fatura` | Geçerli klasörde adında "fatura" geçenler (büyük/küçük harf fark etmez) |
| `fastfind fatura D:\ E:\` | Birden fazla sürücüde aynı anda arar |
| `fastfind -g "*.pdf" C:\Users` | Glob: tüm PDF'ler |
| `fastfind -r "^IMG_\d{4}"` | Regex (düzenli ifade) |
| `fastfind rapor -e pdf -e docx` | Yalnızca bu uzantılar |
| `fastfind node_modules -t d` | Yalnızca klasörler (`-t f` yalnızca dosyalar) |
| `fastfind rapor -d 2` | En fazla 2 seviye derine iner |
| `fastfind gizli -H` | Gizli dosya ve klasörleri de tarar |
| `fastfind config --gitignore` | `.gitignore` kurallarına uyar |
| `fastfind log -n 10` | 10 sonuçtan sonra durur |
| `fastfind log -q \| more` | Özet satırı yazmaz, pipe için kullanışlı |

Eşleştirme yalnızca **dosya adına** bakar ve `-s` verilmedikçe büyük/küçük harf
duyarsızdır. Sonuçlar her zaman tam yol olarak yazılır. Özet satırı stderr'e
gider, böylece stdout pipe'lar için temiz kalır.

Çıkış kodları: `0` = sonuç bulundu, `1` = sonuç yok, `2` = hata.

Tüm seçenekler için `fastfind --help` komutunu çalıştırın.

## Neden bu kadar hızlı?

- **Paralel gezinme:** ripgrep ve fd'nin de kullandığı [`ignore`](https://crates.io/crates/ignore)
  kütüphanesinin iş çalan (work-stealing) gezgini sayesinde her çekirdek aynı anda klasör okur.
- **Yalnızca isimler:** hiçbir dosya açılmaz, isim doğrudan klasör listesinden gelir.
- **SIMD ile metin arama:** alt dizgi aramaları `regex` kütüphanesi tarafından
  vektörleştirilmiş bir eşleştiriciye derlenir.
- **Paylaşılan sayaç yok:** her iş parçacığı kendi sayacını tutar ve sonunda bir kez
  birleştirir. Böylece iş parçacıkları aynı önbellek satırı için çekişmez.
- **Tek tamponlu yazıcı:** iş parçacıkları sonuçları bir kanal üzerinden tek bir yazıcıya
  gönderir. Bu yazıcı her satırda stdout'u kilitlemek yerine 64 KB'lık bir tampon kullanır.
- **Release profili:** fat LTO, tek codegen birimi, `panic = "abort"`.

## Yol haritası

- [ ] Tekrarlanan aramaların anında sonuçlanması için isteğe bağlı indeks/önbellek
- [ ] NTFS dosya tablosunu (MFT) doğrudan okuma (*Everything* gibi)
- [ ] Bulanık (fuzzy) eşleştirme
- [ ] GitHub Releases'ta hazır derlenmiş dosyalar

## Lisans

[MIT](LICENSE)
