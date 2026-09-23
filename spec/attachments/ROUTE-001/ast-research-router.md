# AST Research — Language checkpoint routing (ROUTE-001)

Generated via AstGrep + direct read of `src/router.rs` during reverse ACDD discovery.

## Public API (src/lib.rs)

```rust
#[cfg(not(target_arch = "wasm32"))]
pub mod router;
#[cfg(not(target_arch = "wasm32"))]
pub use router::{route, Checkpoint};
```

Native-only — the browser demo picks its checkpoint from the model picker instead (website/src/lib/models.ts).

## `Checkpoint` (src/router.rs:14-18)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkpoint {
    English,
    Multilingual,
}
```

## `Script` + `SCRIPT_RANGES` (src/router.rs:20-77)

26-script enum (Latin, Greek, Cyrillic, Armenian, Hebrew, Arabic, Devanagari, Bengali,
Gurmukhi, Gujarati, Oriya, Tamil, Telugu, Kannada, Malayalam, Sinhala, Thai, Lao, Georgian,
Hangul, Ethiopic, Khmer, Myanmar, Han, Hiragana, Katakana) with Unicode codepoint ranges.
Note: no Coptic, no Tifinagh, no Mongolian, no Runic, no Deseret, no Glagolitic, etc. —
only the 26 blocks above are recognized; everything else counts as "no alphabetic content".

## `classify_char` (src/router.rs:79-82)

Linear scan over `SCRIPT_RANGES` (26 entries, no ordering guarantee beyond the table);
returns `Option<Script>`.

## `script_histogram` (src/router.rs:85-95)

Per-script codepoint counts, sorted descending by count. Returns `Vec<(Script, usize)>`.

## `route` (src/router.rs:106-116)

```rust
pub fn route(text: &str) -> Checkpoint {
    let hist = script_histogram(text);
    match hist.first().map(|&(s, _)| s) {
        Some(s) if s != Script::Latin => return Checkpoint::Multilingual,
        _ => {}
    }
    match whichlang::detect_language(text) {
        Lang::Eng => Checkpoint::English,
        _ => Checkpoint::Multilingual,
    }
}
```

- Dominant non-Latin script → `Multilingual` immediately (whichlang never runs).
- Dominant Latin (or empty histogram) → whichlang decides; `Lang::Eng` → `English`,
  everything else → `Multilingual`.

## Existing tests (src/router.rs:118-131)

- `english_stays_english` — the exact "password reset / account locked" sentence → `Checkpoint::English`.
- `devanagari_routes_multilingual` — "मुझसे दो बार शुल्क लिया गया" → `Checkpoint::Multilingual`.

## whichlang coverage

whichlang 0.1.1's `Lang` enum has 16 languages: Ara, Cmn, Deu, Eng, Fra, Hin, Ita, Jpn,
Kor, Nld, Por, Rus, Spa, Swe, Tur, Vie. French is `Lang::Fra` — the French scenario is
deterministic on all native targets.

## Testability

Fully weight-free and deterministic. `route` and `Checkpoint` are re-exported from the
lib (`laya::route`, `laya::Checkpoint`), so integration tests can call them directly.
Native-only: tests must be gated to `cfg(not(target_arch = "wasm32"))` or run via the
native test harness only (cargo test defaults are native — no gate needed for `cargo test`).
