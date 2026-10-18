# Windows Defender development-build investigation

Date: 2026-10-17. This investigation concerns the local ZT development executable and isolated tests. The installed archive and its running service were not upgraded during this investigation.

## Observed failure

Microsoft Defender real-time protection detected `Trojan:Win32/Bearfoos.A!ml` (ThreatID 2147731250) in the temporary test copy of `zt.exe` at 16:13:28, then recorded the running copy at 16:13:46. At 16:19:06 it detected `D:\zt-target\debug\zt.exe` while PowerShell attempted to read/copy it. Event 1117 records quarantine with success. Definitions were `1.461.76.0`, engine `1.1.26090.9`.

The second detection occurred before that copy was executed. Therefore a particular Card operation is not necessary to reproduce the later block. The Windows event identifies the file and detection, but does not identify a source line or the classifier's decisive feature.

The user restored the first temporary copy and authorized continuing the investigation. An attempted exact-file restore of the debug executable returned administrator-required error `0x80070005`. No exclusions, real-time-protection changes, signature removals, or quarantine-wide restores were applied.

## Evidence and limits

The root source and lockfile review found no dependency or build-script changes between the design baseline and published digest changes. The root source had no uncommitted changes when the failure was investigated. The existing optimized build, SHA-256 `F0CCEB17797C22D86AA72A6979FBD6200396CFBECB49FB09898C07F6BCF4D518`, scanned with no threats before changing startup. Thus the old startup code alone is not sufficient to explain the difference between accepted release and blocked debug builds.

The leading explanation is a false positive specific to the development binary's classification. This is an inference, not a Microsoft-confirmed determination. The original blocked binary could not be read for a hash comparison. A clean rebuilt file does not reveal the original classifier's reasoning or guarantee acceptance of every future build.

## Application change

`src/main.rs::spawn_daemon` previously ran hidden PowerShell with `-ExecutionPolicy Bypass` and `Start-Process`. The application requires only its own daemon. It now calls Windows `CreateProcessW` directly, names the executable explicitly, quotes the archive argument, suppresses the console and disables handle inheritance. This removes a shell dependency and the unnecessary execution-policy override. It does not remove the daemon, GUI, sign-in support, or shutdown handling.

The first Rust `Command` attempt exposed an inherited-pipe problem: the long-lived daemon kept the caller's captured output open. The final native call disables inheritance, closes its returned process/thread handles in the parent, and preserves the existing readiness check. `Cargo.toml` enables process/security type bindings in the existing `windows-sys` dependency; it adds no new dependency package.

The public regression starts ZT with no shell on PATH and an archive path containing spaces and an apostrophe, then checks status and stops it. Before the change it failed with `failed to launch daemon through PowerShell: program not found`; the final implementation passes. This proves the startup behavior, not the cause of an opaque antivirus verdict.

## Scan and execution verification

Scans use Defender's normal custom file scan with `-DisableRemediation -ReturnHR`; they do not disable scanning. Real-time protection and behavior monitoring remain enabled.

| Artifact | SHA-256 | Defender result |
| --- | --- | --- |
| Final debug `D:\zt-target\debug\zt.exe` | `8C274C6B63F8CA6D249FB1F35A8EE597B1A4292F6241AB1DA68A692D964D578B` | No threats, exit 0 |
| Final release `D:\zt-target\release\zt.exe` | `CD98CFD826D4628A718302F7B384BA71590B5B6D5E7E1BCBD386063684073780` | No threats, exit 0 |

The previously interrupted `scoped-bulk.tests.ps1` copied and ran the final debug build, created its own archive, passed group and address-checker checks, and reached its expected pre-fix `--cards` assertion failure. It no longer failed at the executable copy/start boundary. Logs: `D:/zt-target/qualification/defender-native-*.log` and `issue67-check-cards-red.log`.

The complete root regression passes: 338 tests, zero failures, one deliberately ignored real-clipboard test. This includes daemon restart, sign-in, sign-out, shutdown, GUI, CLI, MCP, model and Workspace suites. The PRD and unfinished implementation issues remain open. No actual digest workflow was run.

## Later host observations

At 16:47:45 Defender recorded the same detection on `C:\Users\chenx\AppData\Local\Temp\scoped-bulk-test-76288\bin\zt.exe`. At 16:55 that copy was readable and its SHA-256 was exactly `8C274C6B63F8CA6D249FB1F35A8EE597B1A4292F6241AB1DA68A692D964D578B`, matching the rebuilt debug executable above. A clean scan and the direct launcher therefore do not establish a permanent Defender fix. The same bytes have received different outcomes in normal testing; the decisive classifier/context signal remains unknown. No exclusion or protection change was applied.

A later parallel regression run failed during PowerShell startup with an explicit `System.OutOfMemoryException`, and another MCP child closed before the book assertions. Windows System event 2004 recorded virtual-memory exhaustion at 16:50:24 and 16:53:48. At 16:55 the host had about 13 GiB of commit headroom again. The failures were retained under `D:/zt-target/qualification/issue67-book-suites/`; the two failed suites were rerun sequentially after the host recovered. Their outcome is recorded in the delivery status. User applications and security settings were not changed to make those tests pass.

At 21:09 on 2026-10-17, the current debug build had SHA-256 `870917B1585B2646BCEFC6FA36B3379F12B53D6660854B24FAA52AF7781BAFC2`. Its complete root suite passed 361 tests (one manual clipboard test ignored); 81 digest unit tests and 14 sequential public compiler/ledger suites also passed. The final two group-completion suites passed again after an additional anchor-coverage regression fix. Defender real-time protection and behavior monitoring were both enabled, with definition version `1.461.76.0`. The newest retained `zt.exe` detection was still the 16:47:45 event above, including after the public suite copied and executed the current build. This confirms current executable availability under the observed protection settings; it does not identify the original classifier trigger or establish a permanent antivirus fix. These tests did not upgrade the production installation or run a digest workflow.

If a later build is detected, retain its exact hash, detection events and build identity for review. Microsoft's [false-positive guidance](https://learn.microsoft.com/en-us/defender-endpoint/defender-endpoint-false-positives-negatives) describes submitting the affected file to Microsoft for analysis. No application file was uploaded during this investigation.

At 21:59 on 2026-10-17, an additional normal Defender custom scan of the current debug executable (SHA-256 `870917B1585B2646BCEFC6FA36B3379F12B53D6660854B24FAA52AF7781BAFC2`) reported no threats and exit 0. The exact public test `service_starts_without_a_shell_and_preserves_archive_path` passed again, starting and stopping its isolated service with an empty shell search path. Protection and behavior monitoring remained enabled, and the latest recorded ZT detection remained 16:47. No new source change is justified as a proven antivirus fix by this result: the current failure does not reproduce, and Defender's retained events do not expose its deciding classifier feature.
