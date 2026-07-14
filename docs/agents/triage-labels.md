# Triage labels

| Canonical role | GitHub label | Meaning |
| --- | --- | --- |
| `needs-triage` | `needs-triage` | A maintainer needs to evaluate the issue |
| `needs-info` | `needs-info` | Work is waiting for information from the reporter |
| `ready-for-agent` | `ready-for-agent` | The issue is fully specified and an AFK agent can implement it |
| `ready-for-human` | `ready-for-human` | The issue is fully specified but requires human implementation |
| `wontfix` | `wontfix` | The issue will not be actioned |

Apply `ready-for-agent` directly to synthesized PRDs and AFK implementation slices. Do not add `needs-triage` first when the work is already fully specified.
