# Zauth

The name is open for discussion.

## Development setup

1. We currently use a postgresql server for persistent storage (also in development 
  mode). So you'll have to install and run your own postgresql server.

2. Next, you'll have to create a user with permissions to create a database:
    ```sql
    CREATE USER zauth WITH PASSWORD 'zauth' CREATEDB;
    ```

3. We use [diesel](http://diesel.rs/) to manage our database. Install the cli
  with `cargo install diesel_cli`. If you wish to only install the postgres features
  of `diesel` (for nix-shell for example), run
  ```
  cargo install diesel_cli --no-default-features --features postgres --force
  ```

4. Create the development and testing database with
    ```shell script
    diesel database reset --database-url "postgresql://zauth:zauth@localhost/zauth"
    diesel database reset --database-url "postgresql://zauth:zauth@localhost/zauth_test"
    ```     
  This will also run the migrations.

5. You can start the server with `cargo run`.
   If you want to create an admin user you can start it with the
   `ZAUTH_ADMIN_PASSWORD` environment variable:
    ```
    ZAUTH_ADMIN_PASSWORD=admin cargo run
    ```
   The server should then run on [localhost:8000](http://localhost:8000) and create
   an admin user with password 'admin'.

You can now start developing! A good way to start is to look at the routes defined in the [controllers](./src/controllers/).

### Using `.envrc` for dev environment

The snippet below can be used in a `.envrc` file in combination with `direnv` to
automatically setup the dev environment needed to work on `zauth`. It does the
following things:

1. Create a `db` folder to store the Postgres data and config
2. Adds a Postgres config to run on `localhost`
3. Sets up a default user and the `zauth` user
4. Run `postgres` in the directory to run a postgres server with above configuration

```
eval "$(direnv)"

# Place the data directory inside the project directory
export PGDATA="$(pwd)/db"
# Place Postgres' Unix socket inside the data directory
export PGHOST="$PGDATA"

if [[ ! -d "$PGDATA" ]]; then
        # If the data directory doesn't exist, create an empty one, and...
        initdb
        # ...configure it to listen only on the Unix socket, and...
        cat >> "$PGDATA/postgresql.conf" <<-EOF
                listen_addresses = 'localhost'
                unix_socket_directories = '$PGHOST'
        EOF
        # ...create a database using the name Postgres defaults to.
        echo "CREATE DATABASE $USER;" | postgres --single -E postgres
        echo "CREATE USER zauth WITH PASSWORD 'zauth' CREATEDB;" | postgres --single -E postgres
fi
```
