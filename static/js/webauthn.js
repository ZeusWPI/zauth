function register_passkey() {
  const name = document.getElementById("passkey-name").value;
    const resident = document.getElementById("passkey-resident").checked;
    fetch('/webauthn/start_register', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(resident)
    })
    .then(response => response.json() )
    .then(credentialCreationOptions => {
        credentialCreationOptions.publicKey.challenge = Base64.toUint8Array(credentialCreationOptions.publicKey.challenge);
        credentialCreationOptions.publicKey.user.id = Base64.toUint8Array(credentialCreationOptions.publicKey.user.id);
        credentialCreationOptions.publicKey.excludeCredentials?.forEach(function (listItem) {
            listItem.id = Base64.toUint8Array(listItem.id)
        });

        return navigator.credentials.create({
            publicKey: credentialCreationOptions.publicKey
        });
    })
    .then((credential) => {
        fetch('/webauthn/finish_register', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
              name: name,
              credential: {
                id: credential.id,
                rawId: Base64.fromUint8Array(new Uint8Array(credential.rawId), true),
                type: credential.type,
                response: {
                    attestationObject: Base64.fromUint8Array(new Uint8Array(credential.response.attestationObject), true),
                    clientDataJSON: Base64.fromUint8Array(new Uint8Array(credential.response.clientDataJSON), true),
                },
              }
            })
        })
        .then(finish);
    })
}

function login_passkey() {
    const username = document.getElementById("login-username").value;
    let id = null;
    fetch('/webauthn/start_auth',  {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify((username.length > 0) ? username : null)
    })
    .then(response => response.json())
    .then(([id,credentialRequestOptions]) => {
        credentialRequestOptions.publicKey.challenge = Base64.toUint8Array(credentialRequestOptions.publicKey.challenge);
        credentialRequestOptions.publicKey.allowCredentials?.forEach(function (listItem) {
            listItem.id = Base64.toUint8Array(listItem.id)
        });

        this.id = id
        return navigator.credentials.get({
            publicKey: credentialRequestOptions.publicKey,
        });
    })
    .then((assertion) =>  {
      return {
          id: assertion.id,
          rawId: Base64.fromUint8Array(new Uint8Array(assertion.rawId), true),
          type: assertion.type,
          response: {
            authenticatorData: Base64.fromUint8Array(new Uint8Array(assertion.response.authenticatorData), true),
            clientDataJSON: Base64.fromUint8Array(new Uint8Array(assertion.response.clientDataJSON), true),
            signature: Base64.fromUint8Array(new Uint8Array(assertion.response.signature), true),
            userHandle: Base64.fromUint8Array(new Uint8Array(assertion.response.userHandle), true)
          }
      }
    }
    , (error) => null)
    .then((credential) => {
        fetch('/webauthn/finish_auth', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                id: this.id,
                username: ((username.length > 0) ? username : null),
                credential: credential,
            }),
        })
        .then(finish);
    })
}

function finish(response) {
    const contentType = response.headers.get('Content-Type');
    if (response.ok && response.redirected){
        window.location.href = response.url;
    } else if (contentType && contentType.includes('text/html')){
        response.text().then((html) => document.documentElement.innerHTML = html);
    } 
}
