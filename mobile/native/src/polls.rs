use crate::commons::reqwest_client;
use crate::config;
use crate::db;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use commons::Answer;
use commons::Choice;
use commons::Poll;
use commons::PollAnswers;
use reqwest::Url;

pub(crate) async fn get_new_polls() -> Result<Vec<Poll>> {
    let new_polls = fetch_polls().await?;
    tracing::debug!(new_polls = new_polls.len(), "Fetched new polls");
    let answered_polls = db::load_ignored_or_answered_polls()?;
    let unanswered_polls = new_polls
        .into_iter()
        .filter(|poll| {
            !answered_polls
                .iter()
                .any(|answered_poll| answered_poll.poll_id == poll.id)
        })
        .collect::<Vec<_>>();
    tracing::debug!(unanswered_polls = unanswered_polls.len(), "Polls to answer");
    for i in &unanswered_polls {
        tracing::debug!(poll_id = i.id, "Unanswered polls");
    }
    Ok(unanswered_polls)
}

pub(crate) async fn answer_poll(choice: Choice, poll_id: i32, trader_pk: PublicKey) -> Result<()> {
    post_selected_choice(choice.clone(), poll_id, trader_pk).await?;
    db::set_poll_to_ignored_or_answered(poll_id)?;
    tracing::debug!(poll_id, choice = ?choice, "Answered poll");

    Ok(())
}

pub(crate) fn ignore_poll(poll_id: i32) -> Result<()> {
    db::set_poll_to_ignored_or_answered(poll_id)?;
    tracing::debug!(poll_id, "Poll won't be shown again");
    Ok(())
}

async fn fetch_polls() -> Result<Vec<Poll>> {
    let client = reqwest_client();
    let url = format!("http://{}", config::get_http_endpoint());
    let url = Url::parse(&url).expect("correct URL");
    let url = url.join("/api/polls")?;
    let response = client.get(url).send().await?;
    let polls = response.json().await?;
    Ok(polls)
}

async fn post_selected_choice(choice: Choice, poll_id: i32, trader_pk: PublicKey) -> Result<()> {
    let client = reqwest_client();
    let url = format!("http://{}", config::get_http_endpoint());
    let url = Url::parse(&url).expect("correct URL");
    let url = url.join("/api/polls")?;
    let response = client
        .post(url)
        .json(&PollAnswers {
            poll_id,
            trader_pk,
            answers: vec![Answer {
                choice_id: choice.id,
                value: choice.value,
            }],
        })
        .send()
        .await?;
    response.error_for_status()?;
    Ok(())
}
