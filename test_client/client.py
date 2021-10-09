import os
import random
import string
import html
import urllib.parse

import requests as requests
from flask import Flask, redirect, request

CLIENT_PORT = 8001
CLIENT_ID = os.environ.get("CLIENT_ID", "test")
CLIENT_SECRET = os.environ.get("CLIENT_SECRET", "CHANGE ME")

ZAUTH_BASE_URL = "http://zauth.localhost:8000"

app = Flask(__name__)

state = None


def authenticate_params():
    global state
    state = "".join(random.choices(string.ascii_letters, k=10))
    return {
        "client_id": CLIENT_ID,
        "response_type": "code",
        "redirect_uri": f"http://client.localhost:{CLIENT_PORT}/callback",
        "state": state
    }


@app.route('/')
def homepage():
    return f"""
        <h1>Zauth test client</h1>
        <p><b>Client ID:</b> { CLIENT_ID }
        <form action="/secret" method="post">
            <label for="secret"><b>Client secret: </b></label>
            <input type="text" value="{ CLIENT_SECRET }"
                   name="secret" id="secret">
            <input type="submit" value="submit">
        </form>
        <p><b>Client redirect URI:</b> http://localhost:{CLIENT_PORT}/callback
        <p><a href="/authenticate">Start authentication flow</a>

    """


@app.route('/secret', methods=['POST'])
def change_secret():
    global CLIENT_SECRET
    CLIENT_SECRET = request.form['secret']
    return redirect("/")


@app.route('/authenticate')
def authenticate():
    params = urllib.parse.urlencode(authenticate_params())
    return redirect(f"{ZAUTH_BASE_URL}/oauth/authorize?{params}")


def fetch_token(code):
    auth = (CLIENT_ID, CLIENT_SECRET)
    data = {
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": f"http://client.localhost:{CLIENT_PORT}/callback",
    }
    return requests.post(f"{ZAUTH_BASE_URL}/oauth/token", auth=auth, data=data)


def fetch_user(access_token):
    return requests.get(
        f"{ZAUTH_BASE_URL}/current_user",
        headers={
            "Authorization": "Bearer " + access_token,
            "Accept": "application/json"
        }
    )


@app.route('/callback')
def callback():
    global state
    response = [
        "<h1> Callback received </h1>"
    ]
    code = request.args['code']
    response.append(f"<p><b>Code</b>: { html.escape(code) }")
    callback_state = request.args['state']
    response.append(f"<p><b>State</b>: { html.escape(callback_state) } ")
    if state == callback_state:
        response.append("<b>OK</b>")
    else:
        response.append(f"<b>NOT OK</b> (should be {state})")

    token_response = fetch_token(code)
    if token_response.ok:
        token_json = token_response.json()
        access_token = token_json["access_token"]
        response.append(
            f"<p>OK fetching access token: { html.escape(access_token) }")

        user_response = fetch_user(access_token)
        if user_response.ok:
            user_json = user_response.json()
            response.append(f"<p>OK fetching current user:")
            response.append(str(user_json))
        else:
            response.append(f"<p>ERROR fetching current user:")
            response.append(html.escape(str(user_response.content)))
    else:
        response.append(f"<p>ERROR fetching token: ")
        response.append(html.escape(str(token_response.content)))
    return "".join(response)


if __name__ == "__main__":
    app.run(debug=True, port=CLIENT_PORT)
