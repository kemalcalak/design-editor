# Antivirus False Positives on Windows Builds

Some antivirus engines occasionally flag the OpenPencil Windows installer
(`OpenPencil-<version>-<arch>-win-setup.exe`) as a trojan or generic threat
(e.g. `Wacatac`, `Trojan.Generic`, `Unsafe.AI_Score`). These detections are
**heuristic false positives**, not evidence of malware. This page explains
why they happen and how to verify that your download is the exact file our
CI produced.

## Why this happens

- **Low prevalence.** Reputation-based engines (Microsoft SmartScreen,
  many cloud AV heuristics) score files by how many machines have seen
  them. Every new OpenPencil release is a brand-new binary with a new
  hash, so it starts with zero reputation until enough users run it.
- **NSIS installer packaging.** OpenPencil ships in an NSIS installer —
  the same packaging used by thousands of legitimate apps, but also
  historically abused by malware droppers. Several heuristics weight NSIS
  self-extractors negatively by default.
- **Certificate trust chain.** Release binaries are currently signed with
  a project-local certificate rather than a CA-issued Authenticode
  certificate, so Windows reports an "Unknown publisher" and AV engines
  get no trust-chain signal to offset the heuristics above. Moving to a
  CA-issued certificate is on the roadmap.

## How to verify a download is genuine

Every release is built from public source in GitHub Actions and uploaded
directly by CI — no human touches the binaries.

1. **Checksums.** Each release attaches `SHA256SUMS.txt`. Compare it with
   your download:

   ```powershell
   Get-FileHash .\OpenPencil-<version>-<arch>-win-setup.exe -Algorithm SHA256
   ```

2. **Build provenance.** Each asset carries a signed [SLSA build
   provenance attestation](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations)
   proving it was produced by this repository's release workflow at a
   specific commit:

   ```bash
   gh attestation verify OpenPencil-<version>-<arch>-win-setup.exe \
     --repo ZSeven-W/openpencil
   ```

3. **Build it yourself.** The entire product is open source:

   ```bash
   cargo build -p op-host-desktop --release
   ```

## If your antivirus flags a release

- Verify the file with the steps above. If verification **fails**, delete
  the file and [open an issue](https://github.com/ZSeven-W/openpencil/issues)
  immediately.
- If verification passes, the detection is a false positive. Please report
  it to your AV vendor (for Microsoft Defender:
  <https://www.microsoft.com/en-us/wdsi/filesubmission>) — vendor-side
  clearances are what make these warnings disappear for everyone — and
  feel free to open an issue naming the engine and detection so we can
  track and submit it too.
