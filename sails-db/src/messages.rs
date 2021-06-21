use crate::{error::SailsDbResult as Result, schema::messages, users::UserId};
use chrono::naive::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// A psuedo struct for managing messages
pub struct Messages;

impl Messages {
    pub fn get_conv(
        conn: &SqliteConnection,
        participant_a: &UserId,
        participant_b: &UserId,
    ) -> Result<Vec<Message>> {
        use crate::schema::messages::dsl::*;
        // ((send == A) && (recv == B)) || ((send == B) && (recv == A))
        Ok(messages
            .filter(
                (send
                    .eq(participant_a.get_id())
                    .and(recv.eq(participant_b.get_id())))
                .or(send
                    .eq(participant_b.get_id())
                    .and(recv.eq(participant_a.get_id()))),
            )
            .order(time_sent.asc())
            .load::<Message>(conn)?)
    }

    // Return a vector of messages sent by distinct users in a descending chronological order.
    pub fn get_list(conn: &SqliteConnection, receiver: &UserId) -> Result<Vec<Message>> {
        use crate::schema::messages::dsl::*;
        Ok(messages
            .filter(recv.eq(receiver.get_id()))
            .group_by(send)
            .order(diesel::dsl::max(time_sent).desc())
            .load::<Message>(conn)?)
    }

    pub fn send<T: ToString>(
        conn: &SqliteConnection,
        sender: &UserId,
        receiver: &UserId,
        body_provided: T,
    ) -> Result<()> {
        use crate::schema::messages::dsl::*;

        let msg = Message::new(sender, receiver, body_provided);
        diesel::insert_into(messages).values(msg).execute(conn)?;
        Ok(())
    }

    pub fn delete_msg_with_user(conn: &SqliteConnection, user: &UserId) -> Result<usize> {
        use crate::schema::messages::dsl::*;

        Ok(
            diesel::delete(messages.filter((send.eq(user.get_id())).or(recv.eq(user.get_id()))))
                .execute(conn)?,
        )
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
    pub fn new<T: ToString>(sender: &UserId, receiver: &UserId, body: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            send: sender.get_id().to_string(),
            recv: receiver.get_id().to_string(),
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
    use crate::{test_utils::establish_connection, users::*};

    #[test]
    fn get_list() {
        let conn = establish_connection();
        // Our sender
        let sender = UserForm::new(
            "TestUser@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let sender2 = UserForm::new(
            "AnotherSender@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let receiver = UserForm::new(
            "Him@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        Messages::send(&conn, &sender, &receiver, "Hello").unwrap();
        Messages::send(&conn, &sender, &receiver, "Are you there?").unwrap();
        Messages::send(&conn, &receiver, &sender, "Yes!").unwrap();
        Messages::send(&conn, &sender2, &receiver, "Hello?").unwrap();
        Messages::send(&conn, &sender, &receiver, "Do you have that book?").unwrap();
        Messages::send(
            &conn,
            &receiver,
            &sender,
            "Sure, it is in pretty good condition",
        )
        .unwrap();
        Messages::send(&conn, &sender2, &receiver, "Can you hear me?").unwrap();

        let list = Messages::get_list(&conn, &receiver).unwrap();
        assert_eq!(list.get(0).unwrap().send, "AnotherSender@example.org");
        assert_eq!(list.get(1).unwrap().send, "TestUser@example.org");
    }

    #[test]
    fn trivial() {
        let conn = establish_connection();
        // Our sender
        let sender = UserForm::new(
            "TestUser@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let sender2 = UserForm::new(
            "AnotherSender@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let receiver = UserForm::new(
            "Him@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        Messages::send(&conn, &receiver, &receiver, "Self-message").unwrap();

        Messages::send(&conn, &sender, &receiver, "Hello").unwrap();
        Messages::send(&conn, &sender, &receiver, "Are you there?").unwrap();
        Messages::send(&conn, &receiver, &sender, "Yes!").unwrap();
        Messages::send(&conn, &sender, &receiver, "Do you have that book?").unwrap();
        Messages::send(
            &conn,
            &receiver,
            &sender,
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

    #[test]
    fn delete_msg() {
        let conn = establish_connection();
        // Our sender
        let sender = UserForm::new(
            "TestUser@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let sender2 = UserForm::new(
            "AnotherSender@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let receiver = UserForm::new(
            "Him@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        Messages::send(&conn, &receiver, &receiver, "Self-message").unwrap();

        Messages::send(&conn, &sender, &receiver, "Hello").unwrap();
        Messages::send(&conn, &sender, &receiver, "Are you there?").unwrap();
        Messages::send(&conn, &receiver, &sender, "Yes!").unwrap();
        Messages::send(&conn, &sender, &receiver, "Do you have that book?").unwrap();
        Messages::send(
            &conn,
            &receiver,
            &sender,
            "Sure, it is in pretty good condition",
        )
        .unwrap();
        Messages::send(&conn, &sender2, &receiver, "Hello?").unwrap();

        // THis tests the select statement
        assert!(Messages::get_conv(&conn, &sender, &receiver).unwrap().len() > 0,);
        assert!(
            Messages::get_conv(&conn, &sender2, &receiver)
                .unwrap()
                .len()
                > 0,
        );

        // After deleting the messages of the receiver.
        Messages::delete_msg_with_user(&conn, &receiver).unwrap();

        // There should be no conversation referencing any
        assert_eq!(
            Messages::get_conv(&conn, &sender, &receiver).unwrap().len(),
            0
        );
        assert_eq!(
            Messages::get_conv(&conn, &sender2, &receiver)
                .unwrap()
                .len(),
            0
        );
    }
}
