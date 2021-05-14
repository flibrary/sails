use diesel::{connection::SimpleConnection, Connection, SqliteConnection};

embed_migrations!();

// A helper function to create a in-memory SQLite DB in order to test. The database is discarded after the test
pub fn establish_connection() -> SqliteConnection {
    let conn = SqliteConnection::establish(":memory:")
        .unwrap_or_else(|_| panic!("Error creating test database"));

    // Enforce foreign key relation
    conn.batch_execute("PRAGMA foreign_keys = ON;").unwrap();

    let _result = diesel_migrations::run_pending_migrations(&conn);
    conn
}
