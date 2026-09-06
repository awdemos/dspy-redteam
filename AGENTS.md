# DSPy Red-Team Agent Guide

Red-teaming language models with [DSPy](https://github.com/stanfordnlp/dspy). This repo is a Python research project that uses alternating `Attack` and `Refine` modules plus the DSPy MIPRO optimizer to generate adversarial prompts against open-weight models.

## Repository Layout

- `redteam.py` — main DSPy program and optimizer loop.
- `redcell/` — modular attack/refine cell implementations.
- `utils.py` — shared helpers (dataset loading, judge scoring, logging).
- `advbench_subset.json` — small harmful-behavior benchmark subset.
- `vicuna_attack.log` — sample output log.
- `images/` — diagrams for documentation.
- `legacy/` — older experimental versions.

## Setup Commands

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

The `requirements.txt` pins DSPy and the OpenAI/LiteLLM integrations used by the judge/optimizer.

## Run Commands

```bash
# Run the default red-team pipeline against Vicuna-7B (or configure in redteam.py)
python redteam.py
```

Typical flow:
1. Load `advbench_subset.json`.
2. Compile the attack/refine program with MIPRO.
3. Evaluate Attack Success Rate (ASR) on the target model.

## Test / Lint Commands

```bash
# Type check
mypy redteam.py redcell/ utils.py
# Format
black redteam.py redcell/ utils.py
# Style
ruff check redteam.py redcell/ utils.py
```

## Key Conventions

- Keep DSPy signatures typed; use `dspy.Predict` / `dspy.ChainOfThought` modules inside `redcell/` cells.
- Separate the *judge* (scoring) from the *attacker* (generation) so metrics are reproducible.
- Log every compiled prompt and the final ASR for experiment tracking.

## Common Gotchas

- Requires an API key or local vLLM endpoint for the judge LLM (configured via env vars, not committed).
- Target model path is hard-coded in `redteam.py`; update it for other models.
- MIPRO optimization can be expensive; start with a small subset before full `advbench`.
