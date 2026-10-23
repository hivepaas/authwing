# Design notes

How authwing stores, syncs and protects data. The vault, its sync and the
devices joining it come from shellwing, where these notes were agreed in
October 2026, and are built; where a note still says *host*, authwing's
counterpart is a *code*. Questions still to settle are marked **Open**.

| Note | Covers |
| --- | --- |
| [config.md](config.md) | Files on disk, their formats, settings precedence |
| [sync.md](sync.md) | Accounts, the sync engine, backends, plan limits |
| [vault.md](vault.md) | Secrets, the key hierarchy, encryption, local storage |
| [teams.md](teams.md) | Organizations, sharing, the self-hosted server |
| [note-conflicts.md](note-conflicts.md), [note-images.md](note-images.md) | shellwing's notes: how the vault keeps both texts of one changed on two devices, and its attachments. authwing has no notes; the vault keeps the mechanisms. |

Principles shared by all of them:

- **Works without an account.** Signing in only adds authwing's own cloud sync
  and team features.
- **End-to-end encrypted sync.** Servers and storage backends hold ciphertext,
  the services' names included. Signing in proves who you are; it never
  unlocks data.
- **One sync engine.** authwing cloud, a self-hosted server, Git, S3 and a
  plain folder are all backends of the same engine.
- **A code goes only where it belongs.** It is typed only into a site saved
  with it; a page that looks like one is said to, and gets nothing.
- **What the app learns stays on the device.** The windows in use, the pages
  open and what was said aloud are read on the device and never sent; names
  the user teaches sync encrypted, like the codes.
