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
  with `cargo install diesel_cli`.

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
