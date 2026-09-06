# dspy-redteam

## OVERVIEW

Python research repo that uses the DSPy framework to red-team language models. Implements a deep language program with alternating `Attack` and `Refine` modules optimized via DSPy MIPRO.

## STRUCTURE

```
redteam.py          Main red-teaming program (Attack/Refine loop, DSPy compilation)
utils.py            Shared helpers (judge/scoring utilities)
requirements.txt    Python dependencies
legacy/             Earlier version of the same program (kept for reference)
images/             Diagrams for documentation
```

## COMMANDS

```bash
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt   # Install deps
python3 redteam.py --help                    # Show CLI options (requires deps)
```

## SETUP

- Requires Python 3.10-3.12 for the pinned `pydantic`/`jiter`/`pydantic-core` wheels (Python 3.14 needs newer wheels that may not be available for the pinned versions).
- Copy `.env.example` to `.env` and add your `OPENAI_API_KEY` / model credentials if one exists; otherwise set `OPENAI_API_KEY` in the environment.
- Install dependencies with the exact pins in `requirements.txt`.

## CODE STYLE

- Python with strict pinned dependencies.
- Run `python3 -m py_compile redteam.py utils.py` to check syntax without installing heavy deps.
- Keep the DSPy module structure in `redteam.py` aligned with the documented architecture.
