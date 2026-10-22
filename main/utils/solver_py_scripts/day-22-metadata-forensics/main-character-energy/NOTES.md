# Day 22 metadata - Main Character Energy

New kind: `jar-main-class` (`d22-jar-001-jar-main-class`) on `application-001.jar` (1146 B zip: `META-INF/MANIFEST.MF`,
`arena/evidence/ArtifactMain.class`, `evidence/header.bin` = `HUNTRESS`, `evidence/marker.txt`). The answer format is
`exact_text` ("use the literal none only when the requested field is absent").
Graded **CORRECT on the first submit**: the manifest `Main-Class`, verbatim (`arena.evidence.ArtifactMain`). The
manifest uses CRLF and has `Created-By: Huntress Artifact Assembly Line`. The class declares
`public static void main(String[])` and prints `JAR artifact <marker>`.

The first pass refused because there was no JAR kind. Fix:
- `formats/jar.rs` (new): a manifest parser (main section plus per-entry `Name:` sections, 72-byte continuation lines
  joined, case-insensitive lookup, CR/LF/CRLF, BOM). It reads every `.class` through `jvm::class_info` and records
  `this_class` and whether the class has a public static `main([Ljava/lang/String;)V`. Nested `*.jar`/`*.war`
  members are parsed recursively (depth 4).
- `lanes/metadata.rs::jar_answer` handles any `-jar-<field>` key, except fields that start with a digit, so
  `d22-jar-001-sha256` still reaches the hash handlers. `main-class` (and `start-class`): the attribute verbatim, or
  `none` when it is absent. If the named class is missing from the archive or has no launcher `main`, that is an
  **error**, not a guess: the prompt says "manifests disagree with their launchers", and the graded rule for that
  case is unknown. These fields are ungraded guesses: `is-runnable`, `class-count`, `nested-jars` (sorted JSON), and
  any manifest attribute by name (`created-by`, `implementation-version`, ...). Unknown fields error with
  `jar_facts`: the manifest, the classes (`+main` marked, `this_class` shown when it differs from the path), and
  nested jars. `jar_facts` is also in `facts()`.
- Unit test `jar_fields` uses `testdata/d22-application-001.jar`.

TT (27 questions) and mastery (162, 6 sets) are the cumulative bundle plus this kind. Every set used the same
`application-001.jar`. All CORRECT on the first try (untimed regular attempts). Fixtures:
`ctf-run/fixtures/metadata/d22-*.json`.

Prompt: "Java archives contain libraries, applications, and nested dependencies whose manifests disagree with their
launchers. Determine what is runnable and where execution begins." Next-day guesses:
- a library jar with no `Main-Class` (-> `none`, or `false` for runnable);
- a `Main-Class` naming a class that is absent, or present without a static `main` (the lane errors; settle the rule
  with free single-solve probes);
- Spring Boot style `Main-Class: ...JarLauncher` plus `Start-Class` (the real entry point);
- nested `BOOT-INF/lib/*.jar` or `lib/*.jar` dependencies, whose own manifests may name a different main class;
- `Class-Path`, `Launcher-Agent-Class`, `Multi-Release`, or `Automatic-Module-Name` fields.
