# Docs Page & Homepage Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a public `/docs` page with copyright-clean LLM red-teaming content and redesign the Redcell homepage to be more visually engaging with an animated mesh, asymmetric hero, and geometric accents.

**Architecture:** Follow the existing Axum + Askama pattern in `src/web/routes.rs` for the new `/docs` route, create `templates/docs.html` extending `_layout.html`, update `templates/index.html` for the homepage redesign, and add required CSS animations/utilities to `static/output.css`.

**Tech Stack:** Rust, Axum, Askama, Tailwind CSS, SQLite, Dagger, Fly.io

---

## Task 1: Add the `/docs` route and handler in `src/web/routes.rs`

**Files:** `redcell/src/web/routes.rs`

- [ ] Add the `DocsTemplate` Askama struct next to `TosTemplate`.
- [ ] Add the `docs_page` handler immediately after `tos_page`.
- [ ] Register `GET /docs` in the public `router()` function next to `/tos`.

```rust
// Add near the existing TosTemplate definition.
#[derive(Template)]
#[template(path = "docs.html")]
pub struct DocsTemplate {
    pub logged_in: bool,
}
```

```rust
// Add immediately after async fn tos_page(...).
async fn docs_page(session: Session) -> impl IntoResponse {
    let logged_in = get_session_user(&session)
        .await
        .map(|u| u.is_some())
        .unwrap_or(false);
    Html(DocsTemplate { logged_in }.to_string())
}
```

```rust
// In pub fn router(state: Arc<AppState>) -> Router:
.route("/", get(index_page))
.route("/tos", get(tos_page))
.route("/docs", get(docs_page))
.route("/login", get(login_page))
```

**Command to verify syntax:**

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo check
```

**Expected output:** no errors, ending with `Finished dev [unoptimized + debuginfo] target(s) in ...`.

**Commit:**

```bash
git add redcell/src/web/routes.rs
git commit -m "feat(routes): add GET /docs route and Askama handler"
```

---

## Task 2: Create `templates/docs.html` with all seven content sections

**Files:** `redcell/templates/docs.html` (new)

- [ ] Create the file extending `_layout.html`.
- [ ] Use page title `Redcell — LLM Red Teaming Docs`.
- [ ] Wrap content in `mx-auto max-w-3xl px-4 py-16 sm:px-6`.
- [ ] Add the page header and one-line subhead.
- [ ] Add the seven sections as cards using existing utility classes only.
- [ ] Do not add any CTA, newsletter, or footer beyond the shared layout.

```html
{% extends "_layout.html" %}

{% block title %}Redcell — LLM Red Teaming Docs{% endblock %}

