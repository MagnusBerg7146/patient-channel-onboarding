use crate::infrai_client::{InfraiClient, InfraiError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignupChannel {
    Email,
    Sms,
}

#[derive(Debug, Deserialize)]
pub struct AppointmentSignup {
    pub user_id: String,
    pub patient_name: String,
    pub email: String,
    pub phone: String,
    pub signup_channel: SignupChannel,
    pub appointment_id: String,
    pub starts_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryChoice {
    Email,
    Sms,
    Hold,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OnboardingOutcome {
    Sent {
        channel: &'static str,
        message_id: String,
    },
    Held {
        reason: &'static str,
    },
}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error(transparent)]
    Infrai(#[from] InfraiError),
}

pub fn choose_channel(
    preferred: SignupChannel,
    email_suppressed: bool,
    sms_suppressed: bool,
) -> DeliveryChoice {
    match preferred {
        SignupChannel::Email if !email_suppressed => DeliveryChoice::Email,
        SignupChannel::Sms if !sms_suppressed => DeliveryChoice::Sms,
        SignupChannel::Email if !sms_suppressed => DeliveryChoice::Sms,
        SignupChannel::Sms if !email_suppressed => DeliveryChoice::Email,
        _ => DeliveryChoice::Hold,
    }
}

pub async fn onboard_patient(
    client: &InfraiClient,
    signup: &AppointmentSignup,
) -> Result<OnboardingOutcome, WorkflowError> {
    if !client.has_appointment_consent(&signup.user_id).await? {
        return Ok(OnboardingOutcome::Held {
            reason: "appointment notification consent is absent",
        });
    }

    let (email_suppressed, sms_suppressed) = tokio::try_join!(
        client.email_suppressed(&signup.email),
        client.sms_suppressed(&signup.phone)
    )?;
    let message = format!(
        "Hi {}, your appointment {} is scheduled for {}.",
        signup.patient_name, signup.appointment_id, signup.starts_at
    );
    let idempotency_key = format!("appointment-welcome-{}", signup.appointment_id);

    match choose_channel(signup.signup_channel, email_suppressed, sms_suppressed) {
        DeliveryChoice::Email => {
            let sent = client
                .send_email(
                    &signup.email,
                    "Your appointment is scheduled",
                    &message,
                    &format!("{idempotency_key}-email"),
                )
                .await?;
            Ok(OnboardingOutcome::Sent {
                channel: "email",
                message_id: sent.message_id,
            })
        }
        DeliveryChoice::Sms => {
            let sent = client
                .send_sms(&signup.phone, &message, &format!("{idempotency_key}-sms"))
                .await?;
            Ok(OnboardingOutcome::Sent {
                channel: "sms",
                message_id: sent.message_id,
            })
        }
        DeliveryChoice::Hold => Ok(OnboardingOutcome::Held {
            reason: "all registered channels are suppressed",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_signup_falls_back_to_sms_when_email_is_suppressed() {
        assert_eq!(
            choose_channel(SignupChannel::Email, true, false),
            DeliveryChoice::Sms
        );
    }

    #[test]
    fn holds_when_both_patient_channels_are_suppressed() {
        assert_eq!(
            choose_channel(SignupChannel::Sms, true, true),
            DeliveryChoice::Hold
        );
    }
}
