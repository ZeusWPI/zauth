#![allow(dead_code)]

extern crate diesel;
extern crate parking_lot;
extern crate rocket;
extern crate tempfile;
extern crate urlencoding;
extern crate zauth;

use diesel::sql_query;
use diesel::RunQueryDsl;
use parking_lot::Mutex;
use std::str::FromStr;

use crate::common::zauth::config::Config;
use crate::common::zauth::models::client::*;
use crate::common::zauth::models::user::*;
use crate::common::zauth::DbConn;
use lettre::Address;
use rocket::http::{ContentType, Status};
use rocket::tokio::time::sleep;
use std::future::Future;
use std::time::Duration;
use zauth::mailer::STUB_MAILER_OUTBOX;

pub type HttpClient = rocket::local::asynchronous::Client;

// Rocket doesn't support transactional testing yet, so we use a lock to
// serialize tests.
static DB_LOCK: Mutex<()> = Mutex::new(());

pub fn url(content: &str) -> String {
	urlencoding::encode(content).into_owned()
}

pub static BCRYPT_COST: u32 = 4;

pub fn config() -> Config {
	Config {
		admin_email: "admin@example.com".to_string(),
		user_session_seconds: 300,
		client_session_seconds: 300,
		authorization_token_seconds: 300,
		email_confirmation_token_seconds: 300,
		secure_token_length: 64,
		bcrypt_cost: BCRYPT_COST,
		ec_private_key: "keys/jwt_key.pem".to_string(),
		base_url: "example.com".to_string(),
		mail_queue_size: 10,
		mail_queue_wait_seconds: 0,
		mail_from: "zauth@example.com".to_string(),
		mail_server: "stub".to_string(),
		mailing_list_name: "Leden".to_string(),
		mailing_list_email: "leden@zeus.ugent.be".to_string(),
		maximum_pending_users: 5,
	}
}

async fn reset_db(db: &DbConn) {
	db.run(|conn| {
		sql_query("TRUNCATE TABLE mails, sessions, users, clients, passkeys")
			.execute(conn)
			.expect("drop all tables");
	})
	.await
}

/// Creates a rocket::local::Client for testing purposes. The rocket instance
/// will be configured with a Sqlite database located in a tmpdir  This
/// executes the given function with the Client a connection to that
/// database. The database and its directory will be removed after the function
/// has run.
pub async fn as_visitor<F, R>(run: F)
where
	F: FnOnce(HttpClient, DbConn) -> R,
	R: Future<Output = ()>,
{
	// Prepare config
	let db_url = "postgresql://zauth:zauth@localhost/zauth_test";
	let config = rocket::Config::figment()
		.merge(("mail_queue_wait_seconds", 0))
		.merge(("maximum_pending_users", 5))
		.merge(("databases.postgresql_database.url", db_url));

	let _lock = DB_LOCK.lock();

	let client = HttpClient::tracked(zauth::prepare_custom(config))
		.await
		.expect("rocket client");

	let db = DbConn::get_one(client.rocket())
		.await
		.expect("database connection");
	reset_db(&db).await;
	assert_eq!(0, User::all(&db).await.unwrap().len());
	assert_eq!(0, Client::all(&db).await.unwrap().len());

	run(client, db).await;
}

pub async fn as_user<F, R>(run: F)
where
	F: FnOnce(HttpClient, DbConn, User) -> R,
	R: Future<Output = ()>,
{
	as_visitor(async move |client: HttpClient, db| {
		let user = User::create(
			NewUser {
				username: String::from("username"),
				password: String::from("password"),
				full_name: String::from("full"),
				email: String::from("user@domain.tld"),
				ssh_key: Some(String::from("ssh-rsa base64== key@hostname")),
				not_a_robot: true,
			},
			BCRYPT_COST,
			&db,
		)
		.await
		.unwrap();

		{
			let response = client
				.post("/login")
				.body("username=username&password=password")
				.header(ContentType::Form)
				.dispatch()
				.await;
			assert_eq!(response.status(), Status::SeeOther, "login failed");
		}

		run(client, db, user).await;
	})
	.await;
}

