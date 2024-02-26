use anyhow::bail;
use bitcoin::secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Poll {
    pub id: i32,
    pub poll_type: PollType,
    pub question: String,
    pub choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Choice {
    pub id: i32,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Answer {
    pub choice_id: i32,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PollType {
    SingleChoice,
}

impl TryFrom<&str> for PollType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "single_choice" => Ok(PollType::SingleChoice),
            _ => {
                bail!("Unsupported poll type")
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PollAnswers {
    pub poll_id: i32,
    pub trader_pk: PublicKey,
    pub answers: Vec<Answer>,
}
