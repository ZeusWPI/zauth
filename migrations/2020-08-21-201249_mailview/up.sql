CREATE VIEW postfix_view AS
    SELECT username, email
    FROM users
    WHERE state = 'active';
