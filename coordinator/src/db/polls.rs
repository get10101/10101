use crate::schema::answers;
use crate::schema::choices;
use crate::schema::polls;
use crate::schema::sql_types::PollTypeType;
use anyhow::bail;
use anyhow::Result;
use diesel::query_builder::QueryId;
use diesel::AsExpression;
use diesel::FromSqlRow;
use diesel::Identifiable;
use diesel::Insertable;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::QueryResult;
use diesel::Queryable;
use diesel::RunQueryDsl;
use diesel::Selectable;
use diesel::SelectableHelper;
use std::any::TypeId;
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression, Eq, Hash)]
#[diesel(sql_type = PollTypeType)]
pub enum PollType {
    SingleChoice,
}

impl QueryId for PollTypeType {
    type QueryId = PollTypeType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

#[derive(Insertable, Queryable, Identifiable, Selectable, Debug, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = polls)]
#[diesel(primary_key(id))]
pub struct Poll {
    pub id: i32,
    pub poll_type: PollType,
    pub question: String,
    pub active: bool,
    pub creation_timestamp: OffsetDateTime,
}

#[derive(Insertable, Queryable, Identifiable, Selectable, Debug, Clone, Eq, PartialEq)]
#[diesel(belongs_to(Poll))]
#[diesel(table_name = choices)]
#[diesel(primary_key(id))]
pub struct Choice {
    pub id: i32,
    pub poll_id: i32,
    pub value: String,
}
#[derive(Insertable, Queryable, Identifiable, Debug, Clone)]
#[diesel(primary_key(id))]
pub struct Answer {
    pub id: Option<i32>,
    pub choice_id: i32,
    pub trader_pubkey: String,
    pub value: String,
    pub creation_timestamp: OffsetDateTime,
}

pub fn active(conn: &mut PgConnection) -> QueryResult<Vec<commons::Poll>> {
    let _polls: Vec<Poll> = polls::table.load(conn)?;
    let _choices: Vec<Choice> = choices::table.load(conn)?;

    let results = polls::table
        .left_join(choices::table)
        .select(<(Poll, Option<Choice>)>::as_select())
        .load::<(Poll, Option<Choice>)>(conn)?;

    let mut polls_with_choices = HashMap::new();
    for (poll, choice) in results {
        let entry = polls_with_choices.entry(poll).or_insert_with(Vec::new);
        if let Some(choice) = choice {
            entry.push(choice);
        }
    }

    let polls = polls_with_choices
        .into_iter()
        .map(|(poll, choice_vec)| commons::Poll {
            id: poll.id,
            poll_type: poll.poll_type.into(),
            question: poll.question,
            choices: choice_vec
                .into_iter()
                .map(|choice| commons::Choice {
                    id: choice.id,
                    value: choice.value,
                })
                .collect(),
        })
        .collect();
    Ok(polls)
}

impl From<PollType> for commons::PollType {
    fn from(value: PollType) -> Self {
        match value {
            PollType::SingleChoice => commons::PollType::SingleChoice,
        }
    }
}

pub fn add_answer(conn: &mut PgConnection, answers: commons::PollAnswers) -> Result<()> {
    let mut affected_rows = 0;
    for answer in answers.answers {
        affected_rows += diesel::insert_into(answers::table)
            .values(Answer {
                id: None,
                choice_id: answer.choice_id,
                trader_pubkey: answers.trader_pk.to_string(),
                value: answer.value,
                creation_timestamp: OffsetDateTime::now_utc(),
            })
            .execute(conn)?;
    }

    if affected_rows == 0 {
        bail!(
            "Could not insert answers by user {}.",
            answers.trader_pk.to_string()
        );
    } else {
        tracing::trace!(%affected_rows, trade_pk = answers.trader_pk.to_string(),
            "Added new answers to a poll.");
    }
    Ok(())
}
