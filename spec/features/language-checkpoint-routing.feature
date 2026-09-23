@done
@language-routing
@decision-engine
@ROUTE-001
Feature: Route a state to the English or multilingual checkpoint
  """
  Two-stage router (src/router.rs): (1) a 26-script Unicode block detector over the state text — dominant non-Latin script returns Checkpoint::Multilingual immediately; (2) for dominant-Latin (or no-alphabetic) text, whichlang::detect_language decides — Lang::Eng → Checkpoint::English, everything else → Multilingual. whichlang is native-only (cfg(not(target_arch = "wasm32"))), so `route` and the `Checkpoint` type are not compiled for wasm. Deterministic, no model confidence involved — an English-only checkpoint can never be confidently wrong on an unreadable script.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. A fast Unicode script detector counts codepoints over 26 script blocks (Latin, Greek, Cyrillic, Armenian, Hebrew, Arabic, Devanagari, Bengali, Gurmukhi, Gujarati, Oriya, Tamil, Telugu, Kannada, Malayalam, Sinhala, Thai, Lao, Georgian, Hangul, Ethiopic, Khmer, Myanmar, Han, Hiragana, Katakana); when the dominant script is anything other than Latin, the state routes to the multilingual checkpoint immediately, without running any language-ID model
  #   2. When the dominant script is Latin (or there is no alphabetic content at all), the router defers to the whichlang statistical language-ID model: English routes to the English checkpoint, any other detected language routes to the multilingual checkpoint
  #
  # EXAMPLES:
  #   1. Routing an ordinary English sentence like "A customer says a password reset succeeded, but every login attempt still returns 'account locked'." returns the English checkpoint — a hand-rolled stopword heuristic once misrouted exactly this text, which is why whichlang is used
  #   2. Routing a Devanagari sentence (e.g. "मुझसे दो बार शुल्क लिया गया") returns the multilingual checkpoint straight from the script detector, without invoking whichlang
  #   3. Routing a French sentence (e.g. "Le client signale que la facture de mars a été facturée deux fois.") returns the multilingual checkpoint — script alone can't tell French from English, so whichlang decides
  #
  # ========================================
  Background: User Story
    As a caller of `route()` (the CLI ask/demo paths, RLAgent)
    I want to determine, from a state's text, which laya checkpoint can actually read it
    So that an English-only checkpoint is never confidently wrong on a script it can't read

  Scenario: Ordinary English text routes to the English checkpoint
    Given the state text is the English sentence "A customer says a password reset succeeded, but every login attempt still returns 'account locked'."
    When the router runs `route()` over the state text
    Then the router returns the English checkpoint

  Scenario: Devanagari text routes to the multilingual checkpoint
    Given the state text is the Devanagari sentence "मुझसे दो बार शुल्क लिया गया"
    When the router runs `route()` over the state text
    Then the router returns the multilingual checkpoint

  Scenario: French text routes to the multilingual checkpoint
    Given the state text is the French sentence "Le client signale que la facture de mars a été facturée deux fois."
    When the router runs `route()` over the state text
    Then the router returns the multilingual checkpoint