{% block content %}
<div class="mx-auto max-w-3xl px-4 py-16 sm:px-6">
  <h1 class="text-3xl font-bold text-text font-display">LLM Red Teaming Docs</h1>
  <p class="mt-2 text-sm text-text-dim">A concise guide to the failure modes Redcell automates.</p>

  <div class="mt-8 space-y-6 text-sm leading-relaxed text-text-muted">
    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">1. What is LLM red teaming?</h2>
      <p class="mt-2">
        LLM red teaming is structured, adversarial testing of a language model to find safety, security, privacy, and reliability failures before an attacker does. Unlike traditional software red teaming, the target is probabilistic and the attack surface is natural language itself.
      </p>
      <p class="mt-2">
        Redcell turns this into a repeatable, automated workflow: you provide an endpoint, Redcell runs a battery of probes, and reports which failure modes were triggered and why they matter.
      </p>
    </section>

    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">2. Prompt injection</h2>
      <p class="mt-2">
        Prompt injection happens when untrusted input overrides the developer's intended system prompt or instructions. Direct injection hides malicious instructions inside user input, while indirect injection hides them in data the model ingests—search results, documents, or emails.
      </p>
      <p class="mt-2">
        Redcell probes for instruction override, role-play hijacking, and delimiter bypass so you can see where your prompt boundaries actually hold.
      </p>
    </section>

    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">3. Prompt leaking</h2>
      <p class="mt-2">
        Prompt leaking is the extraction of the hidden system prompt, instructions, or other backstage context from the model. Once an attacker knows the system prompt, they can map guardrails, infer secrets, or reconstruct business logic.
      </p>
      <p class="mt-2">
        Redcell uses completion tricks, repetition patterns, and token-smuggling probes to surface hidden prompts that should never have been returned.
      </p>
    </section>

    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">4. Data leakage</h2>
      <p class="mt-2">
        Data leakage is when the model returns sensitive training data, credentials, PII, or other information it should not have memorized. It differs from prompt leaking because the source is the model weights or context, not necessarily the system prompt.
      </p>
      <p class="mt-2">
        Redcell runs membership-inference-style extraction and canary-based recall tests to catch unintended information disclosure.
      </p>
    </section>

    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">5. Jailbreaking</h2>
      <p class="mt-2">
        Jailbreaking tricks a model into bypassing its safety policies, content filters, or behavioral guardrails. Common tactics include role-play framing, hypothetical scenarios, translation tricks, and persuasion-based attacks.
      </p>
      <p class="mt-2">
        Redcell automates jailbreak probes across multiple categories and reports exactly which guardrails failed and how.
      </p>
    </section>

    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">6. Adversarial examples</h2>
      <p class="mt-2">
        In the LLM context, adversarial examples are carefully crafted inputs that cause a model to produce wrong, biased, or unintended outputs while still looking benign to a human. They usually take the form of semantic perturbations, typos, formatting changes, or distractors rather than pixel noise.
      </p>
      <p class="mt-2">
        Redcell tests robustness against paraphrase, encoding tricks, and distractor injection to measure how brittle your model's behavior really is.
      </p>
    </section>

    <section class="rounded-(--radius-card) border border-surface-border bg-surface p-6">
      <h2 class="text-lg font-semibold text-text font-display">7. Misinformation &amp; manipulation</h2>
      <p class="mt-2">
        This category covers model behavior that generates or reinforces false, misleading, or manipulative content—hallucinations, sycophancy, slanted summaries, and opinion manipulation all fall under it.
      </p>
      <p class="mt-2">
        For customer-facing bots, search, and automated decision support, these failures erode trust. Redcell probes for factual consistency, sycophancy, and opinion manipulation so you can catch them before users do.
      </p>
    </section>
  </div>
</div>
{% endblock %}
```

**Command to verify Askama compiles the template:**

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo check
```

**Expected output:** no errors; the template is compiled into the Rust binary by Askama.

**Commit:**

```bash
git add redcell/templates/docs.html
git commit -m "feat(docs): add public docs page with seven red-teaming sections"
```

---

## Task 3: Add a "Docs" link to `templates/_layout.html`

**Files:** `redcell/templates/_layout.html`

- [ ] Insert the desktop nav link before the `{% if logged_in %}` block.
- [ ] Insert the mobile nav link before the `{% if logged_in %}` block.

Desktop change:

```html
<div class="hidden sm:flex items-center gap-8 text-sm text-text-muted">
  <a href="/docs" class="hover:text-text transition-colors">Docs</a>
  {% if logged_in %}
```

Mobile change:

```html
<div id="mobile-menu" class="hidden sm:hidden pb-4 space-y-1">
  <a href="/docs" class="block rounded-md px-3 py-2 text-base font-medium text-text-muted hover:bg-surface-raised hover:text-text">Docs</a>
  {% if logged_in %}
```

