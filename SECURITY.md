# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.x     | :white_check_mark: |

## Scope

This security policy covers the following components of the tokenless project:

- **tokenless binary** -- the CLI application distributed via crates.io and GitHub Releases
- **tokenless-schema crate** -- schema and response compression library
- **tokenless-stats crate** -- statistics recording library
- **MCP server** -- the built-in JSON-RPC 2.0 server for Model Context Protocol integration
- **TOON encoding/decoding** -- compression format for additional token savings

Dependencies and transitive dependencies are not covered by this policy; please report
vulnerabilities in upstream projects directly to their maintainers.

## Reporting a Vulnerability

If you discover a security vulnerability in tokenless, please report it by
opening a GitHub issue with the `security` tag:

<https://github.com/TokenFleet-AI/tokenless/issues/new>

When opening a security issue, please include:

1. A clear description of the vulnerability
2. Steps to reproduce (minimal example, if possible)
3. Affected version(s)
4. Potential impact
5. Any suggested mitigation or fix (optional)

Vulnerability reports will be acknowledged within 5 business days. We aim to
provide an initial assessment and remediation timeline within 10 business days.

## Security Considerations

- Tokenless processes command-line input and may record statistics to a local
  SQLite database. Review recorded data periodically for sensitive information.
- The MCP server should only be exposed to trusted clients (stdin/stdout
  communication is inherently local).
- When using the RTK (Rust Token Killer) proxy, commands are rewritten locally
  and never sent to external services.
