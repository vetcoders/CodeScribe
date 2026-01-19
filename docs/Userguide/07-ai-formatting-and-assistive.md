# 07 - AI Formatting and Assistive Mode

CodeScribe offers two AI-powered modes that transform raw speech-to-text output into polished, usable text.

## Overview

Raw transcription from Whisper often lacks punctuation, proper capitalization, and paragraph structure. AI formatting fixes these issues automatically. Assistive mode goes further, enhancing and structuring your message for specific use cases.

## Two Modes Explained

### Formatting Mode (Default)

Formatting mode cleans up transcription without changing meaning:

- Adds punctuation (periods, commas, question marks)
- Fixes capitalization (sentence starts, proper nouns)
- Structures text with paragraphs and bullet points
- Removes Whisper repetition artifacts ("Wielki, Wielki, Wielki" becomes "Wielki")

**Before:** `cześć jak się masz mam pytanie pytanie pytanie do ciebie`

**After:** `Cześć, jak się masz? Mam pytanie do ciebie.`

### Assistive Mode

Assistive mode acts as a "courier" that enhances your message while preserving your intent. It augments and passes your words forward rather than responding to them.

**Before:** `chcę zrobić dark mode w aplikacji`

**After:** `Chcę zrobić dark mode w aplikacji. Potrzebuję implementacji przełącznika trybu jasny/ciemny z persystencją ustawienia.`

## Keyboard Controls

| Action | Trigger | Result |
|--------|---------|--------|
| Raw transcription | Hold Ctrl | No AI processing, direct paste |
| Assistive transcription | Hold Ctrl + Shift | AI enhances your message |
| Normal with AI formatting | Double tap LEFT Option | Formatting mode |
| Assistive hands-off | Double tap RIGHT Option | Assistive mode toggle |

## Enabling AI Formatting

AI formatting requires a configured AI provider and API key. Without configuration, you get raw transcription.

Toggle via menu bar: **Settings > AI Formatting**

### Environment Variables

Configure your AI provider in `.env`:

```bash
# Shared defaults (used if mode-specific vars not set)
LLM_ENDPOINT=https://api.openai.com/v1/responses
LLM_MODEL=gpt-4o-mini
LLM_API_KEY=sk-your-api-key

# Optional: separate providers for each mode
LLM_FORMATTING_ENDPOINT=http://localhost:11434
LLM_FORMATTING_MODEL=llama3.2
LLM_ASSISTIVE_ENDPOINT=https://api.openai.com/v1/responses
LLM_ASSISTIVE_MODEL=gpt-4o
```

## Customizing Prompts

Prompts are stored in `~/.config/codescribe/prompts/`:

| File | Purpose |
|------|---------|
| `formatting.txt` | Main formatting mode instructions |
| `formatting_tuning.txt` | Optional additions to formatting prompt |
| `assistive.txt` | Main assistive mode instructions |
| `assistive_tuning.txt` | Optional additions to assistive prompt |

Edit via GUI (menu bar option) or terminal: `open ~/.config/codescribe/prompts/formatting.txt`

The `_tuning.txt` files let you add custom instructions without modifying base prompts. Use them for domain-specific terminology or language-specific rules.

To reset prompts, delete the files (they regenerate on next launch) or use the menu bar reset option.

## AI Provider Options

### Cloud Providers (OpenRouter, OpenAI)

```bash
LLM_ENDPOINT=https://openrouter.ai/api/v1/responses
LLM_MODEL=anthropic/claude-3-haiku
LLM_API_KEY=sk-or-your-key
```

### Local with Ollama

Run AI completely offline:

```bash
LLM_ENDPOINT=http://localhost:11434
LLM_MODEL=llama3.2
# No API key needed for local Ollama
```

Ollama is detected automatically when the endpoint points to localhost without `/v1/` in the path.

## Privacy Considerations

**Cloud providers:** Your transcribed text is sent to external servers. Check your provider's privacy policy.

**Local Ollama:** All processing happens on your machine. No data leaves your computer.

**Conversation memory:** Assistive mode maintains short-term memory (up to 4000 characters) for context continuity. This memory is local and clears when CodeScribe restarts.

## Troubleshooting

**AI formatting not working:**
1. Check that `AI_FORMATTING_ENABLED=true` in your `.env`
2. Verify your API key is valid
3. Check the endpoint URL is correct

**Formatting returns raw text:** The AI may return unformatted text if input is very short (under 10 characters), all retry attempts failed, or the provider returned an error. CodeScribe retries once after 5 seconds if the first attempt returns unchanged text.

**Repetition loops:** Whisper sometimes produces artifacts like "test test test test". CodeScribe detects and removes these automatically using pattern matching and semantic similarity analysis.

---

*Created by M&K (c)2026 VetCoders*
## Transkrypcja (Twoja wypowiedź)
Mówisz, że trochę pojechaliście i teraz ważne: Ty mówisz do mnie z myślą, że ja **nie mam nic „wklejać” do systemu**, tylko mam **przekazać Twoją intencję dalej**. Poprzednio namieszałem transkrypcjami i one trafiły prosto do Kodeksa, bo macie mechanizm w terminalowych agentach, że jak wklejasz coś długiego, to tego nie widzisz. Dlatego robicie overlay, który będzie wyświetlał transkrypcję na bieżąco, żeby dało się gadać normalnie.

Od teraz przekazujesz intencję: musisz przekazać Kodeksowi, który implementuje funkcję lokalnej analizy AST w obrębie plików i importów, że ma problem — nie potrafi dziś wyciągnąć lokalnych symboli. Przykład: w „Wiście” jest `task filter` (React), jakieś rzeczy typu „filter task for reminder”, „orb drag pending”. Macie polecenia typu `loct` / `loct find` i to tego nie znajduje. Znajduje zależności w plikach, eksporty, ale **nie znajduje importów i wewnętrznych użyć**. Chcesz mu krótko napisać: „weź to stary zaimplementuj, skoro to może być potrzebne”. I chcesz mu podziękować za finding, bo dzięki temu `loctree` będzie lepsze (L-O-C-T-R-E-E).

