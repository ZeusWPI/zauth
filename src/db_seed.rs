use std::default::Default;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Rocket, Orbit};
use crate::DbConn;

pub struct Seeder {
    empty_db: bool,
    clients_to_seed: usize,
    users_to_seed: usize,
    admin_password: Option<String>,
    client_name: Option<String>,
}

impl Default for Seeder {
    fn default() -> Self {
        Seeder {
            empty_db: false,
            clients_to_seed: 0,
            users_to_seed: 0,
            admin_password: None,
            client_name: None,
        }
    }
}

impl Seeder {
    fn from_env() {
        let mut seeder = Self::default();
        if let Ok(_) = std::env::var("ZAUTH_EMPTY_DB") {
            seeder.empty_db = true;
        }
        if let Ok(number) = std::env::var("ZAUTH_SEED_CLIENT") {
            match number.parse() {
                Ok(num) => seeder.clients_to_seed = num,
                Err(_) =>
                    eprintln!(
                        "ZAUTH_SEED_CLIENT=\"{}\" error, expected number",
                        number
                    ),
            };
        }
        if let Ok(number) = std::env::var("ZAUTH_SEED_USER") {
            match number.parse() {
                Ok(num) => seeder.users_to_seed = num,
                Err(_) =>
                eprintln!(
                    "ZAUTH_SEED_USER=\"{}\" error, expected number",
                    number
                ),
            };
        }
        if let Ok(pw) = std::env::var("ZAUTH_ADMIN_PASSWORD") {
            seeder.admin_password = Some(pw);
        }
        if let Ok(client) = std::env::var("ZAUTH_CLIENT_NAME") {
            seeder.client_name = Some(client);
        }
    }

    async fn delete_all(conn: &DbConn) {
        todo!()
    }

    async fn seed_client(conn: &DbConn) {
        todo!()
    }

    async fn seed_user(conn: &DbConn) {
        todo!()
    }
}

impl Fairing for Seeder {
    fn info(&self) -> Info {
        Info {
            name: "Seed database",
            kind: Kind::Liftoff,
        }
    }

    async fn on_liftoff(&self, rocket: &Rocket<Orbit>) {

    }
}