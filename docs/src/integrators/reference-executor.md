# Reference local executor

PostProject includes an opt-in local worker for proxy and thumbnail jobs. It
runs a separately installed `ffmpeg` executable; PostProject does not link,
bundle, or download FFmpeg. A missing executable is a capability gap and leaves
an unclaimed job in `requested` state.

The executor is deliberately small:

| Job kind | Named profiles | Requested output kind |
| --- | --- | --- |
| `org.postproject:generate-proxy` | `proxy-720p`, `proxy-1080p` | normally `proxy` |
| `org.postproject:generate-thumbnail` | `thumbnail-640`, `thumbnail-1280` | normally `derived` |

Record the profile with the job as a string metadata value whose vocabulary is
`https://postproject.org/ns/executor-parameters/1` and whose property is
`profile`. `job request --profile` does this for the CLI. The job must have one
input representation, that input must be a single file on the requested output
asset, and the requested output must name a logical target root. Supply this
machine's directory for that root to `job run --root-map NAME=PATH`.

```{code-variants} reference-executor
:::{no-variant} c
The C API exposes the complete job worker protocol, but not the optional local
executor. A C host can run its own process and complete the job through that
protocol.
:::
:::{no-variant} cpp
The C++ wrapper exposes the complete job worker protocol, but not the optional
local executor.
:::
:::{no-variant} python
The Python binding exposes the complete job worker protocol, but not the
optional local executor.
:::
```

`job run --once` processes one eligible request. Without `--once`, the command
processes the eligible requested jobs present at startup. It does not remain as
a daemon and never creates work implicitly.

The runner probes `ffmpeg` before claiming. During execution it renews its lease
through the normal public transaction contract. A successful subprocess writes
to an execution-specific temporary path and is renamed into its own final path
before the runner atomically records the fingerprinted representation,
activity, input/output
snapshots, copied parameters, and succeeded job state. A tool failure removes
the temporary output, records a bounded diagnostic, and creates no
representation or activity.

The executable, process timeout, lease duration, and machine root mapping are
all explicit CLI options. Profiles are stable data recorded verbatim on the
activity, so a regeneration plan can recover the same request.