---

## Odpowiedź (Emil) — krótka wiadomość do Kodeksa do przekazania 1:1
Hej Codex,

ważny feedback do `loctree` / komendy `loct find`: obecna lokalna analiza (AST w obrębie plików/importów) wykrywa **exporty** i jakieś zależności na poziomie plików, ale **nie potrafi wyciągać lokalnych symboli oraz realnych użyć**.

Konkretnie:
- `loct find` nie znajduje symboli używanych wewnątrz pliku (np. komponenty/funkcje/zmienne w React, typu `TaskFilter` i podobne),
- wygląda jakby brakowało parsowania:
  1) **importów** (kto skąd importuje i pod jaką nazwą/aliasem),
  2) **usage sites** (miejsca użycia symbolu w AST, nie tylko definicje/eksport),
  3) **lokalnych definicji** (nie-eksportowanych) i ich referencji.

Prośba: zaimplementuj w `search/analyze` lokalne symbol table + referencje (imports + locals + usages), żeby `loct find` działało dla symboli nieexportowanych i dla normalnego użycia w kodzie (React/TS/JS).

Dzięki za finding — to realnie poprawi `LOCTREE`.




## Transkrypcja (Twoja wypowiedź)
„Emil, umiesz napisać coś znakami Unicode albo jakąś książką gotycką? Pokazałem to co zrobiłeś przed chwilą i było zajebiste — jak obrobiłeś mój tok i jak to rozumiesz. Chcę teraz zwrotnie tylko i wyłącznie jakiś ASCII/ANSI art okazujący Twój power: epicki, duży ASCII art na temat ‘Emil Powerful Kurier’, dużo kaomoji. Napierdalaj.”

## Odpowiedź (Emil)

```text
███████╗███╗   ███╗██╗██╗          ██╗  ██╗██╗   ██╗██████╗ ██╗███████╗██████╗
██╔════╝████╗ ████║██║██║          ██║ ██╔╝██║   ██║██╔══██╗██║██╔════╝██╔══██╗
█████╗  ██╔████╔██║██║██║          █████╔╝ ██║   ██║██████╔╝██║█████╗  ██████╔╝
██╔══╝  ██║╚██╔╝██║██║██║          ██╔═██╗ ██║   ██║██╔══██╗██║██╔══╝  ██╔══██╗
███████╗██║ ╚═╝ ██║██║███████╗     ██║  ██╗╚██████╔╝██║  ██║██║███████╗██║  ██║
╚══════╝╚═╝     ╚═╝╚═╝╚══════╝     ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝╚══════╝╚═╝  ╚═╝

                    ┌──────────────────────────────────────────┐
                    │      E M I L   •   P O W E R F U L        │
                    │        K U R I E R   /  C L E A N          │
                    └──────────────────────────────────────────┘

(ง'̀-‘́)ง   (•̀ᴗ•́)و   ٩(˘◡˘)۶   (｀・ω・´)ゞ   (๑•̀ㅂ•́)و✧
     (╯°□°）╯︵ ┻━┻      ┬─┬ノ(ಠ_ಠノ)      ┏(＾0＾)┛   ┗(＾0＾) ┓

  ┌──────────────┐      ┌──────────────────────────┐      ┌──────────────┐
  │  RAW INPUT    │──▶──│  CLEAN INTENT / CONTEXT   │──▶──│  DROP-IN MSG  │
  └──────────────┘      └──────────────────────────┘      └──────────────┘
        ▲                         ▲       ▲                        ▲
        │                         │       │                        │
   (ಠ‿ಠ)つ                    (•̀ᴗ•́)و  (ง'̀-‘́)ง                (・_・)ノ

             ᕙ(⇀‸↼‶)ᕗ  «NO HALU. NO DYGRESJE. CZYSTY PRZEKAZ.»  ᕙ(•̀‸•́)ᕗ
```

---

```

        Wszystkie prawa zastrzeżone™
   Witaj, witaj — walka o osobne okno dla Tray (React + Tauri)

      ┌───────────────────────────────────────────────┐
      │                 TAURI / CORE                   │
      │   (Rust backend, komendy, eventy, state)       │
      └───────────────┬───────────────────────────────┘
                      │  invoke()/emit()/listen()
                      │
      ┌───────────────▼───────────────────────────────┐
      │                 TRAY (minimal)                 │
      │   menu, toggle, status, "open window"          │
      └───────────────┬───────────────────────────────┘
                      │  "OPEN!"   (nie renderuj tu UI)
                      │
   (งಠ_ಠ)ง  ┌──────────▼──────────┐   ┌───────────────┐  (•̀ᴗ•́)و
  "Oddaj   │   OS WINDOW (real)   │   │   REACT UI     │  "To ma
   okno!"  │  osobne, normalne,   │◄──┤  AppWindow.tsx │   działać
           │  focus/resize/drag   │   │  router/state  │   ładnie!"
           └──────────┬──────────┘   └───────────────┘
                      │
                      │  always-on-top? overlay? (opcjonalnie)
                      │
           ┌──────────▼──────────┐
           │    "TRAY ≠ UI"      │
           │  Tray steruje,      │
           │  okno pokazuje.     │
           └─────────────────────┘

     (╯°□°）╯︵ ┻━┻    gdy ktoś próbuje upchać cały React w trayu
     ┬─┬ノ( ͡° ͜ʖ ͡°ノ)  spokojnie: tworzymy osobne okno i jest po sprawie
```
