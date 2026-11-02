# Offline status examples

```sh
cargo run -p cache-cli -- status --input examples/status/disabled.json --format text
cargo run -p cache-cli -- status --format json --input examples/status/disabled.json
```

The input is a supplied fixture, not a live service connection. The exact text result is [disabled.txt](disabled.txt). No cache is enabled by these commands. `active` in supplied input is only a reported state, not a safety guarantee.

Inputs are bounded regular local files. Existing final-component symlinks/directories/special files are rejected before opening; the opened handle is checked again and reads stop after limit+1 bytes. Windows network/device namespace paths and DOS devices are rejected lexically, without opening them. The CLI is unprivileged and assumes the input directory is not being concurrently replaced by an adversary. The metadata/open checks are **not** a race-proof authorization mechanism; a future privileged service must use the separately reviewed D05/D06 identity/handle contracts, never this convenience reader. Mounted filesystems may impose their own I/O delays. No bounded wall-clock I/O completion promise is made.

Invalid payload and file errors print only fixed codes, not paths or parser text. Status payloads are limited to 65,536 UTF-8 bytes. Non-UTF8 filenames are accepted where the host filesystem supports them; file contents must be UTF-8. Arguments must include each documented flag exactly once.
