# Native core adapters

Built-ins and their authors are in `config/cores.json`. Custom adapters live in
private `core-adapters/*.json`; A on Cores imports one after validation.
An adapter cannot replace a built-in ID. Example:

```json
{
  "id": "customcore", "name": "Example core", "author": "Upstream authors",
  "source": "https://example.com/project", "license": "See upstream LICENSE",
  "executable": "example.exe", "format": "json", "default_port": 17900,
  "start_args": ["run", "-c", "{config}"],
  "validate_args": ["check", "-c", "{config}"],
  "tun": false, "process_rules": false
}
```

Placeholders: `{config}` selected native file; `{data}` private core-data/ID;
`{root}` selected runtime. Arguments are expanded and Windows-quoted as separate
arguments to the executable, without a shell. IDs: lowercase, 2–32 characters;
only a basename `.exe` is accepted. Metadata must include the author/license.
B on Cores copies a trusted local PE executable to tools/ID; it does not fetch or
run it. Supply DLLs and license files beside it. Built-ins use pinned SHA-256 downloads.

Managed YAML profiles require a top-level mixed-port. JSON adapters require a
loopback `http`/`mixed` inbound using type/listen_port (sing-box shape) or
protocol/port (Xray shape). This supports HTTP diagnostics and Windows Proxy.
Other vendor shapes need an adapter implementation, not an unverified promise.
Custom adapters are native-profile only; generic generation is defined for the
three built-ins. Capability flags describe generation, not all vendor features.
The universal label refers to this extension contract and editable routing library.

Do not import adapters or execute binaries from untrusted sources. Their own
native config validation can execute vendor functionality and make network calls.
URTW does not sandbox third-party executables. It controls only its recorded
PID/path/creation-time, leaves external instances alone, and keeps private configs
out of logs shown by the dashboard.
