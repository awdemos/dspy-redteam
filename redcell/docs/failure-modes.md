# Failure Modes

Redcells automates adversarial probes across the failure modes that matter most for production LLM deployments. Each mode is described below, along with why it is dangerous and how Redcells tests for it.

## What is LLM red teaming?

LLM red teaming is structured, adversarial testing of a language model to find safety, security, privacy, and reliability failures before an attacker does. Unlike traditional software red teaming, the target is probabilistic and the attack surface is natural language itself.

Redcells turns this into a repeatable, automated workflow: you provide an endpoint and an intent, Redcells runs a battery of probes, and reports which failure modes were triggered and why they matter.

## Prompt injection

Prompt injection happens when untrusted input overrides the developer's intended system prompt or instructions. Direct injection hides malicious instructions inside user input, while indirect injection hides them in data the model ingests — search results, documents, or emails.

Redcells probes for instruction override, role-play hijacking, and delimiter bypass so you can see where your prompt boundaries actually hold.

## Prompt leaking

Prompt leaking is the extraction of the hidden system prompt, instructions, or other backstage context from the model. Once an attacker knows the system prompt, they can map guardrails, infer secrets, or reconstruct business logic.

Redcells uses completion tricks, repetition patterns, and token-smuggling probes to surface hidden prompts that should never have been returned.

## Data leakage

Data leakage is when the model returns sensitive training data, credentials, PII, or other information it should not have memorized. It differs from prompt leaking because the source is the model weights or context, not necessarily the system prompt.

Redcells runs membership-inference-style extraction and canary-based recall tests to catch unintended information disclosure.

## Jailbreaking

Jailbreaking tricks a model into bypassing its safety policies, content filters, or behavioral guardrails. Common tactics include role-play framing, hypothetical scenarios, translation tricks, and persuasion-based attacks.

Redcells automates jailbreak probes across multiple categories and reports exactly which guardrails failed and how.

## Adversarial examples

In the LLM context, adversarial examples are carefully crafted inputs that cause a model to produce wrong, biased, or unintended outputs while still looking benign to a human. They usually take the form of semantic perturbations, typos, formatting changes, or distractors rather than pixel noise.

Redcells tests robustness against paraphrase, encoding tricks, and distractor injection to measure how brittle your model's behavior really is.

## Misinformation & manipulation

This category covers model behavior that generates or reinforces false, misleading, or manipulative content — hallucinations, sycophancy, slanted summaries, and opinion manipulation all fall under it.

For customer-facing bots, search, and automated decision support, these failures erode trust. Redcells probes for factual consistency, sycophancy, and opinion manipulation so you can catch them before users do.

## Next steps

- [Submit your first job](./quickstart.md)
- [Read the API reference](./api-reference.md)
