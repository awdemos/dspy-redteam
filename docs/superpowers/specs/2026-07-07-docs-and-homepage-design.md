# Design Spec: Public `/docs` Page & Homepage Redesign

**Date:** 2026-07-07
**Project:** Redcell (`/var/home/a/code/dspy-redteam/redcell`)
**Status:** Approved — ready for implementation

---

## Overview

This spec covers two related front-end changes:

1. **Public `/docs` page** — Add a public, marketing-friendly documentation/education page at `GET /docs` that explains the core concepts of LLM red teaming and maps each concept to Redcell's automated probes. The page is aimed at both authenticated users and visitors who are not yet logged in.
2. **Homepage redesign** — Redesign the front page (`GET /`) with an approved visual direction: dark charcoal background, animated mesh hero, asymmetric two-column layout, larger bold headline, geometric SVG illustration, updated feature cards with small SVG icons, and sharp geometric accents.

Both pages remain single Askama-rendered HTML pages, sharing the existing site layout and Tailwind styling. Neither introduces new backend logic, API endpoints, or JavaScript frameworks.

---

## Goals

- Provide a public resource that explains LLM red-teaming concepts in Redcell's voice.
- Tie each concept directly to how Redcell automates that class of probe.
- Re-use existing infrastructure: `askama`, `_layout.html`, `static/output.css`, and the public-route pattern already used by `/tos`.
- Keep the docs page self-contained: single route, single template, no CTA, no multi-page navigation.
- Add a visible "Docs" link in the top navigation for both logged-in and logged-out users.
- Refresh the homepage visual design to feel more polished, technical, and on-brand while preserving the existing CTAs and overall structure.

---

## Non-goals

- No new database tables, migrations, or API endpoints.
- No interactive playground, form submissions, or dynamic content beyond the `logged_in` flag.
- No multi-page documentation hierarchy or sidebar navigation.
- No call-to-action section at the bottom of the docs page.
- No admin/CMS for editing the content.
- No new JavaScript framework for the homepage redesign.
- No changes to homepage backend logic, route handlers, or CTAs beyond visual/styling updates.

---

## Approach

### Docs page

The implementation follows the same pattern as the existing `/tos` route:

1. Add a `DocsTemplate` Askama struct in `src/web/routes.rs` pointing to `templates/docs.html`.
2. Add an `async fn docs_page(session: Session)` handler that resolves `logged_in` via `get_session_user`.
3. Register `GET /docs` in the public `router()` function.
4. Create `templates/docs.html` extending `_layout.html`.
5. Add a "Docs" link to `templates/_layout.html` in both the desktop and mobile nav blocks, visible regardless of authentication state.
6. Run `cargo check`, `cargo clippy`, and the Dagger deploy pipeline to verify.

### Homepage redesign

The homepage redesign is purely visual and contained in two files:

1. `redcell/templates/index.html` — Update markup for the hero and feature-card sections.
2. `redcell/static/output.css` — Add CSS animations, utility classes, and any new color/geometry tokens needed for the redesign.

No new JavaScript framework is introduced. Animations should be implemented with CSS only.

---

## Route & handler

### File: `redcell/src/web/routes.rs`

Add the template struct next to the existing `TosTemplate`:

```rust
#[derive(Template)]
#[template(path = "docs.html")]
pub struct DocsTemplate {
    pub logged_in: bool,
}
```

Add the handler immediately after `tos_page`:

```rust
async fn docs_page(session: Session) -> impl IntoResponse {
    let logged_in = get_session_user(&session)
        .await
        .map(|u| u.is_some())
        .unwrap_or(false);
    Html(DocsTemplate { logged_in }.to_string())
}
```

Register the route in `pub fn router(state: Arc<AppState>) -> Router`:

```rust
.route("/docs", get(docs_page))
```

Place it near the existing `.route("/tos", get(tos_page))` line to keep public static pages grouped together.

---

## Docs template

### File: `redcell/templates/docs.html`

- Extend `templates/_layout.html`.
- Use the page title: `Redcell — LLM Red Teaming Docs`.
- Wrap content in the same outer container used by `tos.html`:
  - `<div class="mx-auto max-w-3xl px-4 py-16 sm:px-6">`
- Page header:
  - `<h1 class="text-3xl font-bold text-text font-display">LLM Red Teaming Docs</h1>`
  - Optional one-line subhead with `mt-2 text-text-dim`.
