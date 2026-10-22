# Day 21 metadata - Package Deal

New kind: `apk-package-name` (`d21-apk-001-apk-package-name`) on `mobile-001.apk` (8535 B zip with
`AndroidManifest.xml` (binary AXML), `classes.dex`, and a v1 JAR signature `META-INF/FIXTURE-.SF/.RSA` plus `MANIFEST.MF`).
The central directory sits at offset 8192, after a padding gap. The `.SF` says `X-Android-APK-Signed: 2`, but there is
no APK Signing Block. Answer format `exact_text`, "use the literal none only when the requested field is absent".
Graded **CORRECT on the first submit**: the manifest `package` attribute, verbatim (`com.huntress.arena.p08281778`).

The first pass refused because there was no APK kind. Fix:
- `formats/apk.rs` (new): an AXML parser (string pool UTF-8/UTF-16, resource map, start/end element chunks). Attribute
  names resolve through the resource map first, using framework ids like `0x0101021b` versionCode, so a renamed pool
  string cannot hide an `android:` attribute. Typed values print like aapt does: strings, decimal ints, bools,
  `@0x..` refs. It also has a plain-text manifest fallback, `resources.arsc` lookup for `@0x7f..` refs (default config
  preferred), PKCS#7 cert extraction (`META-INF/*.RSA|DSA|EC`), APK Signing Block v2/v3/v3.1 certs, and a tiny X.509
  reader (subject/issuer/serial/validity).
- `lanes/metadata.rs::apk_answer` handles any `-apk-<field>` key, except fields that start with a digit, so
  `d21-apk-001-sha256` still reaches the hash handlers. Only `package-name` is graded. These fields are ungraded guesses:
  `version-code/name`, `min/target-sdk`, `app-label` (+aliases like `product-name`), `launcher-activity`,
  `permissions` (sorted JSON), `signer-sha256/sha1` (hash of the first cert's DER), `signer-subject/issuer`
  (RDNs **reversed** from encoding order, so `C=US, O=Huntress Labs, CN=Huntress Artifact Assembly Line`; keytool would
  print `CN=..., O=..., C=US`; unverified), `signer-cn`, `signer-serial`. Unknown fields error with a full manifest and
  cert dump (`apk_facts`, also in `facts()`).
- Unit test `apk_fields` uses `testdata/d21-mobile-001.apk`.

TT (26 questions) and mastery (130, 5 sets) are the cumulative bundle plus this kind. Every set used the same
`mobile-001.apk`. All CORRECT on the first try (untimed regular attempts). Fixtures: `ctf-run/fixtures/metadata/d21-*.json`.

Sample facts: manifest versionCode=1 versionName=1.0 platformBuildVersionCode=23; uses-sdk min/target 23;
application label `Huntress Evidence 08281778` (a literal, not a reference), theme `@0x01030237`, allowBackup=false;
activity `.MainActivity` exported with MAIN/LAUNCHER. The signer is self-signed `CN=Huntress Artifact Assembly Line, O=Huntress Labs, C=US`,
cert sha256 `e5605ad68fe614df4316c17f0b4c3f2a88a4fa26bfc67348e04851b67ee800ff`.

Prompt: "Mobile packages have been mirrored under friendly filenames. Recover Android package identities,
distinguish rebuilds from clones, and expose the borrowed product name." Next-day guesses: several APKs per set. A
"rebuild" would share package + signer and differ in versionCode. A "clone" would share the label/product name under a
different package and/or signer. Questions could be versionCode, signer cert sha256, app label (maybe a
`resources.arsc` reference, which is already resolved), or a JSON list of clone filenames. The `-apk-` guesses above
cover the single-file fields. Settle formats with free single-solve probes.
