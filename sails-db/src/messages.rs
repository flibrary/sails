use crate::{error::SailsDbResult as Result, schema::messages};
use chrono::naive::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// A psuedo struct for managing messages
pub struct Messages;

impl Messages {
    pub fn get_conv(
        conn: &SqliteConnection,
        participant_a: &str,
        participant_b: &str,
    ) -> Result<Vec<Message>> {
        use crate::schema::messages::dsl::*;
        // ((send == A) && (recv == B)) && ((send == B) && (recv A))
        Ok(messages
            .filter(
                (send.eq(participant_a).and(recv.eq(participant_b)))
                    .or(send.eq(participant_b).and(recv.eq(participant_a))),
            )
            .order(time_sent.asc())
            .load::<Message>(conn)?)
    }

    // Return a vector of messages sent by distinct users in a descending chronological order.
    pub fn get_list(conn: &SqliteConnection, receiver: &str) -> Result<Vec<Message>> {
        use crate::schema::messages::dsl::*;
        Ok(messages
            .filter(recv.eq(receiver))
            .group_by(send)
            .order(diesel::dsl::max(time_sent).desc())
            .load::<Message>(conn)?)
    }

    pub fn send<T: ToString>(
        conn: &SqliteConnection,
        sender: T,
        receiver: T,
        body_provided: T,
    ) -> Result<()> {
        use crate::schema::messages::dsl::*;

        let msg = Message::new(sender, receiver, body_provided);
        diesel::insert_into(messages).values(msg).execute(conn)?;
        Ok(())
    }
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Clone,
    PartialEq,
)]
pub struct Message {
    id: String,
    send: String,
    recv: String,
    body: String,
    time_sent: NaiveDateTime,
}

impl Message {
    pub fn new<T: ToString>(sender: T, receiver: T, body: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            send: sender.to_string(),
            recv: receiver.to_string(),
            body: body.to_string(),
            // This might have some issue with UTC
            time_sent: chrono::offset::Local::now().naive_utc(),
        }
    }

    /// Get a reference to the message's send.
    pub fn get_send(&self) -> &str {
        &self.send
    }

    /// Get a reference to the message's recv.
    pub fn get_recv(&self) -> &str {
        &self.recv
    }

    /// Get a reference to the message's body.
    pub fn get_body(&self) -> &str {
        &self.body
    }

    /// Get a reference to the message's time sent.
    pub fn get_time_sent(&self) -> &NaiveDateTime {
        &self.time_sent
    }
}

#[cfg(test)]
mod tests {
    use super::Messages;
    use crate::{test_utils::establish_connection, users::Users};

    #[test]
    fn get_list() {
        let conn = establish_connection();
        // Our sender
        let sender = Users::register(
            &conn,
            "TestUser@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
        )
        .unwrap();

        let sender2 = Users::register(
            &conn,
            "AnotherSender@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
        )
        .unwrap();
        let receiver = Users::register(
            &conn,
            "Him@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
        )
        .unwrap();

        Messages::send(&conn, sender.as_str(), receiver.as_str(), "Hello").unwrap();
        Messages::send(&conn, sender.as_str(), receiver.as_str(), "Are you there?").unwrap();
        Messages::send(&conn, receiver.as_str(), sender.as_str(), "Yes!").unwrap();
        Messages::send(&conn, sender2.as_str(), receiver.as_str(), "Hello?").unwrap();
        Messages::send(
            &conn,
            sender.as_str(),
            receiver.as_str(),
            "Do you have that book?",
        )
        .unwrap();
        Messages::send(
            &conn,
            receiver.as_str(),
            sender.as_str(),
            "Sure, it is in pretty good condition",
        )
        .unwrap();
        Messages::send(
            &conn,
            sender2.as_str(),
            receiver.as_str(),
            "Can you hear me?",
        )
        .unwrap();

        let list = Messages::get_list(&conn, &receiver).unwrap();
        assert_eq!(list.get(0).unwrap().send, "AnotherSender@example.org");
        assert_eq!(list.get(1).unwrap().send, "TestUser@example.org");
    }

    #[test]
    fn trivial() {
        let conn = establish_connection();
        // Our sender
        let sender = Users::register(
            &conn,
            "TestUser@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
        )
        .unwrap();
        let sender2 = Users::register(
            &conn,
            "TestUser2@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
        )
        .unwrap();
        let receiver = Users::register(
            &conn,
            "Him@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
        )
        .unwrap();

        Messages::send(&conn, receiver.as_str(), receiver.as_str(), "Self-message").unwrap();

        Messages::send(&conn, sender.as_str(), receiver.as_str(), "Hello").unwrap();
        Messages::send(&conn, sender.as_str(), receiver.as_str(), "Are you there?").unwrap();
        Messages::send(&conn, receiver.as_str(), sender.as_str(), "Yes!").unwrap();
        Messages::send(
            &conn,
            sender.as_str(),
            receiver.as_str(),
            "Do you have that book?",
        )
        .unwrap();
        Messages::send(
            &conn,
            receiver.as_str(),
            sender.as_str(),
            "Sure, it is in pretty good condition",
        )
        .unwrap();

        // THis tests the select statement
        assert_eq!(
            Messages::get_conv(&conn, &sender, &receiver).unwrap(),
            Messages::get_conv(&conn, &receiver, &sender).unwrap()
        );

        // There should be no message between these two
        // There should also be no self-message be disclosed
        assert_eq!(
            Messages::get_conv(&conn, &sender2, &receiver)
                .unwrap()
                .len(),
            0
        );
    }
}
