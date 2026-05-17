import html
import json
import os
import random
import string
import urllib.parse

from flask import Flask, redirect, request
import requests

CLIENT_PORT = 8001
CLIENT_ID = os.environ.get("CLIENT_ID", "test")
CLIENT_SECRET = os.environ.get("CLIENT_SECRET", "CHANGE ME")

ZAUTH_BASE_URI = "http://localhost:8000"
CLIENT_BASE_URI = f"http://localhost:{CLIENT_PORT}"
CLIENT_CALLBACK_URI = f"{CLIENT_BASE_URI}/callback"

app = Flask(__name__)

state = None


def authenticate_params():
    global state
    state = "".join(random.choices(string.ascii_letters, k=10))
    return {
        "client_id": CLIENT_ID,
        "response_type": "code",
        "redirect_uri": CLIENT_CALLBACK_URI,
        "state": state,
        "scope": "roles"
    }


def fetch_token(code):
    auth = (CLIENT_ID, CLIENT_SECRET)
    data = {
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": CLIENT_CALLBACK_URI
    }
    return requests.post(f"{ZAUTH_BASE_URI}/oauth/token", auth=auth, data=data)


def fetch_user(access_token):
    return requests.get(
        f"{ZAUTH_BASE_URI}/current_user",
        headers={
            "Authorization": "Bearer " + access_token,
            "Accept": "application/json"
        }
    )


@app.route('/')
def homepage():
    return f"""\
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Zauth test client</title>
</head>
<body>
    <h1>Zauth test client</h1>
    <p><b>Client ID:</b> {CLIENT_ID}</p>
    <form action="/secret" method="post">
        <label for="secret"><b>Client secret: </b></label>
        <input type="text" name="secret" value="{CLIENT_SECRET}" />
        <input type="submit" value="submit" />
    </form>
    <p><b>ZAuth base URI:</b> {ZAUTH_BASE_URI}</p>
    <p><b>Client base URI:</b> {CLIENT_BASE_URI}</p>
    <p><b>Client redirect URI:</b> {CLIENT_CALLBACK_URI}</p>
    <p><a href="/authenticate">Start authentication flow</a></p>
</body>
</html>
"""


@app.route('/secret', methods=['POST'])
def change_secret():
    global CLIENT_SECRET
    CLIENT_SECRET = request.form['secret']
    return redirect("/")


@app.route('/authenticate')
def authenticate():
    params = urllib.parse.urlencode(authenticate_params())
    return redirect(f"{ZAUTH_BASE_URI}/oauth/authorize?{params}")


@app.route('/callback')
def callback():
    global state

    callback_code = request.args['code']
    html_callback_code = html.escape(callback_code)

    callback_state = request.args['state']
    if callback_state == state:
        html_callback_state = f'{callback_state} <b>OK</b>'
    else:
        html_callback_state = f'{callback_state} <b>ERROR</b> (should be {state})'

    token_res = fetch_token(callback_code)
    if token_res.ok:
        token_res_json = token_res.json()
        html_token_res = f'<b>OK</b>:<br /><pre>{html.escape(json.dumps(token_res_json, indent=4))}</pre>'

        access_token = token_res_json["access_token"]
        html_access_token = html.escape(access_token)

        user_res = fetch_user(access_token)
        if user_res.ok:
            user_res_json = user_res.json()
            html_user_res = f'<b>OK</b>:<br /><pre>{html.escape(json.dumps(user_res_json, indent=4))}</pre>'
        else:
            html_user_res = f'<b>ERROR ({user_res.status_code})</b><br /><pre>{html.escape(str(user_res.content.decode()))}</pre>'
    else:
        html_token_res = f'<b>ERROR ({token_res.status_code})</b><br /><pre>{html.escape(str(token_res.content.decode()))}</pre>'
        html_access_token = '<b>SKIP</b>'
        html_user_res = '<b>SKIP</b>'
    return f"""\
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Zauth test client</title>
</head>
<body>
    <h1>Received callback from browser:</h1>
    <p>
        <b>Code</b>: {html_callback_code}
    </p>
    <p>
        <b>State</b>: {html_callback_state}
    </p>

    <h1>Getting token from <code>{html.escape(f'{ZAUTH_BASE_URI}/oauth/token')}</code>:</h1>
    <p>
        <b>Response</b>: {html_token_res}
    </p>
    <p>
        <b>Access token</b>: {html_access_token}
    </p>

    <h1>Fetching user info from <code>{html.escape(f'{ZAUTH_BASE_URI}/current_user')}</code>:</h1>
    <p>
        <b>Response</b>: {html_user_res}
    </p>
</body>
</html>
"""


if __name__ == "__main__":
    app.run(debug=True, port=CLIENT_PORT)
