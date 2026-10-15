# Day 15 metadata - A Word to the Wise

New kind: `ooxml-external-relationship-targets` (`d15-ooxml-NNN-ooxml-external-relationship-targets`) on a `.docx`:
every `<Relationship ... TargetMode="External">` in the package's `.rels` parts, as compact JSON.

**Graded order: sorted by value.** The single-solve doc had `word/_rels/document.xml.rels` rId9
`https://example.test/evidence/<token>` then rId10 `https://example.test/control`; file order was `incorrect`,
`["https://example.test/control","https://example.test/evidence/<token>"]` CORRECT. A string sort of the rIds gives the
same order here (rId10 < rId9) - if a future variant separates the two, check with the untimed oracle.

Implementation: `ctf-run/src/formats/ooxml.rs` (zip package, `.rels` scanner with single-pass XML unescape, core/app
props) + `ooxml_answer` in `ctf-run/src/lanes/metadata.rs` (also answers relationship ids/counts, hyperlink targets,
core/app properties by name - guesses, unverified). FACTS dumps now include OOXML rels/props.

TT (20 questions) and mastery (100 questions, 5 sets) were the cumulative bundle + this kind; all CORRECT first try.
Route: dual-pool, ranked 32 / regular 32; TT 50 s, mastery 170 s.
