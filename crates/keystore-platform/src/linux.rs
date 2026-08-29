//! Reviewed Linux Secret Service implementation.
//!
//! This module contains no `unsafe`: the D-Bus conversation is carried by
//! `zbus`, and no descriptor, connection, or session object crosses the module
//! boundary. The broker *stores* the secret, so the blob persisted beside the
//! recipient record carries no key byte at all -- only the label binding needed
//! to find the item again.
//!
//! Secret Service is asynchronous and `zbus`'s own `block_on` drives a shared
//! multi-threaded runtime, which panics when called from inside another Tokio
//! runtime. The bridge below therefore runs each conversation on a dedicated
//! thread with its own current-thread runtime, so the facade stays callable
//! from a synchronous caller and from inside the daemon's runtime alike. The
//! calling thread blocks for the length of the unlock, which is the intended
//! behaviour for a key broker.

use std::collections::HashMap;

use zbus::{
    Connection,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

use crate::{
    KeystoreError, KeystoreErrorCode, KeystoreLabel, PROVIDER, PurgeOutcome, RecoveredSecret,
    encode_envelope,
};

const SERVICE_NAME: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";
const SERVICE_INTERFACE: &str = "org.freedesktop.Secret.Service";
const COLLECTION_INTERFACE: &str = "org.freedesktop.Secret.Collection";
const ITEM_INTERFACE: &str = "org.freedesktop.Secret.Item";
const SESSION_INTERFACE: &str = "org.freedesktop.Secret.Session";
const DEFAULT_COLLECTION_PATH: &str = "/org/freedesktop/secrets/aliases/default";

/// Attribute key the broker searches on. Values are labels, never secrets.
const ATTRIBUTE_KEY: &str = "academic-os-label";
/// Second attribute pinning the record shape, so a future format can coexist.
const ATTRIBUTE_SCHEMA: &str = "xdg:schema";
const ATTRIBUTE_SCHEMA_VALUE: &str = "org.academic.os.DeviceWrappingKey.v1";

/// A D-Bus `Secret`: `(o session, ay parameters, ay value, s content_type)`.
type SecretValue = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);

const PLAINTEXT_CONTENT_TYPE: &str = "application/octet-stream";

/// The `/` object path the service returns when no prompt is required.
const NO_PROMPT: &str = "/";

fn unavailable(operation: &'static str) -> KeystoreError {
    KeystoreError::new(KeystoreErrorCode::Unavailable, operation, None)
}

fn os_failure(operation: &'static str) -> KeystoreError {
    KeystoreError::new(KeystoreErrorCode::OperatingSystem, operation, None)
}

/// Runs one Secret Service conversation on its own current-thread runtime.
///
/// A dedicated thread is used rather than `zbus::block_on` so the call is legal
/// from inside an existing Tokio runtime.
fn run_conversation<T, F>(operation: &'static str, work: F) -> Result<T, KeystoreError>
where
    T: Send + 'static,
    F: FnOnce(Connection) -> BoxedFuture<T> + Send + 'static,
{
    let joined = std::thread::Builder::new()
        .name("academic-keystore".to_owned())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| unavailable(operation))?;
            runtime.block_on(async move {
                let connection = Connection::session()
                    .await
                    .map_err(|_| unavailable(operation))?;
                work(connection).await
            })
        })
        .map_err(|_| unavailable(operation))?
        .join();
    match joined {
        Ok(result) => result,
        // The worker thread panicked. Report it as an operating-system failure
        // rather than propagating the panic into a caller that is holding key
        // material; the crate denies `panic` in its own code.
        Err(_) => Err(os_failure(operation)),
    }
}

type BoxedFuture<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, KeystoreError>> + Send>>;

/// Opens a `plain` transport session.
///
/// The device wrapping key therefore crosses the session bus in the clear, on
/// `CreateItem` and on every `GetSecret`. That is a registered decision, not an
/// oversight: ADR-005 "Secret Service transport: the session is `plain`" states
/// what it exposes and why `dh-ietf1024-sha256-aes128-cbc-pkcs7` is not used
/// instead. Do not change the algorithm here without changing that section.
async fn open_session(
    connection: &Connection,
    operation: &'static str,
) -> Result<OwnedObjectPath, KeystoreError> {
    let empty = Value::new(String::new());
    let reply = connection
        .call_method(
            Some(SERVICE_NAME),
            SERVICE_PATH,
            Some(SERVICE_INTERFACE),
            "OpenSession",
            &("plain", &empty),
        )
        .await
        .map_err(|_| unavailable(operation))?;
    let (_output, session): (OwnedValue, OwnedObjectPath) = reply
        .body()
        .deserialize()
        .map_err(|_| os_failure(operation))?;
    Ok(session)
}