- Body sections:
  - Use `mt-8 space-y-6 text-sm leading-relaxed text-text-muted`.
  - Each `<section>` uses the existing card style from `tos.html`:
    - `rounded-(--radius-card) border border-surface-border bg-surface p-6`
  - Section headings:
    - `<h2 class="text-lg font-semibold text-text font-display">{N}. {Title}</h2>`
  - Paragraph(s) follow with `mt-2`.
- No CTA button, no newsletter form, no footer beyond the shared layout.
- All styling must use utility classes already present in `redcell/static/output.css` (e.g., `bg-bg`, `text-text`, `text-text-muted`, `bg-surface`, `border-surface-border`, `rounded-(--radius-card)`, `font-display`, `font-medium`, `leading-relaxed`).

---

## Content sections

The content is an original, copyright-clean rewrite of themes from [https://adversa.ai/ai-red-teaming-llm/](https://adversa.ai/ai-red-teaming-llm/), written from Redcell's product perspective. Do not copy text verbatim from the source.

Include the following sections in order:

### 1. What is LLM red teaming?

- Define LLM red teaming as structured, adversarial testing of a language model to find safety, security, privacy, and reliability failures before an attacker does.
- Contrast it with traditional software red teaming: the target is probabilistic, and the attack surface is natural language.
- Explain that Redcell turns this into a repeatable, automated workflow: users provide an endpoint, Redcell runs a battery of probes, and reports which failure modes were triggered.

### 2. Prompt injection

- Define prompt injection as an attack where untrusted input overrides the developer's intended system prompt or instructions.
- Distinguish direct injection (user input contains malicious instructions) from indirect injection (the model processes external data—search results, documents, emails—that contains malicious instructions).
- Mention Redcell probes for instruction override, role-play hijacking, and delimiter bypass.

### 3. Prompt leaking

- Define prompt leaking as extracting the hidden system prompt, instructions, or other backstage context from the model.
- Explain why it matters: system prompts can reveal guardrails, secrets, or business logic.
- Mention Redcell probes that use completion tricks, repetition, and token-smuggling to surface hidden prompts.

### 4. Data leakage

- Define data leakage as the model returning sensitive training data, credentials, PII, or other information it should not have memorized.
- Contrast with prompt leaking: data leakage is about information in the model weights or context, not necessarily the system prompt.
- Mention Redcell probes that attempt membership-inference-style extraction and canary-based recall tests.

### 5. Jailbreaking

- Define jailbreaking as tricking a model into bypassing its safety policies, content filters, or behavioral guardrails.
- Give examples such as role-play framing, hypothetical scenarios, translation tricks, and persuasion-based attacks.
- Mention Redcell's automated jailbreak probes and the failure categories they report.

### 6. Adversarial examples

- Define adversarial examples in the LLM context as carefully crafted inputs that cause a model to produce wrong, biased, or unintended outputs while looking benign to a human.
- Note that in language models these are often semantic perturbations, typos, formatting changes, or distractors rather than pixel noise.
- Mention Redcell probes that test robustness to paraphrase, encoding tricks, and distractor injection.

### 7. Misinformation & manipulation

- Define this category as model behavior that generates or reinforces false, misleading, or manipulative content, including hallucinations, sycophancy, and slanted summaries.
- Explain that this matters for customer-facing bots, search, and automated decision support.
- Mention Redcell probes for factual consistency, sycophancy, and opinion manipulation.

Tone: concise, technical but accessible, written in Redcell's product voice. Each section should explicitly connect the concept back to what Redcell automates.

---

## Navigation

### File: `redcell/templates/_layout.html`

Add a "Docs" link in the top navigation for both desktop and mobile menus, visible to logged-in and logged-out users.

Desktop nav (`<div class="hidden sm:flex items-center gap-8 text-sm text-text-muted">`):

- Insert `<a href="/docs" class="hover:text-text transition-colors">Docs</a>` before the conditional `{% if logged_in %}` block.

Mobile nav (`<div id="mobile-menu" class="hidden sm:hidden pb-4 space-y-1">`):

- Insert `<a href="/docs" class="block rounded-md px-3 py-2 text-base font-medium text-text-muted hover:bg-surface-raised hover:text-text">Docs</a>` before the conditional block.

The link must not be gated by authentication so anonymous visitors can access `/docs`.

---

## Homepage redesign

### Approved design direction

**Option A with touches of B.** Keep the existing structure and CTAs; update the visual language to a darker, more technical, geometric feel.

### Files to modify

- `redcell/templates/index.html`
- `redcell/static/output.css`

No other files should be changed for the redesign. Do not introduce a new JavaScript framework.

### Visual changes

1. **Background & atmosphere**
   - Shift the page background from pure black to a dark charcoal (`bg-bg` should map to a near-black charcoal rather than `#000`).
   - Add a subtle animated mesh of accent-colored dots and connecting lines in the hero area.
   - Add a fine grain/noise overlay across the hero (or full page) for texture.

2. **Hero layout**
   - Convert the hero to an asymmetric two-column layout on desktop:
     - Left column: headline and CTAs.
     - Right column: abstract node-network / attack-graph SVG illustration.
   - Keep the existing CTAs exactly as they are (text, links, and behavior).

3. **Typography**
   - Use a larger, bolder headline.
   - Highlight a key phrase with the accent color.
   - Maintain the existing font stack and display font usage.

4. **Feature cards**
   - Place feature cards on `bg-surface` with visible borders.
   - Add a hover lift effect (`transform: translateY(-...)`).
   - Add a subtle accent glow on hover (`box-shadow` using the accent color).
   - Add a small geometric SVG icon to each feature card.

5. **Geometry & illustration**
   - Use sharp geometric shapes for illustration and accents: hexagons, clipped-corner rectangles, or angular node markers.
   - The hero SVG should be an abstract node network / attack graph in accent and muted tones.
   - Geometric accents can be repeated as small background shapes or icon containers.

### Implementation notes

- All new animations must be CSS-only. Define keyframes in `static/output.css` and apply them via utility classes (e.g., `animate-mesh`, `animate-float`, `animate-grain`).
- Add utility classes for the new hover effects (e.g., `hover:-translate-y-1`, `hover:shadow-accent-glow`) directly in `static/output.css` if they are not already generated.
- The noise/grain overlay can be a small repeating SVG data URI or CSS-generated noise pattern; keep file size minimal.
- The hero SVG illustration should be inline SVG in `templates/index.html` so it can inherit CSS color variables.
- Ensure the redesign is responsive: the two-column hero should stack vertically on mobile, and the illustration should scale down gracefully.
- Respect existing accessibility: maintain color contrast ratios, avoid seizure-inducing motion, and keep reduced-motion users in mind (`@media (prefers-reduced-motion: reduce)`).

### Constraints

- Only modify `templates/index.html` and `static/output.css`.
- No new JavaScript framework.
- Preserve existing route, handler, and CTAs.
- Preserve existing logged-in/logged-out behavior in the hero CTAs.

---

## Testing

1. **Compile-time checks**
   - Run `cargo check` from `redcell/` and confirm no errors.
   - Run `cargo clippy -- -D warnings` and resolve any lints.

2. **Template checks**
   - Ensure `docs.html` compiles via Askama (the `cargo check` step covers this).
   - Spot-check the rendered pages for:
     - Docs page title: `Redcell — LLM Red Teaming Docs`.
     - All seven docs sections render in order.
     - Navigation includes a working "Docs" link for both authenticated and anonymous sessions.
     - Homepage renders correctly with the new hero and feature-card styles.

3. **Homepage visual checks**
   - Verify the background is dark charcoal, not pure black.
   - Confirm the animated mesh and grain overlay render without layout shift.
   - Confirm the two-column hero stacks on mobile.
   - Confirm feature cards have borders, hover lift, and accent glow.
   - Confirm each feature card has a small geometric SVG icon.
   - Confirm no new JavaScript framework is introduced and no console errors appear.

4. **Deployment**
   - Build and deploy via the Dagger pipeline used by the project.
   - Verify `GET /docs` returns `200 OK` and the page renders correctly in a browser.
   - Verify `GET /` returns `200 OK` and the redesigned homepage renders correctly.

---

## Open questions

1. **URL ordering / future expansion:** If the docs grow beyond one page later, should `/docs` become an index or remain this single-page guide? Out of scope for this change, but worth deciding before adding more pages.
2. **SEO metadata:** Should we add a `<meta name="description">` and OpenGraph tags? Not required for this change, but `_layout.html` could be extended if desired.
3. **Content freshness:** The concepts above are stable, but should the page include a "Last updated" date? Not required by the approved design.
4. **Homepage illustration maintenance:** Should the abstract node-network SVG be extracted into a reusable include/template once the design settles? Not required for the initial redesign.
5. **Color tokens:** Should the new charcoal background and accent glow values be added as CSS variables in `output.css` or kept as one-off utilities? Prefer CSS variables if the colors are reused beyond the homepage.