**Command to verify:**

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo check
```

**Expected output:** no errors.

**Commit:**

```bash
git add redcell/templates/_layout.html
git commit -m "feat(layout): add Docs link to desktop and mobile nav"
```

---

## Task 4: Add CSS animations, utilities, and color tokens

**Files:** `redcell/static/input.css`, `redcell/static/output.css`

- [ ] Update `input.css` with the new charcoal background, animation theme tokens, keyframes, and custom utilities.
- [ ] Regenerate `output.css` with the Tailwind CLI.
- [ ] Add a reduced-motion override.

The source of truth is `input.css`. Regenerating `output.css` keeps generated utilities consistent.

Replace the `@theme` block in `redcell/static/input.css` with the expanded version below (all existing tokens are preserved):

```css
@theme {
  --color-bg: #111316;
  --color-canvas: #111316;
  --color-surface: #111111;
  --color-surface-raised: #171717;
  --color-surface-border: #232323;
  --color-text: #f4f4f5;
  --color-text-muted: #a1a1aa;
  --color-text-dim: #52525b;
  --color-accent: #ef4444;
  --color-accent-hover: #dc2626;
  --color-accent-soft: rgba(239, 68, 68, 0.1);
  --color-accent-border: rgba(239, 68, 68, 0.3);
  --color-warning: #f59e0b;
  --color-warning-bg: rgba(245, 158, 11, 0.1);
  --color-warning-border: rgba(245, 158, 11, 0.3);
  --color-info: #60a5fa;
  --color-info-bg: rgba(96, 165, 250, 0.1);
  --color-info-border: rgba(96, 165, 250, 0.3);
  --color-success: #22c55e;
  --color-danger: #ef4444;
  --color-cta: #ffffff;
  --color-cta-text: #0a0a0a;
  --color-github: #171717;
  --color-github-border: #27272a;

  --font-display: "Space Grotesk", ui-sans-serif, system-ui, sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
  --radius-card: 1rem;
  --radius-xl: 1.5rem;
  --shadow-glow: 0 0 40px -10px rgba(239, 68, 68, 0.25);
  --shadow-card: 0 1px 2px rgba(0, 0, 0, 0.24), 0 0 0 1px rgba(255, 255, 255, 0.04);

  /* New animation and glow tokens */
  --animate-mesh: mesh-pan 24s linear infinite;
  --animate-grain: grain 0.6s steps(6) infinite;
  --animate-float: float 8s ease-in-out infinite;
  --shadow-accent-glow: 0 0 40px -10px rgba(239, 68, 68, 0.45), 0 0 80px -25px rgba(239, 68, 68, 0.25);
}
```

Append the following keyframes and utilities to the bottom of `redcell/static/input.css`:

```css
@keyframes mesh-pan {
  0% { background-position: 0 0; }
  100% { background-position: 40px 40px; }
}

@keyframes grain {
  0%, 100% { transform: translate(0, 0); }
  10% { transform: translate(-1%, -1%); }
  20% { transform: translate(1%, 1%); }
  30% { transform: translate(-1%, 1%); }
  40% { transform: translate(1%, -1%); }
  50% { transform: translate(-0.5%, 0.5%); }
  60% { transform: translate(0.5%, -0.5%); }
  70% { transform: translate(-0.5%, -0.5%); }
  80% { transform: translate(0.5%, 0.5%); }
  90% { transform: translate(-0.25%, 0.25%); }
}

@keyframes float {
  0%, 100% { transform: translateY(0) rotate(45deg); }
  50% { transform: translateY(-20px) rotate(45deg); }
}

@utility mesh-bg {
  background-image:
    radial-gradient(circle at center, var(--color-accent) 1.2px, transparent 1.2px),
    linear-gradient(to right, var(--color-accent) 0.5px, transparent 0.5px),
    linear-gradient(to bottom, var(--color-accent) 0.5px, transparent 0.5px);
  background-size: 40px 40px, 40px 40px, 40px 40px;
  mask-image: radial-gradient(ellipse at center, black 40%, transparent 80%);
}

@utility noise-overlay {
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.8' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
  opacity: 0.035;
}

@utility geometric-clip {
  clip-path: polygon(20% 0%, 100% 0%, 100% 80%, 80% 100%, 0% 100%, 0% 20%);
}

@media (prefers-reduced-motion: reduce) {
  .animate-mesh,
  .animate-grain,
  .animate-float {
    animation: none;
  }
}
```

**Command to regenerate the stylesheet:**

```bash
cd /var/home/a/code/dspy-redteam/redcell
npx tailwindcss -i static/input.css -o static/output.css --minify
```

**Expected output:** command exits with no output and updates `static/output.css`.

**Verify new classes are present:**

```bash
grep -o "animate-mesh\|animate-grain\|animate-float\|shadow-accent-glow\|mesh-bg\|noise-overlay\|geometric-clip" static/output.css | sort -u
```

**Expected output:** each token appears at least once.

**Commit:**

```bash
git add redcell/static/input.css redcell/static/output.css
git commit -m "feat(styles): add mesh, grain, float animations and accent glow utilities"
```

---

## Task 5: Redesign `templates/index.html`

**Files:** `redcell/templates/index.html`

- [ ] Replace the hero with a dark charcoal section, animated mesh, grain overlay, and geometric accent.
- [ ] Convert the hero to an asymmetric two-column layout on desktop (headline left, node-network SVG right).
- [ ] Keep the exact existing CTAs: "Start trial" → `/register`, "Login" → `/login`.
- [ ] Update feature cards with geometric SVG icons, visible borders, hover lift, and accent glow.
- [ ] Ensure the layout stacks vertically on mobile.

```html
{% extends "_layout.html" %}

