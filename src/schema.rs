table! {
    client (id) {
        id -> Integer,
        name -> Varchar,
        secret -> Varchar,
        needs_grant -> Bool,
        redirect_uri_list -> Varchar,
    }
}

table! {
    user (id) {
        id -> Integer,
        username -> Varchar,
        password -> Varchar,
        admin -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    client,
    user,
);
