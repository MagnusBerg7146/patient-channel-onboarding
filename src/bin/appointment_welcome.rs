use patient_channel_onboarding::{
    appointment_onboarding::{onboard_patient, AppointmentSignup},
    infrai_client::InfraiClient,
};
use std::{env, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .ok_or("pass one JSON appointment signup as the first argument")?;
    let signup: AppointmentSignup = serde_json::from_str(&input)?;
    let client = InfraiClient::from_env()?;
    let outcome = onboard_patient(&client, &signup).await?;
    println!("{}", serde_json::to_string(&outcome)?);
    Ok(())
}
