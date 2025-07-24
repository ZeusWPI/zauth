// @generated automatically by Diesel CLI.

pub mod sql_types {
	#[derive(diesel::sql_types::SqlType)]
	#[diesel(postgres_type(name = "user_state"))]
	pub struct UserState;
}

diesel::table! {
	clients (id) {
		id -> Int4,
		#[max_length = 255]
		name -> Varchar,
		description -> Text,
		#[max_length = 255]
		secret -> Varchar,
		needs_grant -> Bool,
		redirect_uri_list -> Text,
		created_at -> Timestamp,
	}
}

diesel::table! {
	mails (id) {
		id -> Int4,
		sent_on -> Timestamp,
		subject -> Text,
		body -> Text,
		#[max_length = 255]
		author -> Varchar,
	}
}

diesel::table! {
	passkeys (id) {
		id -> Integer,
		user_id -> Integer,
		#[max_length = 255]
		name -> Varchar,
		cred -> Varchar,
		cred_id -> Varchar,
		last_used -> Timestamp,
		created_at -> Timestamp,
	}
}

diesel::table! {
	roles (id) {
		id -> Int4,
		#[max_length = 255]
		name -> Varchar,
		#[max_length = 255]
		description -> Varchar,
		client_id -> Nullable<Int4>,
	}
}

diesel::table! {
	sessions (id) {
		id -> Int4,
		#[max_length = 255]
		key -> Nullable<Varchar>,
		user_id -> Int4,
		client_id -> Nullable<Int4>,
		created_at -> Timestamp,
		expires_at -> Timestamp,
		valid -> Bool,
		scope -> Nullable<Text>,
	}
}

diesel::table! {
	use diesel::sql_types::*;
	use crate::models::user::UserStateMapping;

		users {
			id -> Int4,
			username -> Varchar,
			hashed_password -> Varchar,
			admin -> Bool,
			password_reset_token -> Nullable<Varchar>,
			password_reset_expiry -> Nullable<Timestamp>,
			full_name -> Varchar,
			email -> Varchar,
			pending_email -> Nullable<Varchar>,
			pending_email_token -> Nullable<Varchar>,
			pending_email_expiry -> Nullable<Timestamp>,
			ssh_key -> Nullable<Text>,
			state -> UserStateMapping,
			last_login -> Timestamp,
			created_at -> Timestamp,
			subscribed_to_mailing_list -> Bool,
			unsubscribe_token -> Varchar,
		}
}

diesel::table! {
	users_roles (user_id, role_id) {
		user_id -> Int4,
		role_id -> Int4,
	}
}

diesel::joinable!(passkeys -> users (user_id));
diesel::joinable!(roles -> clients (client_id));
diesel::joinable!(sessions -> clients (client_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(users_roles -> roles (role_id));
diesel::joinable!(users_roles -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
	clients,
	mails,
	passkeys,
	roles,
	sessions,
	users,
	users_roles,
);
