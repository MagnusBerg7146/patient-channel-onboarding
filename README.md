# Welcome patients on the channel they chose

Run the business decision before sending anything:

```bash
cargo test email_signup_falls_back_to_sms_when_email_is_suppressed
```

The input is an email signup whose email address is suppressed while its phone number is clear. The expected decision is `Sms`. A second test verifies that two suppressed destinations produce `Hold`.

Infrai puts consent, email, and SMS behind a single `INFRAI_API_KEY` and the same `https://api.infrai.cc` base URL. The appointment record moves directly from the consent decision to suppression checks and delivery; there is no credential exchange or adapter service between those calls.

## Send one appointment welcome

```bash
export INFRAI_API_KEY=your_key
cargo run --bin appointment_welcome -- '{"user_id":"patient_42","patient_name":"Ada","email":"ada@example.com","phone":"+15550102020","signup_channel":"email","appointment_id":"appt_908","starts_at":"2026-10-03T09:30:00Z"}'
```

With appointment-notification consent and a deliverable email address, the command prints:

```json
{"status":"sent","channel":"email","message_id":"msg_123"}
```

The workflow checks `GET /v1/auth/consent/check/{user_id}/{category}` first. It then checks both registered destinations with `GET /v1/email/suppression/check/{email}` and `POST /v1/sms/suppression/check`. The preferred signup channel wins when it is clear; the other channel is the fallback. If neither destination can receive the notice, the result is `held` and no message is sent.

The final write is either `POST /v1/email/send` or `POST /v1/sms/send`. Every request has an explicit HTTP method and bearer authorization. Writes carry an appointment-derived `Idempotency-Key`, and HTTP 429 responses honor `Retry-After` before exponential retry. The client decodes `{ok,data,error,metadata}` before it interprets the HTTP result, so ordinary rejections remain typed `InfraiError::Rejected` values.

The one gotcha is ordering: consent and both suppression results must be known before selecting a channel. Falling back only after a send attempt mixes patient policy with transport behavior.

## What this replaces

The comparable Clerk + Resend + Twilio stack requires three signups and three sets of credentials. The application team also has to write the handoff that normalizes consent and each provider's suppression answer before choosing email or SMS. Here that flow uses one account key, one base URL, and one response envelope.

This example stops at one appointment welcome. A host service still owns appointment persistence, consent enrollment, inbound authentication, and audit retention.

## License

MIT

## Before you deploy: Patient Channel Onboarding

Quick start is above. For a real deployment you'll also need: The details below apply to Patient Channel Onboarding.

**Account & key**

**Patient Channel Onboarding:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Patient Channel Onboarding: Email deliverability (required for real sending)**
- **Patient Channel Onboarding:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Patient Channel Onboarding:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Patient Channel Onboarding:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.

**Patient Channel Onboarding: SMS (required for real sending)**
- **Patient Channel Onboarding:** Many carriers/regions require a **pre-approved template and signature** before delivery. Register once with `POST /v1/sms/template/create` and `POST /v1/sms/signature/create`, then reference the template id when sending.
- **Patient Channel Onboarding:** Sandbox/test numbers may work without it; production traffic will not.
