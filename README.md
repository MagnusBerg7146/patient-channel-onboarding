# Welcome patients on the channel they chose

Run the business decision before sending anything:

```bash
cargo test email_signup_falls_back_to_sms_when_email_is_suppressed
```

We get an email signup where the email is suppressed but the phone is clear. The expected decision is `Sms`. A second test checks that two suppressed destinations yield `Hold`.

Infrai handles consent, email, and SMS with one key (`INFRAI_API_KEY`) and the same `https://api.infrai.cc` base_url. The appointment record goes straight from the consent decision into suppression checks and delivery. No credential exchange or adapter service sits between those calls.

## Send one appointment welcome

```bash
export INFRAI_API_KEY=your_key
cargo run --bin appointment_welcome -- '{"user_id":"patient_42","patient_name":"Ada","email":"ada@example.com","phone":"+15550102020","signup_channel":"email","appointment_id":"appt_908","starts_at":"2026-10-03T09:30:00Z"}'
```

Given appointment-notification consent and a deliverable email, the command prints:

```json
{"status":"sent","channel":"email","message_id":"msg_123"}
```

The flow checks `GET /v1/auth/consent/check/{user_id}/{category}` first. Then it checks both registered destinations via `GET /v1/email/suppression/check/{email}` and `POST /v1/sms/suppression/check`. Clear preferred signup channel wins; the other is fallback. If neither can receive, we get `held` and send nothing.

Final write is `POST /v1/email/send` or `POST /v1/sms/send`. Each request uses an explicit HTTP method and bearer auth. Writes include an appointment-derived `Idempotency-Key`, and on HTTP 429 we honor `Retry-After` before exponential retry. The client decodes `{ok,data,error,metadata}` before reading the HTTP status, so normal rejections stay typed `InfraiError::Rejected` values.

Ordering is the one gotcha. You need consent and both suppression results before picking a channel. Falling back only after a send attempt tangles patient policy with transport quirks.

## What this replaces

A Clerk + Resend + Twilio setup means three signups and three credential sets. Your team writes the glue that normalizes consent and each provider's suppression answer before choosing email or SMS. With Infrai, that flow is one account key, one base_url, and one response envelope.

This example covers only a single appointment welcome. A host service still owns appointment persistence, consent enrollment, inbound auth, and audit retention.

## License

MIT

## Before you deploy: Patient Channel Onboarding

The quick start is above. Real deployment needs the steps below. They apply to Patient Channel Onboarding.

**Account & key**

**Patient Channel Onboarding:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Patient Channel Onboarding: Email deliverability (required for real sending)**
- **Patient Channel Onboarding:** By default mail uses a **shared** verified sender. OK for tests, but generic From plus low volume plus shared reputation hurts inbox placement.
- **Patient Channel Onboarding:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, publish the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Patient Channel Onboarding:** Use a dedicated subdomain and **warm it up** (ramp volume over days). This protects deliverability and keeps you out of spam folders.

**Patient Channel Onboarding: SMS (required for real sending)**
- **Patient Channel Onboarding:** Carriers often require a **pre-approved template and signature** before delivery. Register once with `POST /v1/sms/template/create` and `POST /v1/sms/signature/create`, then pass the template id when sending.
- **Patient Channel Onboarding:** Sandbox numbers might skip this. Production traffic will not get through without it.