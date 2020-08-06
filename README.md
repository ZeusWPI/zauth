# Zauth

The name is open for discussion.

## Development setup

1. We currently use a MySQL server for persistent storage (also in development 
  mode). So you'll have to install and run your own mysql (or mariadb) server.

2. Next, you'll have to create a user:
```sql
CREATE USER 'zauth'@'localhost' IDENTIFIED BY 'zauth';
CREATE DATABASE zauth;
CREATE DATABASE zauth_test;
GRANT ALL PRIVILEGES ON zauth . * TO 'zauth'@'localhost';
GRANT ALL PRIVILEGES ON zauth_test . * TO 'zauth'@'localhost';
```

3. We use [diesel](http://diesel.rs/) to manage our database. Install the cli
  with `cargo install diesel_cli`.

4. Create the development database with
  `diesel database reset --database-url "mysql://zauth:zauth@localhost/zauth_test"`.
  This will also run the migrations.

5. You can start the server with `cargo run`, it should run on
  [localhost:8000](http://localhost:8000).

You can now start developing! A good way to start is to look at the routes defined in the [controllers](./src/controllers/).
