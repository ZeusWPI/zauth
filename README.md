# Zauth

The name is open for discussion.

## Development setup

1. Make sure you have a recent version of rust installed. We used 1.57-nightly.

2. We currently use a postgresql server for persistent storage (also in development mode). So you'll have to install and run your own postgresql server. After installation, you can start a postgresql shell with `psql`, but you will likely need access it as the postgres user, so use e.g. `sudo -u postgres psql`.

3. Next, in this shell, you'll have to create a user with permissions to create a database:

    ```sql
    CREATE USER zauth WITH PASSWORD 'zauth' CREATEDB;
    ```

    Note that by default the postgresql server is configured to not allow for username:password authentication. Please refer to [the official documentation](https://www.postgresql.org/docs/9.1/auth-pg-hba-conf.html) for information about postgresql authentication.

    Execute `SHOW hba_file;` in the psql shell to find the location of the client authentication file, which might for example be `/var/lib/pgsql/data/pg_hba.conf`.

    A configuration allowing password authentication for the zauth user on the local system could look like this:

    ```pg_hba.conf
    host    all             zauth           127.0.0.1/32            md5
    host    all             zauth           ::1/128                 md5
    ```

4. We use [diesel](http://diesel.rs/) to manage our database. Install the cli with `cargo install diesel_cli`. If you wish to only install the postgres features of `diesel` (for nix-shell for example), run

    ```shell script
    cargo install diesel_cli --no-default-features --features postgres --force
    ```

5. Create the development and testing database with

    ```shell script
    diesel database reset --database-url "postgresql://zauth:zauth@localhost/zauth"
    diesel database reset --database-url "postgresql://zauth:zauth@localhost/zauth_test"
    ```

    This will also run the migrations.

6. Run `npm run build` to compile the css assets. \
   When working on the stylesheets, you can run `npm run watch` to automatically recompile them on every change.

7. You can now start the server with `cargo run`. If you want to create an admin user you can start it with the `ZAUTH_ADMIN_PASSWORD` environment variable:

    ```shell script
    ZAUTH_ADMIN_PASSWORD=adminadmin cargo run
    ```

   The server should then run on [localhost:8000](http://localhost:8000) and create
   an admin user with password 'adminadmin'.

   There are also other environment variables like:
    - `ZAUTH_EMPTY_DB=true` to clean the database from users and clients
    - `ZAUTH_CLIENT_NAME=mozaic_inc` to create a client with the name mozaic_inc
    - `ZAUTH_SEED_USER=100` to create 100 users, some are active, some are pending (always creates the same users)
    - `ZAUTH_SEED_CLIENT=100` to create 100 clients, some need a grant, others don't (always creates the same clients)

You can now start developing! A good way to start is to look at the routes defined in the [controllers](./src/controllers/).

### Testing the OAuth2 flow

There are tests trying out the OAuth2 flow which can be run with `cargo test`.

You can also test the OAuth2 flow manually by running the flask application in
`test_client/client.py`.

### Using Nix

We have provided a [flake.nix](./flake.nix) for easy setup for Nix users. With [flakes enabled](https://nixos.wiki/wiki/Flakes), run `nix develop`.