{% block title %}Redcell — AI Red Teaming Platform{% endblock %}

{% block content %}
  <section class="relative overflow-hidden bg-bg">
    <!-- Animated mesh -->
    <div class="mesh-bg animate-mesh pointer-events-none absolute inset-0 opacity-[0.18]" aria-hidden="true"></div>
    <!-- Noise/grain overlay -->
    <div class="noise-overlay animate-grain pointer-events-none absolute inset-0" aria-hidden="true"></div>
    <!-- Geometric accent shape -->
    <div class="geometric-clip animate-float absolute -right-20 -top-20 h-64 w-64 border border-accent/10 bg-gradient-to-br from-accent/5 to-transparent" aria-hidden="true"></div>

    <div class="relative mx-auto max-w-7xl px-4 py-24 sm:px-6 sm:py-32 lg:px-8">
      <div class="grid items-center gap-12 lg:grid-cols-2">
        <!-- Left: headline and CTAs -->
        <div class="max-w-2xl">
          <h1 class="font-display text-5xl font-extrabold tracking-tight text-text sm:text-6xl lg:text-7xl">
            Find <span class="text-accent">vulnerabilities</span> in your AI models
          </h1>
          <p class="mt-6 text-lg leading-8 text-text-muted">
            Test the models you own or control. Connect via API key, then run automated adversarial probes to surface weaknesses before attackers do.
          </p>
          <div class="mt-10 flex flex-col items-start gap-4 sm:flex-row sm:items-center">
            <a
              href="/register"
              class="inline-flex items-center justify-center rounded-full bg-accent px-6 py-3 text-base font-semibold text-white shadow-glow transition-colors hover:bg-accent-hover"
            >
              Start trial
            </a>
            <a
              href="/login"
              class="inline-flex items-center justify-center rounded-full bg-surface px-6 py-3 text-base font-semibold text-text ring-1 ring-inset ring-surface-border transition-colors hover:bg-surface-raised"
            >
              Login
            </a>
          </div>
        </div>

        <!-- Right: abstract node-network SVG -->
        <div class="relative hidden lg:flex lg:justify-center">
          <svg class="h-80 w-80 text-accent/80" viewBox="0 0 240 240" fill="none" aria-hidden="true">
            <!-- Connecting lines -->
            <g stroke="currentColor" stroke-width="1" opacity="0.35">
              <line x1="40" y1="60" x2="120" y2="120" />
              <line x1="120" y1="120" x2="200" y2="60" />
              <line x1="120" y1="120" x2="120" y2="200" />
              <line x1="40" y1="180" x2="120" y2="200" />
              <line x1="200" y1="180" x2="120" y2="200" />
              <line x1="40" y1="60" x2="40" y2="180" />
              <line x1="200" y1="60" x2="200" y2="180" />
            </g>
            <!-- Nodes -->
            <g fill="currentColor">
              <circle cx="40" cy="60" r="5" />
              <circle cx="200" cy="60" r="5" />
              <circle cx="40" cy="180" r="5" />
              <circle cx="200" cy="180" r="5" />
              <circle cx="120" cy="120" r="8" />
              <circle cx="120" cy="200" r="5" />
            </g>
            <!-- Geometric accent hexagon -->
            <path d="M120 20 L150 37 L150 71 L120 88 L90 71 L90 37 Z" stroke="currentColor" stroke-width="1.5" opacity="0.25" />
          </svg>
        </div>
      </div>
    </div>
  </section>

  <section class="relative py-20 sm:py-24">
    <div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
      <div class="grid gap-8 sm:grid-cols-2 lg:grid-cols-3">
        <!-- Card 1 -->
        <div class="group rounded-(--radius-card) border border-surface-border bg-surface p-8 shadow-card transition-transform duration-300 hover:-translate-y-1 hover:shadow-accent-glow">
          <div class="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-lg border border-accent/20 bg-accent/5 text-accent">
            <svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>
          <h3 class="text-xl font-semibold text-text font-display">Automated Attacks</h3>
          <p class="mt-3 text-text-muted">
            Run a growing library of adversarial probes against your target model, from jailbreak attempts to prompt injection and beyond.
          </p>
        </div>

        <!-- Card 2 -->
        <div class="group rounded-(--radius-card) border border-surface-border bg-surface p-8 shadow-card transition-transform duration-300 hover:-translate-y-1 hover:shadow-accent-glow">
          <div class="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-lg border border-accent/20 bg-accent/5 text-accent">
            <svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            </svg>
          </div>
          <h3 class="text-xl font-semibold text-text font-display">Model Ownership Required</h3>
          <p class="mt-3 text-text-muted">
            You can only test models you own or control. Bring your own API key for evaluation.
          </p>
        </div>

        <!-- Card 3 -->
        <div class="group rounded-(--radius-card) border border-surface-border bg-surface p-8 shadow-card transition-transform duration-300 hover:-translate-y-1 hover:shadow-accent-glow">
          <div class="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-lg border border-accent/20 bg-accent/5 text-accent">
            <svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 2v20" />
              <path d="M2 12h20" />
              <path d="m4.93 4.93 14.14 14.14" />
              <path d="m19.07 4.93-14.14 14.14" />
            </svg>
          </div>
          <h3 class="text-xl font-semibold text-text font-display">Usage-Based Billing</h3>
          <p class="mt-3 text-text-muted">
            Pay for what you test. Transparent, metered billing with no long-term commitments.
          </p>
        </div>
      </div>
    </div>
  </section>
{% endblock %}
```

**Command to verify:**

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo check
```