pub async fn as_admin<F, R>(run: F)
where
	F: FnOnce(HttpClient, DbConn, User) -> R,
	R: Future<Output = ()>,
{
	as_visitor(async move |client: HttpClient, db| {
		let mut user = User::create(
			NewUser {
				username: String::from("admin"),
				password: String::from("password"),
				full_name: String::from("admin name"),
				email: String::from("admin@domain.tld"),
				ssh_key: Some(String::from("ssh-rsa admin admin@hostname")),
				not_a_robot: true,
			},
			BCRYPT_COST,
			&db,
		)
		.await
		.unwrap();

		user.admin = true;
		let user = user.update(&db).await.unwrap();

		{
			let response = client
				.post("/login")
				.body("username=admin&password=password")
				.header(ContentType::Form)
				.dispatch()
				.await;
			assert_eq!(response.status(), Status::SeeOther, "login failed");
		}

		run(client, db, user).await;
	})
	.await;
}

pub async fn dont_expect_mail<T, F, R>(run: F) -> T
where
	F: FnOnce() -> R,
	R: Future<Output = T>,
{
	let (mailbox, _condvar) = &STUB_MAILER_OUTBOX;
	let outbox_size = { mailbox.lock().len() };
	let result: T = run().await;
	sleep(Duration::from_secs(1)).await;
	assert_eq!(
		outbox_size,
		mailbox.lock().len(),
		"Expected no mail to be sent"
	);
	result
}

pub async fn expect_mails<T, F, R>(run: F) -> T
where
	F: FnOnce() -> R,
	R: Future<Output = T>,
{
	let (mailbox, _condvar) = &STUB_MAILER_OUTBOX;
	let outbox_size = { mailbox.lock().len() };
	let result: T = run().await;
	sleep(Duration::from_secs(1)).await;
	assert_ne!(
		outbox_size,
		mailbox.lock().len(),
		"Expected mails to be sent"
	);
	result
}

pub async fn expect_mail_to<T, F, R>(receivers: Vec<&str>, run: F) -> T
where
	F: FnOnce() -> R,
	R: Future<Output = T>,
{
	let (mailbox, condvar) = &STUB_MAILER_OUTBOX;
	let outbox_size = { mailbox.lock().len() };
	let result: T = run().await;

	let mut mailbox = mailbox.lock();
	if mailbox.len() == outbox_size {
		let wait_result =
			condvar.wait_for(&mut mailbox, Duration::from_secs(1));
		assert!(
			!wait_result.timed_out(),
			"Timed out while waiting for email"
		);
	}

	assert_eq!(
		mailbox.len(),
		outbox_size + 1,
		"Expected an email to be sent"
	);
	let last_mail = mailbox.last().unwrap();
	let receivers = receivers
		.into_iter()
		.map(|e| Address::from_str(e).unwrap())
		.collect::<Vec<Address>>();
	assert_eq!(last_mail.envelope().to(), receivers, "Unexpected receivers");
	result
}

pub async fn expect_mails_to<T, F, R>(receivers: Vec<&str>, run: F) -> T
where
	F: FnOnce() -> R,
	R: Future<Output = T>,
{
	let (mailbox, condvar) = &STUB_MAILER_OUTBOX;
	let outbox_size = { mailbox.lock().len() };
	let result: T = run().await;

	let mut mailbox = mailbox.lock();
	while mailbox.len() != outbox_size + receivers.len() {
		let wait_result =
			condvar.wait_for(&mut mailbox, Duration::from_secs(1));
		assert!(
			!wait_result.timed_out(),
			"Timed out while waiting for email"
		);
	}

	assert!(
		mailbox.len() >= outbox_size + 1,
		"Expected at least one email to be sent"
	);
	let sent_mails = mailbox.clone();
	let sent_receivers = sent_mails
		.iter()
		.map(|m| m.envelope().to().to_vec())
		.collect::<Vec<Vec<Address>>>();

	sent_receivers
		.iter()
		.zip(sent_mails)
		.for_each(|(recvs, msg)| {
			assert!(
				recvs.len() == 1,
				"Unexpected receivers {:?} for message {:?}",
				recvs,
				msg
			)
		});

	let mut sent_receivers = sent_receivers
		.into_iter()
		.map(|r| r[0].clone())
		.collect::<Vec<Address>>();

	let mut receivers = receivers
		.into_iter()
		.map(|e| Address::from_str(e).unwrap())
		.collect::<Vec<Address>>();

	assert_eq!(
		sent_receivers.len(),
		receivers.len(),
		"unexpected receivers ({:?}) <-> ({:?})",
		sent_receivers,
		receivers
	);

	sent_receivers.sort_unstable();
	receivers.sort_unstable();

	sent_receivers
		.iter()
		.zip(receivers)
		.for_each(|(sent_recv, recv)| {
			assert_eq!(*sent_recv, recv, "Unexpected receiver");
		});

	println!(
		"sent {} mails to {:?}",
		sent_receivers.len(),
		sent_receivers
	);

	result
}
