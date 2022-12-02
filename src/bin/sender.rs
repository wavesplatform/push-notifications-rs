use diesel::{Connection, PgConnection};
use lib::config::PostgresConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dbconfig = PostgresConfig::load()?;
    let conn = PgConnection::establish(&dbconfig.database_url())?;

    loop {
        // read N messages from SQL
        // init notification object
    }
    // println!("Sender started");
}