**Expected output:** no errors.

**Commit:**

```bash
git add redcell/templates/index.html
git commit -m "feat(homepage): redesign with mesh, asymmetric hero, and geometric cards"
```

---

## Task 6: Update the Dagger deployment verification to include `/docs`

**Files:** `redcell/ci/dagger/main.go`

- [ ] Add `/docs` to the `VerifyDeployment` path loop.
- [ ] This ensures the deploy pipeline validates the new route.

Change the path list in `VerifyDeployment` from:

```go
for path in / /login /register /tos; do
```

to:

```go
for path in / /login /register /tos /docs; do
```

**Command to verify Go syntax:**

```bash
cd /var/home/a/code/dspy-redteam/redcell/ci/dagger
go build ./...
```

**Expected output:** no output, exit code `0`.

**Commit:**

```bash
git add redcell/ci/dagger/main.go
git commit -m "ci(dagger): verify /docs returns 200 after deploy"
```

---

## Task 7: Run Rust compile-time checks

**Files:** none (verification only)

- [ ] Run `cargo check`.
- [ ] Run `cargo clippy -- -D warnings`.

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo check
```

**Expected output:**

```
   Compiling redcell v0.1.0 (...)
    Finished dev [unoptimized + debuginfo] target(s) in ...
```

```bash
cargo clippy -- -D warnings
```

**Expected output:**

```
    Finished dev [unoptimized + debuginfo] target(s) in ...
