# Security policy

Parler handles your microphone, your API keys and the text it types into
other apps, so we take security reports seriously. Thank you for helping keep
it safe.

## Reporting a vulnerability

**Please don't open a public issue.** Email **[parler@synapsr.io](mailto:parler@synapsr.io)**
with `Security` in the subject.

Helpful details:

- The Parler version (shown at the bottom of the sidebar) or the commit you built
  from, and your operating system and version.
- What an attacker could do, and what they need to do it.
- Steps to reproduce, or a proof of concept.
- Whether you've shared the issue with anyone else.

Never include a real API key in a report. If logs help, check them for anything
private before sending them.

## What happens next

Parler is maintained by a small team, so we can't promise fixed deadlines.
We'll do our best to:

- acknowledge your report within a week;
- tell you whether we can reproduce it, and keep you posted while we fix it;
- publish the fix in a new release and credit you in its notes, if you'd like.

Please give us a reasonable time to ship a fix before disclosing the issue
publicly.

## Supported versions

Security fixes go into the latest release only. Before the first release,
they go into the `main` branch.

## Scope

In scope: the code in this repository, including how API keys, settings and
history are stored, the requests sent to speech-to-text providers, the paste
mechanism, the overlay window, and the release workflow and installers it
produces.

Out of scope, please report them to the right place:

- The speech-to-text services themselves (Groq, OpenAI, Fireworks, or a server
  you host).
- [`handy-recorder`](https://crates.io/crates/handy-recorder) and other
  dependencies: report to their maintainers. Tell us too if Parler is
  affected.
- Attacks that require an already compromised computer or user account.
