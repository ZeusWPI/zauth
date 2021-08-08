use core::iter;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub fn random_token(token_length: usize) -> String {
	let mut rng = thread_rng();
	iter::repeat(())
		.map(|()| rng.sample(Alphanumeric))
		.map(char::from)
		.take(token_length)
		.collect()
}

// pub use dev::seed_database;
//
// mod dev {
// use crate::config::Config;
// use crate::models::client::*;
// use crate::models::user::*;
// use crate::DbConn;
// use rocket::fairing::AdHoc;
// use rocket::{Build, Ignite, Rocket};
//
// pub fn seed_database(
// mut rocket: Rocket<Ignite>,
// ) -> Rocket<Ignite> {
// assert!(rocket.config().profile.starts_with("dev"));
//
// let conn = DbConn::get_one(&rocket).expect("database connection");
//
// if let Ok(_) = std::env::var("ZAUTH_EMPTY_DB") {
// delete_all(&conn)
// }
//
// if let Ok(number) = std::env::var("ZAUTH_SEED_CLIENT") {
// let amount = number.parse().unwrap_or_else(|_e| {
// eprintln!(
// "ZAUTH_SEED_DB=\"{}\" error, expected number, defaulting \
// to 10",
// number
// );
// 10
// });
// seed_clients(rocket, amount)
// }
//
// if let Ok(number) = std::env::var("ZAUTH_SEED_USER") {
// let config_copy = config.clone();
// let amount = number.parse().unwrap_or_else(|_e| {
// eprintln!(
// "ZAUTH_SEED_USER=\"{}\" error, expected number, \
// defaulting to 10",
// number
// );
// 10
// });
//
// seed_users(&rocket, config_copy, amount)
// }
//
// if let Ok(pw) = std::env::var("ZAUTH_ADMIN_PASSWORD") {
// let config_copy = config.clone();
//
// rocket = rocket
// .attach(AdHoc::on_attach("Create admin user", |rocket| {
// create_admin(rocket, config_copy, pw)
// }));
// }
//
// if let Ok(client) = std::env::var("ZAUTH_CLIENT_NAME") {
// rocket = rocket
// .attach(AdHoc::on_attach("Create admin user", |rocket| {
// create_client(rocket, client)
// }));
// }
//
// rocket
// }
//
// fn create_admin(
// rocket: Rocket<Ignite>,
// config: Config,
// password: String,
// ) -> std::result::Result<Rocket<Ignite>, Rocket<Ignite>> {
// let username = String::from("admin");
// let conn = DbConn::get_one(&rocket).expect("database connection");
// let admin = User::find_by_username(&username, &conn)
// .or_else(|_e| {
// User::create(
// NewUser {
// username:  username.clone(),
// password:  password.clone(),
// full_name: String::from("Admin McAdmin"),
// email:     String::from("admin@example.com"),
// ssh_key:   None,
// },
// config.bcrypt_cost,
// &conn,
// )
// })
// .and_then(|mut user| {
// user.admin = true;
// user.update(&conn)
// });
// match admin {
// Ok(_admin) => {
// println!(
// "Admin created with username \"{}\" and password \"{}\"",
// username, password
// );
// Ok(rocket)
// },
// Err(e) => {
// eprintln!("Error creating admin {:?}", e);
// Err(rocket)
// },
// }
// }
//
// fn create_client(
// rocket: Rocket<Ignite>,
// name: String,
// ) -> std::result::Result<Rocket<Ignite>, Rocket<Ignite>> {
// let conn = DbConn::get_one(&rocket).expect("database connection");
// let client = Client::find_by_name(&name, &conn).or_else(|_e| {
// let mut new_client = Client::create(NewClient { name }, &conn)?;
// new_client.needs_grant = true;
// new_client.redirect_uri_list =
// String::from("http://localhost:8000/trueclient/home");
// new_client.update(&conn)
// });
//
// match client {
// Ok(client) => {
// println!(
// "Created client \"{}\" with secrect \"{}\"",
// client.name, client.secret
// );
// Ok(rocket)
// },
// Err(e) => {
// eprintln!("Error creating client {:?}", e);
// Err(rocket)
// },
// }
// }
//
// fn seed_users(
// rocket: Rocket<Ignite>,
// config: Config,
// amount: usize,
// ) -> std::result::Result<Rocket<Ignite>, Rocket<Ignite>> {
// let conn = DbConn::get_one(&rocket).expect("database connection");
// for i in 0..amount {
// let name = format!("seeded_user_{}", i);
// let f = if i < amount / 2 {
// User::create
// } else {
// User::create_pending
// };
// if let Err(e) = f(
// NewUser {
// username:  name.clone(),
// password:  super::random_token(12),
// full_name: format!("Example {}", i),
// email:     format!("{}@example.com", name),
// ssh_key:   None,
// },
// config.bcrypt_cost,
// &conn,
// ) {
// eprintln!("Error creating user {:?}", e);
// break;
// }
// }
//
// Ok(rocket)
// }
//
// async fn seed_clients(
// rocket: Rocket<Ignite>,
// amount: usize,
// ) -> std::result::Result<Rocket<Ignite>, Rocket<Ignite>> {
// let conn = DbConn::get_one(&rocket).await.expect("database connection");
//
// for i in 0..amount {
// let name = format!("seeded_client_{}", i);
// if let Err(e) = Client::find_by_name(&name, &conn).or_else(|_e| {
// let mut new_client = Client::create(NewClient { name }, &conn)?;
// new_client.needs_grant = i < amount / 2;
// new_client.redirect_uri_list =
// format!("http://localhost:{}/trueclient/home", 8000 + i);
// new_client.update(&conn)
// }) {
// eprintln!("Error creating client {:?}", e);
// break;
// }
// }
//
// Ok(rocket)
// }
//
// fn delete_all(
// rocket: Rocket<Ignite>,
// ) -> std::result::Result<Rocket<Ignite>, Rocket<Ignite>> {
// use crate::models::client::*;
//
// let conn = DbConn::get_one(&rocket).expect("database connection");
// let delete_users = User::all(&conn).and_then(|users| {
// users
// .into_iter()
// .map(|u| User::delete(u, &conn))
// .fold(Ok(()), Result::and)
// });
// if let Err(e) = delete_users {
// eprintln!("Failed deleting users. {:?}", e);
// }
//
// let delete_clients = Client::all(&conn).and_then(|users| {
// users
// .into_iter()
// .map(|c| Client::delete(c, &conn))
// .fold(Ok(()), Result::and)
// });
// if let Err(e) = delete_clients {
// eprintln!("Failed deleting clients. {:?}", e);
// }
//
// Ok(rocket)
// }
// }