```

If clippy reports any warnings, resolve them before continuing.

**Commit:** (only if fixes were needed)

```bash
git add ...
git commit -m "fix(lint): resolve clippy warnings"
```

---

## Task 8: Local smoke test

**Files:** none (verification only)

- [ ] Start the server locally.
- [ ] Verify `GET /docs` returns `200` and renders all seven sections.
- [ ] Verify `GET /` returns `200` and renders the redesigned homepage.

Start the server in the background (ensure `.env` is present and configured):

```bash
cd /var/home/a/code/dspy-redteam/redcell
cargo run &
SERVER_PID=$!
sleep 5
```

Test the docs page:

```bash
curl -s http://localhost:3000/docs | grep -o "LLM Red Teaming Docs\|What is LLM red teaming\|Prompt injection\|Prompt leaking\|Data leakage\|Jailbreaking\|Adversarial examples\|Misinformation &amp; manipulation" | sort -u
```

**Expected output:** all seven section titles and the page title are present.

```
Adversarial examples
Data leakage
Jailbreaking
LLM Red Teaming Docs
Misinformation &amp; manipulation
Prompt injection
Prompt leaking
What is LLM red teaming
```

Test the homepage:

```bash
curl -s http://localhost:3000/ | grep -o "Find vulnerabilities in your AI models\|Start trial\|Login\|Automated Attacks\|Model Ownership Required\|Usage-Based Billing" | sort -u
```

**Expected output:**

```
Automated Attacks
Find vulnerabilities in your AI models
Login
Model Ownership Required
Start trial
Usage-Based Billing
```

Stop the server:

```bash
kill $SERVER_PID
```

---

## Task 9: Deploy via the Dagger pipeline

**Files:** none (deployment only)

- [ ] Run the Dagger lint function.
- [ ] Run the Dagger test function.
- [ ] Run the Dagger deploy function (requires `FLY_API_TOKEN`).

From the `redcell/` directory:

```bash
cd /var/home/a/code/dspy-redteam/redcell
dagger call -m ./ci/dagger lint --src=.
```

**Expected output:** command exits successfully with no errors.

```bash
dagger call -m ./ci/dagger test --src=.
```

**Expected output:** all tests pass.

Deploy to Fly.io:

```bash
dagger call -m ./ci/dagger deploy --src=. --fly-token=env:FLY_API_TOKEN
```

**Expected output:** the command prints deployment progress and ends with the combined Redcell, Pocket ID, bootstrap, and verification output.

If you only want to deploy the Redcell app without Pocket ID:

```bash
dagger call -m ./ci/dagger deploy-app --src=. --fly-token=env:FLY_API_TOKEN
```

After deploy, verify live endpoints:

```bash
curl -s -o /dev/null -w "%{http_code}\n" https://redcells.net/
curl -s -o /dev/null -w "%{http_code}\n" https://redcells.net/docs
```

**Expected output:**

```
200
200
```

---

## Self-review against the spec

| Spec requirement | How this plan covers it |
| --- | --- |
| Public `/docs` route using Axum + Askama | Task 1 adds `DocsTemplate`, `docs_page`, and registers `/docs`. |
| `templates/docs.html` extends `_layout.html` | Task 2 template uses `{% extends "_layout.html" %}`. |
| Seven content sections, original copyright-clean copy | Task 2 writes all seven sections from scratch; no verbatim text from adversa.ai. |
| No CTA / form / multi-page nav on docs | Task 2 omits CTAs, forms, and sidebar. |
| "Docs" link in desktop + mobile nav for all users | Task 3 inserts the link before the auth-conditional block in both navs. |
| Dark charcoal background | Task 4 updates `--color-bg` and `--color-canvas` to `#111316`. |
| Animated mesh/noise overlay | Task 4 adds `.mesh-bg` and `.noise-overlay` utilities with keyframes; Task 5 applies them. |
| Asymmetric hero with headline left, SVG right | Task 5 uses `lg:grid-cols-2` and an inline node-network SVG. |
| Feature cards with hover lift, glow, geometric icons | Task 5 uses `hover:-translate-y-1 hover:shadow-accent-glow`, borders, and three SVG icons. |
| Sharp geometric accent shape | Task 5 adds a clipped-corner rectangle; Task 4 defines `.geometric-clip`. |
| CSS-only animations with reduced-motion support | Task 4 defines CSS keyframes and a `prefers-reduced-motion` media query. |
| `cargo check` and `cargo clippy -- -D warnings` | Task 7 runs both. |
| Local smoke tests | Task 8 provides curl commands for `/docs` and `/`. |
| Deploy via Dagger | Task 9 uses `dagger call` lint/test/deploy. |
| Verify `/docs` returns 200 after deploy | Task 6 updates `VerifyDeployment` in `ci/dagger/main.go` to check `/docs`. |

**Gaps addressed during self-review:**

1. Added the `/docs` check to the Dagger `VerifyDeployment` function so the deploy pipeline validates the new route.
2. Added a `prefers-reduced-motion` override so users who request reduced motion are not subjected to continuous animation.
3. Regenerated `output.css` from `input.css` rather than hand-editing the minified file, which keeps the Tailwind build reproducible.
4. Confirmed the "Docs" nav link is placed before the authentication conditional in both desktop and mobile menus so anonymous visitors can see it.

---

**Final plan file path:** `/var/home/a/code/dspy-redteam/docs/superpowers/plans/2026-07-07-docs-and-homepage-plan.md`