async fn close_session(connection: &Connection, session: &OwnedObjectPath) {
    let _ = connection
        .call_method(
            Some(SERVICE_NAME),
            session,
            Some(SESSION_INTERFACE),
            "Close",
            &(),
        )
        .await;
}

fn attributes_for(label: &KeystoreLabel) -> HashMap<String, String> {
    HashMap::from([
        (ATTRIBUTE_KEY.to_owned(), label.as_str().to_owned()),
        (
            ATTRIBUTE_SCHEMA.to_owned(),
            ATTRIBUTE_SCHEMA_VALUE.to_owned(),
        ),
    ])
}

/// Finds the single item stored under `label`, unlocking it if needed.
async fn find_item(
    connection: &Connection,
    label: &KeystoreLabel,
    operation: &'static str,
) -> Result<Option<OwnedObjectPath>, KeystoreError> {
    let reply = connection
        .call_method(
            Some(SERVICE_NAME),
            SERVICE_PATH,
            Some(SERVICE_INTERFACE),
            "SearchItems",
            &(attributes_for(label),),
        )
        .await
        .map_err(|_| unavailable(operation))?;
    let (unlocked, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) = reply
        .body()
        .deserialize()
        .map_err(|_| os_failure(operation))?;
    if let Some(found) = unlocked.into_iter().next() {
        return Ok(Some(found));
    }
    let Some(first_locked) = locked.first().cloned() else {
        return Ok(None);
    };
    let reply = connection
        .call_method(
            Some(SERVICE_NAME),
            SERVICE_PATH,
            Some(SERVICE_INTERFACE),
            "Unlock",
            &(vec![first_locked],),
        )
        .await
        .map_err(|_| unavailable(operation))?;
    let (newly_unlocked, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = reply
        .body()
        .deserialize()
        .map_err(|_| os_failure(operation))?;
    if prompt.as_str() != NO_PROMPT {
        // A key broker has no user interface and must not drive a prompt on the
        // caller's behalf. An unlock that needs one fails closed.
        return Err(KeystoreError::new(
            KeystoreErrorCode::AccessDenied,
            operation,
            None,
        ));
    }
    Ok(newly_unlocked.into_iter().next())
}

async fn seal_inner(
    connection: Connection,
    label: KeystoreLabel,
    secret: Vec<u8>,
    operation: &'static str,
) -> Result<Vec<u8>, KeystoreError> {
    let session = open_session(&connection, operation).await?;
    let secret_value: SecretValue = (
        session.clone(),
        Vec::new(),
        secret,
        PLAINTEXT_CONTENT_TYPE.to_owned(),
    );
    let attributes = attributes_for(&label);
    let mut properties: HashMap<&str, Value<'_>> = HashMap::new();
    properties.insert(
        "org.freedesktop.Secret.Item.Label",
        Value::new(format!("Academic OS device key ({})", label.as_str())),
    );
    let attribute_value = match Value::new(attributes).try_to_owned() {
        Ok(owned) => owned,
        Err(_) => {
            close_session(&connection, &session).await;
            return Err(os_failure(operation));
        }
    };
    properties.insert(
        "org.freedesktop.Secret.Item.Attributes",
        attribute_value.into(),
    );

    // Kept owned rather than moved into a temporary, so the copy of the
    // wrapping key this call needed can be cleared once the call is over. The
    // buffer `zbus` serialized the message into is not reachable from here;
    // ADR-005 records that boundary.
    let mut payload = (properties, secret_value, true);
    let result = connection
        .call_method(
            Some(SERVICE_NAME),
            DEFAULT_COLLECTION_PATH,
            Some(COLLECTION_INTERFACE),
            "CreateItem",
            &payload,
        )
        .await;
    close_session(&connection, &session).await;
    payload.1.2.fill(0);

    let reply = result.map_err(|_| unavailable(operation))?;
    let (item, prompt): (OwnedObjectPath, OwnedObjectPath) = reply
        .body()
        .deserialize()
        .map_err(|_| os_failure(operation))?;
    if item.as_str() == NO_PROMPT && prompt.as_str() != NO_PROMPT {
        return Err(KeystoreError::new(
            KeystoreErrorCode::AccessDenied,
            operation,
            None,
        ));
    }
    // The blob carries only the label binding: on this provider the operating
    // system holds the secret, so the recipient record stores no key byte.
    Ok(encode_envelope(PROVIDER, label.as_str().as_bytes()))
}

async fn open_inner(
    connection: Connection,
    label: KeystoreLabel,
    operation: &'static str,
) -> Result<Vec<u8>, KeystoreError> {
    let Some(item) = find_item(&connection, &label, operation).await? else {
        return Err(KeystoreError::new(
            KeystoreErrorCode::NotFound,
            operation,
            None,
        ));
    };
    let session = open_session(&connection, operation).await?;
    let result = connection
        .call_method(
            Some(SERVICE_NAME),
            &item,
            Some(ITEM_INTERFACE),
            "GetSecret",
            &(&session,),
        )
        .await;
    close_session(&connection, &session).await;

    let reply = result.map_err(|_| unavailable(operation))?;
    let (_session, _parameters, value, _content_type): SecretValue = reply
        .body()
        .deserialize()
        .map_err(|_| os_failure(operation))?;
    if value.is_empty() {
        return Err(KeystoreError::new(
            KeystoreErrorCode::NotFound,
            operation,
            None,
        ));
    }
    Ok(value)
}

async fn purge_inner(
    connection: Connection,
    label: KeystoreLabel,
    operation: &'static str,
) -> Result<PurgeOutcome, KeystoreError> {
    let Some(item) = find_item(&connection, &label, operation).await? else {
        return Ok(PurgeOutcome::NothingStored);
    };
    connection
        .call_method(
            Some(SERVICE_NAME),
            &item,
            Some(ITEM_INTERFACE),
            "Delete",
            &(),
        )
        .await
        .map_err(|_| unavailable(operation))?;
    Ok(PurgeOutcome::Removed)
}

/// Confirms the blob was written for this label before any bus traffic.
fn check_label_binding(
    label: &KeystoreLabel,
    payload: &[u8],
    operation: &'static str,
) -> Result<(), KeystoreError> {
    if payload == label.as_str().as_bytes() {
        Ok(())
    } else {
        Err(KeystoreError::new(
            KeystoreErrorCode::InvalidSealedBlob,
            operation,
            None,
        ))
    }
}

pub(crate) fn seal(
    label: &KeystoreLabel,
    secret: &[u8],
    operation: &'static str,
) -> Result<Vec<u8>, KeystoreError> {
    let owned_label = label.clone();
    let owned_secret = secret.to_vec();
    run_conversation(operation, move |connection| {
        Box::pin(seal_inner(connection, owned_label, owned_secret, operation))
    })
}

pub(crate) fn open(
    label: &KeystoreLabel,
    payload: &[u8],
    operation: &'static str,
) -> Result<RecoveredSecret, KeystoreError> {
    check_label_binding(label, payload, operation)?;
    let owned_label = label.clone();
    let recovered = run_conversation(operation, move |connection| {
        Box::pin(open_inner(connection, owned_label, operation))
    })?;
    Ok(RecoveredSecret::new(recovered))
}

pub(crate) fn purge(
    label: &KeystoreLabel,
    payload: &[u8],
    operation: &'static str,
) -> Result<PurgeOutcome, KeystoreError> {
    check_label_binding(label, payload, operation)?;
    let owned_label = label.clone();
    run_conversation(operation, move |connection| {
        Box::pin(purge_inner(connection, owned_label, operation))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(value: &str) -> KeystoreLabel {
        match KeystoreLabel::new(value) {
            Ok(label) => label,
            Err(error) => unreachable!("test label rejected: {error}"),
        }
    }

    #[test]
    fn attributes_carry_the_label_and_schema_but_no_secret() {
        let attributes = attributes_for(&label("device:v1"));
        assert_eq!(
            attributes.get(ATTRIBUTE_KEY).map(String::as_str),
            Some("device:v1")
        );
        assert_eq!(
            attributes.get(ATTRIBUTE_SCHEMA).map(String::as_str),
            Some(ATTRIBUTE_SCHEMA_VALUE)
        );
        assert_eq!(attributes.len(), 2);
    }

    #[test]
    fn a_blob_written_for_another_label_is_refused_before_any_bus_traffic() {
        let stored = label("first");
        assert!(check_label_binding(&stored, b"first", "test").is_ok());
        let Err(error) = check_label_binding(&stored, b"second", "test") else {
            unreachable!("a foreign label binding must be refused");
        };
        assert_eq!(error.code, KeystoreErrorCode::InvalidSealedBlob);
    }
}
