# Teams and the self-hosted server

## Concepts

```
Organization (Acme)
├─ Members        owner, admin, member
├─ Groups         "Backend", "Ops", "Intern": people bundled for granting access
└─ Collections    "Ops", "Finance": shared codes
     granted to a group or a person as:
       Use       see the codes, copy and type them
       Edit      add, change and delete codes
       Manage    grant others access to the collection
```

- **Personal stays private.** Each person's Personal space is never visible to
  the organization.
- **One home per record.** A record lives in exactly one space: Personal or one
  collection.
- **Sidebar.** Each collection is its own section, for example
  "Acme · Ops".

## Sharing with end-to-end encryption

The key hierarchy is in [vault.md](vault.md). In short, each person has a key
pair, and each collection key is wrapped once for each member's public key.

- **Adding a member.**
  1. The new member signs in and publishes a public key.
  2. An admin's app checks the key's fingerprint. The fingerprint is shown on
     both screens, as in Bitwarden's "Confirm member".
  3. The admin's app wraps the collection key for the new member.

  The server cannot do this step, because it never holds collection keys.
- **Removing a member.**
  - The server stops serving the collection to them.
  - An admin's app rotates the collection key and re-encrypts its records.
  - The app lists every code the person could use, so that two-factor
    sign-in can be set up again at each of those sites.
- **SSO.** SSO establishes identity only. The decryption key still comes from a
  trusted device or an admin approving the new one.

Limits to state plainly to customers:

- **"Can use a code but cannot see its key" is a UI rule, not cryptography.**
  The app must hold a code's secret key to make its codes.
- **Removing a member cannot undo what they already took.** Whoever had a
  code's key can make its codes for as long as the site accepts that key.
  That is why removal comes with the list of codes to set up again.

### Setting a code up again

A one-time code has no equivalent of a certificate that expires: the only way
to stop a key working is to turn two-factor sign-in off and on again at the
site, which gives a new key. The removal checklist makes that quick:

- Each code on the list links to its site's two-factor settings, from the
  sites saved with it.
- The new key is added to the collection from the QR code the site shows
  (scanned on the screen), and replaces the old code there.
- The list keeps which codes are done, and who did them, until every one is.

## Audit log

The server records reliably:

- Sign-ins.
- Record created, changed or deleted (by id).
- Membership and permission changes.
- Who downloaded which collection.

"Who used which code, when" is reported by the client and cannot be verified:
the codes are made on the device, offline.

## Editions

| | Community (self-hosted, free) | Team | Business |
| --- | --- | --- | --- |
| Users | Up to 3 (**Open**) | Per seat | Per seat |
| Personal sync, unlimited codes | Yes | Yes | Yes |
| Collections | 1 | Unlimited | Unlimited |
| Groups, Use/Edit/Manage | | Yes | Yes |
| Removal checklist, key rotation | | Yes | Yes |
| Audit log | | 30 days | Unlimited, export |
| SSO (OIDC, SAML), SCIM | | | Yes |
| Policies (no export, codes typed but not shown, vault lock required…) | | | Yes |

Team and Business are sold both in authwing cloud and as self-hosted licenses.
**Open:** prices.

## authwing-server

**One codebase.** authwing cloud runs the same server, with billing turned on.

**Licensing.** This is open core in one repository:

- **The core is AGPL-3.0.** Public code earns trust for a product that holds
  credentials. Anyone offering it as a service must publish their changes.
- **Team features live in `ee/` under a commercial license**, as with GitLab EE or
  Bitwarden. The source is visible, but use needs a valid license.
- **Not FSL or BSL for the whole server.** Those allow modifying the code for
  internal use, which includes removing the license check. That suits Sentry,
  which sells its cloud, not self-hosted licenses.

**License key.** A file signed with Ed25519 by the portal, carrying the
organization, seats, features and expiry.

- The server verifies it offline, so it works without internet access.
- When the license expires, team features become read-only. People are never
  locked out of their own credentials.

**Stack.**

- **Language.** Go: the server never decrypts anything.
  A format spec with test vectors keeps the Rust client and the Go server in
  agreement.
- **Database.** SQLite by default, Postgres optional.
- **Distribution.** A single binary or Docker image, plus a one-click template
  on HivePaaS.

## Order

1. Personal sync, working well ([sync.md](sync.md)).
2. Collections, groups and permissions, plus the removal checklist.
3. Audit log.
4. SSO and SCIM.
5. Policies.
