# Responsible use

`die-a-rea` is a **defensive** tool for people protecting software they own or
are authorized to protect. By using it you agree to the following.

## Intended use

- Protecting your own commercial or proprietary software from unauthorized
  automated analysis.
- Protecting software you have written authorization to harden (e.g. as a
  contractor or on behalf of a client).
- Security research and education on your own artifacts, or on artifacts you
  are legally permitted to analyze.
- Adding traceable canaries so you can detect when your code leaks into
  third-party models or products.

## Prohibited use

- Hiding malware, ransomware, spyware, or other malicious behaviour from
  defenders, sandboxes, AV, or analysts.
- Evading law-enforcement or incident-response analysis of systems you do not
  own.
- Circumventing technical protection measures in ways that violate applicable
  law (e.g. anti-circumvention statutes), or violating software licenses.

## Hard truths

1. **Nothing here is absolute.** Anti-debug, anti-instrumentation and prompt
   injection are all bypassable. This tool raises cost and defeats automation;
   it does not defeat a determined human.
2. **The agent layer is an arms race.** Published research already shows that
   LLM-analysis prompt injections can be detected and that detectors can be
   evaded. Ship with honesty (notice + canaries) by default and treat decoys as
   experimental.
3. **Runtime guards can misfire.** Debuggers, profilers, crash reporters, CI
   sandboxes, containers, and some antivirus products look like instrumentation.
   Default to non-destructive behaviour (notice/degrade), make it configurable,
   and test against your legitimate support environments.
4. **Antivirus may flag your build.** Obfuscation, packing and large appended
   tarpit blobs raise AV scores. Test, sign your binaries, and maintain a
   clean-hash pipeline.
5. **Tarpits and decoys cut both ways.** The default gate is an honest
   authorization notice. Deceptive payloads that make an analysis agent report
   the wrong behaviour are opt-in and experimental, and must never be used to
   mislead defenders about malicious functionality. All padding and decoys cost
   you bandwidth and legitimacy; they are not free.

## Legal note

Laws differ by jurisdiction. Anti-circumvention rules (e.g. DMCA §1201) and
software license terms can restrict what you may do even to software you possess.
Interoperability, security research and preservation may have specific
exemptions. This document is not legal advice; consult counsel for your case.
