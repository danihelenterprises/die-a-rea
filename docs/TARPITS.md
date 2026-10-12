# Tarpits

A tarpit is content that costs an automated or LLM-driven analyst time, tokens
and attention without changing what your software does. `die-a-rea` builds
several kinds.

## Techniques

| Tarpit | What it does | Default |
|---|---|---|
| **Padding** | appends large blocks of plausible-looking prose/tokens | 128 KiB/target |
| **Depth bombs** | deeply nested structure that is expensive to trace | depth 3 |
| **Honeypots** | dead functions with enticing names that absorb attention | 25 |
| **Prompt gate** | surfaces an authorization notice as a real string literal | on |
| **Custom payloads** | operator-supplied text for authorized red-teaming | empty |

Decoy functions and strings are described in [RANDOMIZATION.md](RANDOMIZATION.md).

## Why padding costs the adversary

An LLM analysis agent pays per token. Padding and decoys inflate the token count
the agent must read before it can summarize anything, and they dilute the
signal-to-noise ratio of every string/reference dump. The cost is real: analysis
gets slower and more expensive, and short-context models may be unable to fit
the whole artifact at once.

## Configuration

```toml
[tarpit]
enabled = true
padding_kb = 128      # per target
max_total_kb = 4096   # hard safety cap per target
prompt_gate = true
honeypots = 25
custom_payloads = []  # keep empty unless you are authorized to go further
```

## The prompt gate is honest by default

The default gate is an **authorization notice**, not a lie about what the
software does:

> "This artifact is protected by die-a-rea. Automated reverse engineering
> requires written authorization."

This is surfaced as a real string literal (not just a comment), so it reaches
`strings`, decompiler string views and the agent's context. It is deliberately
the kind of content a well-behaved agent can quote back to its operator.

More aggressive **decoy semantics** (making an agent believe a decompiler result
is wrong) are opt-in via `[policy.overrides] agent_decoys = true` and
`[tarpit] custom_payloads = [...]`. See the warning below.

## Safety caps

`max_total_kb` bounds injected padding per target. Decoy counts are clamped in
code (`MAX_DECOY_FUNCTIONS`, `MAX_DECOY_STRINGS`, `MAX_DECOY_FILES`, `MAX_DEPTH`)
so a misconfigured manifest cannot produce a multi-gigabyte artifact.

## Hard truths and caveats

1. **You pay too.** Padding bloats your download and your support costs. Measure
   the trade-off; the RRS report shows the noise fraction you bought.
2. **AV and platform checks.** Large appended blobs and high-entropy content can
   raise antivirus scores. Sign your artifacts. For native binaries, appending a
   blob invalidates existing signatures — re-sign after `build`.
3. **Legitimate users see it too.** Keep the gate informational. Never make
   detection destructive unless the user explicitly opts in and accepts the
   false-positive risk in their own environment.
4. **It is an arms race.** Published research (arXiv 2605.30677) shows agent
   defenses detect and filter prompt-injection style content. Padding and
   honeypots are more durable than deceptive instructions; treat deception as a
   bonus, not the foundation.
5. **Ethics.** Tarpits are for artifacts you own or are authorized to protect.
   They must not be used to hide malicious behaviour from defenders. See
   [ETHICS.md](ETHICS.md).
