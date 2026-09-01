# Practice events + shared devices — admin guide

> For event organisers setting up training and shared timing devices.
> Status: doco only (Part 3 of `docs/plan/identity-login.md`).

## Demo vs Practice

- **Demo** (`demo-training`) is the built-in dev/test event: local-only, never
  published, no identity required. It exists so developers and testers can
  exercise the app — recordings carry no official attribution (that's fine).
- **Practice** is a **published** event created by an organiser, used to train
  officials with realistic timing. Because it's published, every recording is
  signed and attributed to the operator's app identity, and results behave
  exactly like a real event.

## Creating a practice event

1. On the **Events** page, **Plan a new event** to get a fresh editable draft.
2. Set it up like a real event — one or more stages (Stopwatch style for
   single-operator timing), a class list, and a handful of entrant cars.
   You can copy the look of the Demo by opening the Demo and using **Clone** as
   a starting point if you prefer.
3. Name it (e.g. "Practice") with the current year — the name/year form the
   room alias at publish.
4. Publish it to the event homeserver (local Synapse, open registration). The
   normal publish checks apply, including that **key officials** carry a Real
   Name and a contact mobile number.
5. Once published, the **Event config** page shows the **invite QR** for the
   event. Print it (or the invite link) so officials can scan it at training.

## Officials joining practice

Scanning the invite QR drives the normal login flow:

- Online + no identity yet → Matrix SSO first, then a local account on the
  event homeserver is created with the same localpart (`@alice:synapse`).
- If that localpart already exists on the Synapse, the app prompts: create a
  variant (`alice2`), scan an account QR from another device, or sign in
  manually.
- Fully offline → a local username/password on the event homeserver.

Officials who will run the timing should be in the event's **Organisers** list
(Event config → Organisers) with a **role** (Key official / Official), their
**real name**, and a **contact mobile** — key officials are enforced at publish.

## Shared timekeeper laptop (HQ)

The HQ timekeeper machine may be shared by several officials under a generic OS
login. Two supported setups:

1. **Event shared login** (simplest): create one shared account for the event's
   homeserver and sign the laptop into it. All timing from the laptop is then
   attributed to that shared identity.
2. **Per-device accounts** (larger events): provision an account per device
   (or per official) so the audit trail distinguishes who recorded what. The
   device's **app identity** (`kt_identity`) is what appears on each record —
   a Matrix id where SSO was used, otherwise the local account.

Whatever the setup, the device key signs everything and the app identity names
it; officials auditing a run see both the identity and the signing device.

## Notes

- The event Synapse is expected to be **temporary and non-federated** (live for
  the duration of the event), so joining always creates/uses a local account on
  it — there's no federation lookup.
- Demo stays blank-identity by design; use a Practice event when you want
  attributed training data.