# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Serpentine, please report it responsibly rather than opening a public issue.

Open a [GitHub Security Advisory](https://github.com/serpentine-parser/serpentine/security/advisories/new) on this repository. This keeps the details private until a fix is available.

Please include:

- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof-of-concept
- Any suggested mitigations if you have them

You can expect an acknowledgment within a few days and a resolution or status update within two weeks.

## Scope

Serpentine is a local analysis tool — it reads source files from your filesystem and serves a web UI on localhost. It does not transmit data externally, require authentication, or handle user-supplied credentials.

Relevant security concerns include:

- **Path traversal**: The analyzer or server inadvertently reading files outside the specified project directory
- **Arbitrary code execution**: Maliciously crafted source files causing the parser to execute code
- **WebSocket abuse**: The local WebSocket endpoint being exploitable in unexpected ways
