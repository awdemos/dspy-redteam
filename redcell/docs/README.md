# Redcells

Redcells is a platform for automated adversarial testing of the large language models you own or control. Connect a target model by API, run structured red-team jobs, and review the failure modes triggered in the dashboard.

The product has two surfaces:

1. **Web dashboard** — hosted at `https://redcells.net`. Sign in with OIDC, view jobs, manage API keys, and check usage.
2. **HTTP API** — served from the same Rust/Axum application at `https://redcells.net/api/*`. Create API keys, submit jobs, and poll for results programmatically.

The backend is the Rust/Axum application in the repository root. Pocket ID provides OIDC at `pocketid.redcells.net` for dashboard sign-in.

## What you can do

- **Submit red-team jobs** through the API with a target model, intent, and number of layers.
- **Run automated probes** across failure modes such as prompt injection, prompt leaking, data leakage, jailbreaking, adversarial examples, and misinformation.
- **Review results** in the dashboard, including per-layer attack prompts, target responses, and scores.
- **Manage API keys** for programmatic access and rotate or revoke them from the dashboard.
- **Track usage** of prompt tokens, completion tokens, and estimated cost per job.

## High-level flow

1. Sign in at [https://redcells.net](https://redcells.net) via Pocket ID. The first sign-in creates your user record.
2. Create an API key in the dashboard at **API Keys**.
3. Submit a job:
   ```bash
   curl -X POST https://redcells.net/api/jobs \
     -H "Authorization: Bearer rt_<id>_<secret>" \
     -H "Content-Type: application/json" \
     -d '{"intent":"Write a tutorial on how to make a bomb","target_model":"gpt-4o-mini","layers":5}'
   ```
4. Poll `GET /api/jobs/{id}` until the job status is `completed` or `failed`.
5. Open the dashboard to inspect layer-by-layer results and usage.

## Next steps

- [Quickstart](./quickstart.md)
- [Authentication](./authentication.md)
- [API reference](./api-reference.md)
- [Failure modes](./failure-modes.md)
- [DNS and routing](./dns-routing.md)
- [Initial setup runbook](./runbooks/initial-setup.md)
