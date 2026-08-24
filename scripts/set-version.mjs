/**
 * Sürüm alanlarını TEK yerden günceller.
 *
 *   node scripts/set-version.mjs 0.2.0
 *   node scripts/set-version.mjs 0.2.0-alpha.1
 *
 * Sürüm dört ayrı dosyada tekrarlanıyor ve hepsi birbirine uymak zorunda:
 * `tauri.conf.json` kurulum paketinin ve updater manifestinin sürümünü,
 * `Cargo.toml` derlenen ikilinin sürümünü, `package.json` de depo tarafındaki
 * sürümü belirliyor. `Cargo.lock` bunlardan türese de, güncellenmezse
 * `--locked` derlemeler "lock dosyası güncel değil" diye patlıyor.
 *
 * Elle güncellemenin kaçınılmaz sonucu ikisinin birbirinden ayrı düşmesi:
 * kurulan uygulamanın sürümü bir şey, updater manifestindeki başka bir şey
 * olur ve güncelleme döngüye girer. Bu yüzden hem yerel derlemeler hem de CI
 * aynı betiği çağırıyor.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Kabul edilen biçim: `1.2.3`, `1.2.3-alpha.4`, `1.2.3-beta.4`.
 *
 * Yalnızca bu iki ön-sürüm sonekine izin var, çünkü güncelleme kanalı tam
 * olarak bu sonekten türetiliyor (bkz. `.github/workflows/release.yml`).
 * Serbest bırakılsaydı `1.2.3-rc.1` gibi bir tag hiçbir kanala düşmezdi.
 */
const VERSION_RE = /^\d+\.\d+\.\d+(-(alpha|beta)\.\d+)?$/;

/**
 * Not: aşağıdaki düzenli ifadelerde `\r?` şart. Depo Windows'ta CRLF ile
 * checkout ediliyor ve `$` satır sonundaki `\r`'den ÖNCE eşleşmiyor; `\r?`
 * olmadan hiçbir satır bulunamıyor.
 */
export function setVersion(version) {
	if (!VERSION_RE.test(version)) {
		throw new Error(
			`Geçersiz sürüm: "${version}". Beklenen: 1.2.3 | 1.2.3-alpha.1 | 1.2.3-beta.1`
		);
	}

	const pkgPath = join(ROOT, "package.json");
	const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
	pkg.version = version;
	writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

	const confPath = join(ROOT, "src-tauri", "tauri.conf.json");
	const conf = JSON.parse(readFileSync(confPath, "utf8"));
	conf.version = version;
	writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");

	// `[package]` bölümündeki İLK `version` satırı: bağımlılıkların kendi
	// `version = "..."` satırlarına dokunulmamalı.
	const cargoPath = join(ROOT, "src-tauri", "Cargo.toml");
	const cargo = readFileSync(cargoPath, "utf8");
	const cargoVersion = /^version = ".*"\r?$/m;
	// Eşleşme ARANARAK doğrulanıyor, sonuç karşılaştırılarak değil: sürüm
	// zaten istenen değerdeyse `replace` aynı metni döndürür ve karşılaştırma
	// bunu "satır bulunamadı" sanardı.
	if (!cargoVersion.test(cargo)) throw new Error("Cargo.toml içinde version satırı bulunamadı");
	writeFileSync(cargoPath, cargo.replace(cargoVersion, `version = "${version}"`));

	// Lock dosyasında yalnızca BU paketin bloğu güncelleniyor; aynı ada sahip
	// ikinci bir giriş olamayacağı için tek eşleşme yeterli.
	const lockPath = join(ROOT, "src-tauri", "Cargo.lock");
	const lock = readFileSync(lockPath, "utf8");
	const name = /^name = "(.+)"\r?$/m.exec(cargo)?.[1];
	const lockEntry = new RegExp(`(name = "${name}"\\r?\\nversion = ").*(")`);
	if (lockEntry.test(lock)) {
		writeFileSync(lockPath, lock.replace(lockEntry, `$1${version}$2`));
	} else {
		console.warn(`UYARI: Cargo.lock içinde "${name}" girdisi güncellenemedi.`);
	}

	return version;
}

// Doğrudan çalıştırıldığında (import edildiğinde değil) argümanı işle.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	const version = process.argv[2];
	if (!version) {
		console.error("Kullanım: node scripts/set-version.mjs <sürüm>");
		process.exit(1);
	}
	try {
		setVersion(version);
		console.log(`Sürüm alanları güncellendi: ${version}`);
	} catch (e) {
		console.error(String(e instanceof Error ? e.message : e));
		process.exit(1);
	}
}
